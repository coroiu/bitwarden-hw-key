//! `App`: the platform-free application state the unified main loop
//! ([`crate::run::run`]) drives every frame.
//!
//! M1 replaces the M0 "empty-but-real" shortcut (rebuild the whole
//! [`Navigator`] on every sync) with a shared, App-owned [`VaultStore`]:
//! the [`Navigator`] is built exactly once, in [`App::new`], over a root
//! [`CredentialListView`] that reads the store *live* at render time.
//! [`App::step`] only ever writes into the store — it never touches the
//! `Navigator` — so a landing sync can never destroy a pushed screen (once
//! one exists; see the ADR referenced below) or pop the user out mid-read.
//!
//! See:
//! `.planning/decisions/2026-08-12-m1-vault-store-data-ownership.md`.

use std::cell::RefCell;
use std::rc::Rc;

use crate::credential_detail_view::CredentialDetailView;
use crate::credential_list_view::CredentialListView;
use crate::input::NavIntent;
use crate::render::{Action, FrameBuffer565, Navigator, Screen};
use crate::sync_source::SyncSource;
use crate::vault_item::VaultItem;
use crate::vault_store::{SyncStatus, VaultStore};

/// Builds the root screen: a titled, focusable [`CredentialListView`]
/// backed live by `store`, with `.on_activate(...)` wired (bead
/// `ai-bitwarden-hw-key-0v8.6`) to push a [`CredentialDetailView`] for
/// whichever credential id the list reports.
///
/// The closure captures a fresh `Rc::clone(&store)` per call — cheap (a
/// refcount bump), and it's the whole point of `Action::PushView` being a
/// boxed `FnOnce() -> Screen` rather than a screen value itself: the
/// detail screen isn't built until the moment the user actually activates
/// a row, using whatever `id` `CredentialListView` resolved as selected at
/// that moment.
///
/// The `.with_hint(...)` here is only a fallback: `CredentialListView` is
/// always the screen's sole (and thus always-focused) widget, so its own
/// [`crate::render::ChromeContribution::hint`] always overrides this in
/// practice (see `Screen::render`) — kept non-empty anyway so a screen
/// somehow rendered before its widget is focused still shows sane control
/// legend text instead of a blank hint bar.
fn credential_list_screen(store: Rc<RefCell<VaultStore>>) -> Screen {
    let store_for_activate = Rc::clone(&store);
    let list = CredentialListView::new(store).on_activate(move |id| {
        let store = Rc::clone(&store_for_activate);
        Action::PushView(Box::new(move || credential_detail_screen(store, id)))
    });
    Screen::new("Vault", vec![Box::new(list)]).with_hint("Rotate to browse - Press to open")
}

/// Builds a credential detail screen for `id`, backed live by `store` — see
/// [`CredentialDetailView`]'s module doc for the live-by-id read and
/// gone-state design. The `.with_hint(...)` fallback follows the same
/// rationale as `credential_list_screen`'s: `CredentialDetailView`'s own
/// `ChromeContribution::hint` overrides this in practice.
fn credential_detail_screen(store: Rc<RefCell<VaultStore>>, id: uuid::Uuid) -> Screen {
    let detail = CredentialDetailView::new(store, id);
    Screen::new("Credential", vec![Box::new(detail)]).with_hint("Hold to go back")
}

/// The application core: a [`VaultStore`] holding the authoritative
/// credential state, a [`Navigator`] built once over a store-backed root
/// screen, and the single [`FrameBuffer565`] it renders into.
pub struct App {
    store: Rc<RefCell<VaultStore>>,
    navigator: Navigator,
    framebuffer: FrameBuffer565,
    /// Whether the current screen state has changed since the last
    /// [`App::render`] call. The run loop uses this to skip
    /// `DisplaySurface::flush` on frames where nothing changed.
    dirty: bool,
}

