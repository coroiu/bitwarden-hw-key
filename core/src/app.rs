//! `App`: the platform-free application state the unified main loop
//! ([`crate::run::run`]) drives every frame.
//!
//! M1 replaces the M0 "empty-but-real" shortcut (rebuild the whole
//! [`Navigator`] on every sync) with a shared, App-owned [`VaultStore`]:
//! the [`Navigator`] is built exactly once, in [`App::new`], over a root
//! credential list that reads the store *live* at render time.
//! [`App::step`] only ever writes into the store — it never touches the
//! `Navigator` — so a landing sync can never destroy a pushed screen (once
//! one exists; see the ADR referenced below) or pop the user out mid-read.
//!
//! See:
//! `.planning/decisions/2026-08-12-m1-vault-store-data-ownership.md`.

use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;

use embedded_graphics::prelude::Size;
use embedded_graphics::primitives::Rectangle;

use crate::input::NavIntent;
use crate::render::{Action, FocusEvent, FrameBuffer565, ListItem, Navigator, Screen, VerticalList, Widget};
use crate::sync_source::SyncSource;
use crate::vault_item::VaultItem;
use crate::vault_store::{SyncStatus, VaultStore};

fn vault_item_to_list_item(item: &VaultItem) -> ListItem {
    ListItem::new(item.name.clone()).with_sublabel(item.username.clone())
}

/// The root credential list screen's sole content widget: a thin adapter
/// over a shared [`VaultStore`] rather than an owner of a static
/// `Vec<ListItem>`. `render` re-derives its rows from `store.items()` on
/// every call, so a [`VaultStore`] mutation made by [`App::step`] between
/// two renders shows up on the very next one — no `Navigator` rebuild
/// needed.
///
/// Deliberately minimal — this is the "keep the tree green" list, not the
/// product one:
/// - Selection is tracked by index (`selected`), not by `VaultItem` id.
/// - There is no `on_activate` push to a detail screen.
/// - There is no dedicated empty/syncing/error content rendering (an empty
///   store just renders zero rows under the title/hint chrome).
///
/// All of the above are explicitly bead `ai-bitwarden-hw-key-0v8.4`'s job
/// (`CredentialListView`, per the ADR), which supersedes this type
/// entirely. It exists only so the root list keeps rendering and
/// live-updating while that lands.
struct StoreBackedCredentialList {
    store: Rc<RefCell<VaultStore>>,
    selected: usize,
    focused: bool,
}

impl StoreBackedCredentialList {
    fn new(store: Rc<RefCell<VaultStore>>) -> Self {
        Self { store, selected: 0, focused: false }
    }

    fn item_count(&self) -> usize {
        self.store.borrow().items().len()
    }

    // Mirrors `render::list::VerticalList::move_selection` exactly (same
    // `usize`<->`i32` index arithmetic, same module-level allow rationale
    // in `render/mod.rs`: a vault has, realistically, a few hundred items
    // at most on this embedded/emulator target — nowhere near `i32::MAX` —
    // so these conversions cannot truncate, wrap, or lose a sign in
    // practice.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    fn move_selection(&mut self, delta: i32) {
        let len = self.item_count() as i32;
        if len == 0 {
            return;
        }
        let next = (self.selected as i32 + delta).clamp(0, len - 1);
        self.selected = next as usize;
    }
}

impl Widget for StoreBackedCredentialList {
    fn measure(&self, constraints: Size) -> Size {
        constraints
    }

    fn is_focusable(&self) -> bool {
        self.item_count() > 0
    }

    fn on_focus(&mut self, event: FocusEvent) -> Action {
        match event {
            FocusEvent::Gained => self.focused = true,
            FocusEvent::Lost => self.focused = false,
            FocusEvent::Activated => {}
        }
        Action::None
    }

    fn on_intent(&mut self, intent: NavIntent) -> Action {
        match intent {
            NavIntent::Next => self.move_selection(1),
            NavIntent::Prev => self.move_selection(-1),
            NavIntent::NextN(n) => self.move_selection(i32::from(n)),
            NavIntent::Activate | NavIntent::Back => {}
        }
        Action::None
    }

    fn render(&self, area: Rectangle, target: &mut FrameBuffer565) -> Result<(), Infallible> {
        // Live read: today's store contents, not whatever was current when
        // this widget (or the `Navigator` it lives in) was constructed.
        let list_items: Vec<ListItem> =
            self.store.borrow().items().iter().map(vault_item_to_list_item).collect();

        // `VerticalList` owns the actual row-drawing/scroll/highlight
        // logic; rebuilding one transiently on every render (rather than
        // duplicating that logic here) is cheap and keeps this adapter
        // genuinely thin. `selected`/`focused` are *this* widget's
        // persistent state (survives across renders); `with_selected`/
        // `with_focused` carry them into the transient `VerticalList`.
        VerticalList::new(list_items)
            .with_selected(self.selected)
            .with_focused(self.focused)
            .render(area, target)
    }
}

/// Builds the root screen: a titled, focusable vertical list backed live by
/// `store`.
fn credential_list_screen(store: Rc<RefCell<VaultStore>>) -> Screen {
    let list = StoreBackedCredentialList::new(store);
    Screen::new("Vault", vec![Box::new(list)]).with_hint("Next/Prev  Select  Back")
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
    use embedded_graphics::pixelcolor::{Rgb565, WebColors};
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

        let frame_0 = app.render().pixel(Point::new(2, 18));
        assert_eq!(frame_0, Rgb565::CSS_DARK_SLATE_BLUE, "row 0 should start selected");

        app.handle_input(vec![NavIntent::Next]);
        assert!(app.dirty(), "moving selection should mark the app dirty");

        let frame_1_row_0 = app.render().pixel(Point::new(2, 18));
        assert_ne!(frame_1_row_0, Rgb565::CSS_DARK_SLATE_BLUE, "row 0 should no longer be selected");
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
}
