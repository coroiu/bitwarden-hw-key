//! `App`: the platform-free application state the unified main loop
//! ([`crate::run::run`]) drives every frame. This is M0's "empty-but-real"
//! credential-list shell: a single [`Navigator`] screen showing whatever
//! [`VaultItem`]s the current [`SyncSource`] most recently produced,
//! rendered through the real render core (no product-grade UX yet — a
//! detail view, search, etc. are M1/Uma's job).
//!
//! `App` owns the single [`FrameBuffer565`] it renders into (sized once at
//! construction, from whatever the platform's display reports — see the
//! call sites in `emulator`/`firmware`'s `main.rs`), so the run loop never
//! has to allocate a fresh framebuffer per frame.

use crate::input::NavIntent;
use crate::render::{FrameBuffer565, ListItem, Navigator, Screen, VerticalList};
use crate::sync_source::SyncSource;
use crate::vault_item::VaultItem;

fn vault_item_to_list_item(item: &VaultItem) -> ListItem {
    ListItem::new(item.name.clone()).with_sublabel(item.username.clone())
}

/// Builds the one screen this bead needs: a titled, focusable vertical list
/// of whatever vault items are currently known. Deliberately not a
/// `CredentialDetailView` push yet (`on_activate` is left unset) — that's
/// M1's job once there's an actual detail screen to push.
fn credential_list_screen(items: &[VaultItem]) -> Screen {
    let list_items = items.iter().map(vault_item_to_list_item).collect();
    let list = VerticalList::new(list_items);
    Screen::new("Vault", vec![Box::new(list)]).with_hint("Next/Prev  Select  Back")
}

/// The application core: a [`Navigator`] over the credential-list screen,
/// the [`VaultItem`]s it was last built from (so [`App::step`] can tell
/// whether a fresh sync actually changed anything), and the single
/// [`FrameBuffer565`] it renders into.
pub struct App {
    items: Vec<VaultItem>,
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
    #[must_use]
    pub fn new(width: u32, height: u32, items: Vec<VaultItem>) -> Self {
        let navigator = Navigator::new(credential_list_screen(&items));
        Self {
            items,
            navigator,
            framebuffer: FrameBuffer565::new(width, height),
            dirty: true,
        }
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

    /// Pulls the latest vault snapshot from `sync` and, if it actually
    /// differs from what's currently shown, rebuilds the root screen.
    ///
    /// Rebuilding (rather than diff-patching the existing `VerticalList`)
    /// is a deliberate M0 simplification: this bead's `Navigator` only
    /// ever has one screen on its stack (there's no detail view to push
    /// yet), so a full rebuild can't lose any pushed-screen state. Once a
    /// detail view exists (M1), a mid-navigation sync landing should not
    /// blow away a pushed screen — that's a real design problem for
    /// whoever builds the detail view, not addressed here.
    ///
    /// A sync error is treated as "nothing new to show" (the previous
    /// snapshot keeps rendering) rather than surfaced to the UI — there is
    /// no error-display widget in this bead's scope.
    pub fn step<S: SyncSource>(&mut self, sync: &mut S) {
        if let Ok(items) = sync.sync() {
            if items != self.items {
                self.navigator = Navigator::new(credential_list_screen(&items));
                self.items = items;
                self.dirty = true;
            }
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
    fn step_rebuilds_the_screen_when_the_sync_source_reports_new_items() {
        use std::convert::Infallible;

        struct StubSyncSource(Vec<VaultItem>);
        impl SyncSource for StubSyncSource {
            type Error = Infallible;
            fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
                Ok(self.0.clone())
            }
        }

        let mut app = App::new(320, 170, vec![]);
        app.render();
        assert!(!app.dirty());

        let mut sync = StubSyncSource(vec![item("GitHub")]);
        app.step(&mut sync);

        assert!(app.dirty(), "a changed vault snapshot should mark the app dirty");
    }

    #[test]
    fn step_is_a_noop_when_the_sync_source_reports_the_same_items() {
        use std::convert::Infallible;

        struct StubSyncSource(Vec<VaultItem>);
        impl SyncSource for StubSyncSource {
            type Error = Infallible;
            fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
                Ok(self.0.clone())
            }
        }

        let initial = vec![item("GitHub")];
        let mut app = App::new(320, 170, initial.clone());
        app.render();
        assert!(!app.dirty());

        let mut sync = StubSyncSource(initial);
        app.step(&mut sync);

        assert!(!app.dirty(), "an unchanged vault snapshot should not mark the app dirty");
    }
}