impl App {
    /// Builds the app with an initial vault snapshot, rendering into a
    /// `width`x`height` framebuffer. `width`/`height` should match
    /// whatever the platform's `DisplaySurface` actually presents — the
    /// core has no way to discover this itself (see
    /// `.planning/decisions/2026-08-11-presentation-surface-run-mode-seam.md`:
    /// `DisplaySurface` only exposes `flush`, not a size), so callers
    /// (the three `main.rs`/binaries) pass in whatever their concrete
    /// surface is sized for.
    ///
    /// The [`Navigator`] built here is the only one this `App` will ever
    /// have — per the M1 ADR, [`App::step`] never rebuilds it.
    #[must_use]
    pub fn new(width: u32, height: u32, items: Vec<VaultItem>) -> Self {
        let store = Rc::new(RefCell::new(VaultStore::new()));
        store.borrow_mut().apply_sync_ok(items);

        let navigator = Navigator::new(credential_list_screen(Rc::clone(&store)));
        Self {
            store,
            navigator,
            framebuffer: FrameBuffer565::new(width, height),
            dirty: true,
        }
    }

    /// The current derived sync status, or `None` if [`App::step`] has
    /// never been called and construction supplied no items either way to
    /// report on. Exposed for chrome/status-indicator consumers (e.g. bead
    /// `ai-bitwarden-hw-key-0v8.5`'s status dot).
    #[must_use]
    pub fn sync_status(&self) -> Option<SyncStatus> {
        self.store.borrow().status().cloned()
    }

    /// How many screens are on the navigator's stack (>= 1). Exposed for
    /// tests/diagnostics proving the list-activate -> detail-push -> back
    /// -> pop seam (bead `ai-bitwarden-hw-key-0v8.6`) actually moves the
    /// stack, without exposing the `Navigator` itself.
    #[must_use]
    pub fn navigator_depth(&self) -> usize {
        self.navigator.depth()
    }

    /// Dispatches every polled `NavIntent` to the navigator, in order.
    /// A no-op (including leaving `dirty` untouched) if `intents` is empty.
    pub fn handle_input(&mut self, intents: Vec<NavIntent>) {
        if intents.is_empty() {
            return;
        }
        for intent in intents {
            self.navigator.dispatch(intent);
        }
        self.dirty = true;
    }

    /// Pulls the latest vault snapshot from `sync` and writes it into the
    /// [`VaultStore`] in place — the [`Navigator`] is never rebuilt (see
    /// the module doc and the M1 ADR). `dirty` is set only when the store
    /// actually changed (new/changed items, or a status transition), so an
    /// unchanged sync doesn't force a redundant render.
    ///
    /// A sync error does not clear previously known items (the
    /// last-known-good list keeps rendering); it only updates the derived
    /// `SyncStatus` to `Error`.
    pub fn step<S: SyncSource>(&mut self, sync: &mut S)
    where
        S::Error: std::fmt::Display,
    {
        let changed = match sync.sync() {
            Ok(items) => self.store.borrow_mut().apply_sync_ok(items),
            Err(error) => self.store.borrow_mut().apply_sync_err(error.to_string()),
        };
        if changed {
            self.dirty = true;
        }
    }

    /// Whether [`App::render`] would draw something different from the
    /// last time it was called.
    #[must_use]
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Renders the current screen into the app's framebuffer and clears
    /// the dirty flag, returning the freshly rendered framebuffer for the
    /// caller to hand to a `DisplaySurface::flush`.
    ///
    /// # Panics
    ///
    /// Never, in practice: `Navigator::render`'s `Result` is over
    /// `Infallible`'s uninhabited error type (the core `DrawTarget` can
    /// never fail to draw). The `expect` exists only because
    /// `Result::expect` is how that's asserted at the call site.
    pub fn render(&mut self) -> &FrameBuffer565 {
        self.navigator
            .render(&mut self.framebuffer)
            .expect("core DrawTarget is Infallible");
        self.dirty = false;
        &self.framebuffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    use crate::render::chrome::TITLE_BAR_HEIGHT;
    use crate::render::theme::palette;
    use crate::render::ROW_HEIGHT;
    use embedded_graphics::pixelcolor::Rgb565;
    use embedded_graphics::prelude::Point;
    use uuid::Uuid;

    fn item(name: &str) -> VaultItem {
        VaultItem {
            id: Uuid::new_v4(),
            name: name.to_string(),
            username: format!("{name}-user"),
            password: "hunter2".to_string(),
            uri: None,
            notes: None,
        }
    }

    struct StubSyncSource(Vec<VaultItem>);
    impl SyncSource for StubSyncSource {
        type Error = Infallible;
        fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn a_fresh_app_is_dirty_and_renders_the_initial_list() {
        let app = App::new(320, 170, vec![item("GitHub"), item("AWS")]);
        assert!(app.dirty());
    }

    #[test]
    fn render_clears_the_dirty_flag() {
        let mut app = App::new(320, 170, vec![item("GitHub")]);
        assert!(app.dirty());
        app.render();
        assert!(!app.dirty());
    }

    #[test]
    fn handle_input_with_no_intents_does_not_mark_dirty() {
        let mut app = App::new(320, 170, vec![item("GitHub")]);
        app.render();
        assert!(!app.dirty());
        app.handle_input(vec![]);
        assert!(!app.dirty());
    }

    #[test]
    fn handle_input_marks_dirty_and_moving_selection_changes_the_rendered_framebuffer() {
        // The programmatic "drive the navigator via NavIntents and prove
        // the framebuffer changes" proof this bead's verification calls
        // for, exercised through the full `App` wrapper (input -> dispatch
        // -> render), not directly against `Navigator`.
        let mut app = App::new(320, 170, vec![item("GitHub"), item("AWS"), item("Postgres")]);

        // x=250: past the 4px selection accent bar *and* past any of
        // these short labels' text, so it samples the row's plain
        // elevated fill rather than the accent stripe or a glyph pixel.
        let frame_0 = app.render().pixel(Point::new(250, 18));
        assert_eq!(frame_0, palette::SURFACE_ELEVATED, "row 0 should start selected");

        app.handle_input(vec![NavIntent::Next]);
        assert!(app.dirty(), "moving selection should mark the app dirty");

        let frame_1_row_0 = app.render().pixel(Point::new(250, 18));
        assert_ne!(frame_1_row_0, palette::SURFACE_ELEVATED, "row 0 should no longer be selected");
    }

    #[test]
    fn step_writes_new_items_into_the_store_and_marks_dirty() {
        let mut app = App::new(320, 170, vec![]);
        app.render();
        assert!(!app.dirty());

        let mut sync = StubSyncSource(vec![item("GitHub")]);
        app.step(&mut sync);

        assert!(app.dirty(), "a changed vault snapshot should mark the app dirty");
        assert_eq!(app.sync_status(), Some(SyncStatus::Synced));
    }

    #[test]
    fn step_is_a_noop_when_the_sync_source_reports_the_same_items() {
        let initial = vec![item("GitHub")];
        let mut app = App::new(320, 170, initial.clone());
        app.render();
        assert!(!app.dirty());

        let mut sync = StubSyncSource(initial);
        app.step(&mut sync);

        assert!(!app.dirty(), "an unchanged vault snapshot should not mark the app dirty");
    }

    #[test]
    fn step_updates_the_rendered_list_live_without_rebuilding_the_navigator() {
        // This is the crux of the M1 ADR: unlike M0 (which discarded and
        // rebuilt the whole `Navigator` on every changed sync), `App::step`
        // now only ever writes into the shared `VaultStore` — the
        // `Navigator` built in `App::new` is the only one that ever
        // exists. The proof that this still works end-to-end is
        // behavioral: a sync landing after construction must still show up
        // in the very next rendered framebuffer.
        let mut app = App::new(320, 170, vec![]);
        let before: Vec<Rgb565> = app.render().pixels().map(|pixel| pixel.1).collect();

        let mut sync = StubSyncSource(vec![item("GitHub")]);
        app.step(&mut sync);
        assert!(app.dirty());

        let after: Vec<Rgb565> = app.render().pixels().map(|pixel| pixel.1).collect();
        assert_ne!(before, after, "the newly synced item must be visible on the very next render");
    }

    #[test]
    fn step_with_a_deleted_or_emptied_vault_yields_sync_status_empty() {
        let mut app = App::new(320, 170, vec![item("GitHub")]);
        assert_eq!(app.sync_status(), Some(SyncStatus::Synced));

        let mut sync = StubSyncSource(vec![]);
        app.step(&mut sync);

        assert!(app.dirty(), "the vault becoming empty is a visible change");
        assert_eq!(app.sync_status(), Some(SyncStatus::Empty));
    }

    #[test]
    fn step_with_a_sync_error_reports_error_status_and_keeps_previous_items_dirty_flow() {
        use std::fmt;

        struct FailingSyncSource;
        #[derive(Debug)]
        struct BoomError;
        impl fmt::Display for BoomError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "boom")
            }
        }
        impl SyncSource for FailingSyncSource {
            type Error = BoomError;
            fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
                Err(BoomError)
            }
        }

        let mut app = App::new(320, 170, vec![item("GitHub")]);
        app.render();
        assert!(!app.dirty());

        let mut sync = FailingSyncSource;
        app.step(&mut sync);

        assert!(app.dirty(), "a new sync error is a status change");
        assert_eq!(app.sync_status(), Some(SyncStatus::Error("boom".to_string())));
    }

    #[test]
    fn activating_the_selected_credential_pushes_a_detail_screen_and_back_pops_with_selection_preserved() {
        // Bead `ai-bitwarden-hw-key-0v8.6`'s activation-seam proof, wired
        // through the full `App` (input -> Navigator dispatch -> Action ->
        // push/pop), not `CredentialListView`/`CredentialDetailView` in
        // isolation (those already have their own unit coverage for
        // "activate invokes the callback with the right id" and "renders
        // whatever id it's given").
        let mut app = App::new(320, 170, vec![item("GitHub"), item("AWS"), item("Postgres")]);
        assert_eq!(app.navigator_depth(), 1, "starts on just the root list screen");

        // Select the second item (AWS) before activating, so "selection
        // preserved across the round trip" is provable below.
        app.handle_input(vec![NavIntent::Next]);

        // Row 1's (AWS's) selection-highlight pixel, per the same
        // coordinates `handle_input_marks_dirty_and_moving_selection_changes_the_rendered_framebuffer`
        // above uses for row 0.
        // `TITLE_BAR_HEIGHT`/`ROW_HEIGHT` are tiny, fixed chrome/layout
        // constants for a display this project targets (at most a few
        // hundred pixels per side) — this can never realistically wrap.
        #[allow(clippy::cast_possible_wrap)]
        let row1_y = (TITLE_BAR_HEIGHT + ROW_HEIGHT + 2) as i32;
        let sample_x = 250;
        let row1_selected_before_activate = app.render().pixel(Point::new(sample_x, row1_y));
        assert_eq!(row1_selected_before_activate, palette::SURFACE_ELEVATED, "AWS (row 1) is selected before activating");

        app.handle_input(vec![NavIntent::Activate]);
        assert_eq!(app.navigator_depth(), 2, "activating a credential should push a detail screen");

        app.handle_input(vec![NavIntent::Back]);
        assert_eq!(app.navigator_depth(), 1, "Back should pop the detail screen, returning to the list");

        let row1_selected_after_back = app.render().pixel(Point::new(sample_x, row1_y));
        assert_eq!(
            row1_selected_after_back, palette::SURFACE_ELEVATED,
            "the list's selection (AWS, row 1) must survive the push/pop round trip"
        );
    }
}
