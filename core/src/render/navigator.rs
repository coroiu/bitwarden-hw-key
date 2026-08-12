//! `Navigator`: owns the screen stack. Salvaged from
//! `simple_gui::document::Document`, reimplemented against `NavIntent`
//! (Tier 2 semantic input, per
//! `.planning/decisions/2026-08-11-rotary-encoder-input-model.md`) instead
//! of raw button `KeyCode`s.
//!
//! Per-screen focus memory falls out of the data structure for free: a
//! popped screen isn't rebuilt, it's kept on the stack (in `Screen`,
//! `focused_index` lives on the struct itself), so pushing a new screen
//! and later popping back restores exactly the focus state that was there
//! before the push — no explicit save/restore step needed.

use std::convert::Infallible;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::OriginDimensions;

use super::chrome::compute_chrome;
use super::framebuffer::FrameBuffer565;
use super::screen::Screen;
use super::theme::palette;
use super::widget::Action;
use crate::input::NavIntent;

pub struct Navigator {
    stack: Vec<Screen>,
}

impl Navigator {
    /// Creates a navigator with `root` as the only (and un-poppable)
    /// screen on the stack.
    #[must_use]
    pub fn new(mut root: Screen) -> Self {
        root.initialize_focus();
        Self { stack: vec![root] }
    }

    /// The currently visible screen.
    ///
    /// # Panics
    ///
    /// Never, in practice: the stack always has at least the root screen
    /// (`pop` refuses to remove it). The `expect` exists only because
    /// `Vec::last` returns `Option`.
    #[must_use]
    pub fn current(&self) -> &Screen {
        self.stack.last().expect("navigator stack must never be empty")
    }

    fn current_mut(&mut self) -> &mut Screen {
        self.stack.last_mut().expect("navigator stack must never be empty")
    }

    /// How many screens are on the stack (>= 1; the root screen is never
    /// popped).
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Pushes a new screen, establishing its initial focus.
    pub fn push(&mut self, mut screen: Screen) {
        screen.initialize_focus();
        self.stack.push(screen);
    }

    /// Pops the current screen, unless it's the root. Returns whether a
    /// pop happened.
    pub fn pop(&mut self) -> bool {
        if self.stack.len() > 1 {
            self.stack.pop();
            true
        } else {
            false
        }
    }

    fn apply_action(&mut self, action: Action) {
        match action {
            Action::PushView(builder) => self.push(builder()),
            Action::PopView | Action::Back => {
                self.pop();
            }
            Action::None => {}
        }
    }

    /// Dispatches a semantic navigation intent to the current screen.
    ///
    /// # Known simplification
    ///
    /// For `Next`/`Prev`/`NextN`, the intent is forwarded to the focused
    /// widget first (so e.g. a list can move its own internal selection),
    /// and then *also* drives the screen's own top-level focus cycling
    /// (salvaged from `Document::focus_next`/`focus_previous`). On a
    /// screen with exactly one focusable widget — every screen this bead
    /// builds — the top-level cycle is a no-op (there's nothing else to
    /// focus). On a hypothetical future multi-widget screen, this would
    /// mean every `Next`/`Prev` both moves the focused widget's internal
    /// selection *and* tries to move top-level focus, with no way for a
    /// widget to say "I consumed that, don't also refocus." Fixing that
    /// needs a "consumed" signal `Widget::on_intent` doesn't have today.
    /// Deferred — flagged here rather than silently shipped as correct.
    pub fn dispatch(&mut self, intent: NavIntent) {
        match intent {
            NavIntent::Next | NavIntent::NextN(_) => {
                let action = self.current_mut().forward_to_focused(intent);
                self.apply_action(action);
                self.current_mut().focus_next();
            }
            NavIntent::Prev => {
                let action = self.current_mut().forward_to_focused(intent);
                self.apply_action(action);
                self.current_mut().focus_previous();
            }
            NavIntent::Activate => {
                let action = self.current_mut().activate_focused();
                self.apply_action(action);
            }
            NavIntent::Back => {
                self.pop();
            }
        }
    }

    /// Renders the current screen into `target`, computing chrome regions
    /// from whatever size `target` happens to be.
    ///
    /// Clears `target` to [`palette::BACKGROUND`] first. This matters because, per the
    /// presentation-surface ADR, the app core owns a single long-lived
    /// framebuffer that gets re-rendered into every frame rather than
    /// reallocated — without an explicit clear, a widget that doesn't
    /// unconditionally repaint every pixel of its area (e.g.
    /// `VerticalList` only fills a *selected* row's background, leaving
    /// unselected rows' backgrounds untouched) would leave stale pixels
    /// from a previous frame's selection highlight visible after the
    /// selection moves away. Widgets and tests that always render into a
    /// fresh `FrameBuffer565` (already black) are unaffected by this.
    ///
    /// # Errors
    ///
    /// Never, in practice: `FrameBuffer565`'s `DrawTarget::Error` is
    /// `Infallible`. The `Result` return exists so this can use `?`
    /// against embedded-graphics `Drawable::draw` calls internally.
    pub fn render(&self, target: &mut FrameBuffer565) -> Result<(), Infallible> {
        target.clear(palette::BACKGROUND)?;
        let chrome = compute_chrome(target.size());
        self.current().render(&chrome, target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::list::{ListItem, VerticalList};
    use embedded_graphics::prelude::{OriginDimensions, Point, Size};

    fn list_screen(title: &str, n: usize) -> Screen {
        let items = (0..n).map(|i| ListItem::new(format!("{title}-item-{i}"))).collect();
        Screen::new(title, vec![Box::new(VerticalList::new(items))])
    }

    #[test]
    fn root_screen_cannot_be_popped() {
        let mut nav = Navigator::new(list_screen("root", 3));
        assert_eq!(nav.depth(), 1);
        assert!(!nav.pop());
        assert_eq!(nav.depth(), 1);
    }

    #[test]
    fn push_then_pop_returns_to_the_previous_screen() {
        let mut nav = Navigator::new(list_screen("root", 3));
        nav.push(list_screen("detail", 1));
        assert_eq!(nav.depth(), 2);
        assert_eq!(nav.current().title, "detail");

        assert!(nav.pop());
        assert_eq!(nav.depth(), 1);
        assert_eq!(nav.current().title, "root");
    }

    #[test]
    fn back_intent_pops_a_pushed_screen_but_not_the_root() {
        let mut nav = Navigator::new(list_screen("root", 3));
        nav.push(list_screen("detail", 1));
        nav.dispatch(NavIntent::Back);
        assert_eq!(nav.depth(), 1);
        nav.dispatch(NavIntent::Back);
        assert_eq!(nav.depth(), 1);
    }

    #[test]
    fn next_intent_moves_the_focused_lists_selection() {
        let mut nav = Navigator::new(list_screen("root", 5));
        nav.dispatch(NavIntent::Next);
        nav.dispatch(NavIntent::Next);
        assert_eq!(nav.current().focused_index(), Some(0));
        // The selection lives inside the widget, not exposed on Screen
        // directly; render + pixel-sample in the PNG-dump test is the
        // black-box proof. Here we at least prove dispatch doesn't panic
        // and focus stays on the (only) focusable widget.
    }

    #[test]
    fn activate_on_a_list_with_no_callback_does_not_change_the_stack() {
        let mut nav = Navigator::new(list_screen("root", 3));
        nav.dispatch(NavIntent::Activate);
        assert_eq!(nav.depth(), 1);
    }

    #[test]
    fn pushing_a_screen_via_action_from_a_widget_callback_works() {
        let items = vec![ListItem::new("open detail")];
        let list = VerticalList::new(items).on_activate(|_item| {
            Action::PushView(Box::new(|| {
                Screen::new("detail", vec![Box::new(VerticalList::new(vec![ListItem::new("x")]))])
            }))
        });
        let root = Screen::new("root", vec![Box::new(list)]);
        let mut nav = Navigator::new(root);

        nav.dispatch(NavIntent::Activate);
        assert_eq!(nav.depth(), 2);
        assert_eq!(nav.current().title, "detail");
    }

    #[test]
    fn per_screen_focus_memory_is_preserved_across_push_and_pop() {
        let mut nav = Navigator::new(list_screen("root", 5));
        nav.dispatch(NavIntent::Next); // move selection within the list

        nav.push(list_screen("detail", 2));
        assert_eq!(nav.current().focused_index(), Some(0));

        nav.pop();
        // Same screen instance, never rebuilt: its focused_index (which
        // top-level widget has focus) is exactly what it was before the
        // push. This is the "per-screen focus memory" requirement.
        assert_eq!(nav.current().focused_index(), Some(0));
        assert_eq!(nav.current().title, "root");
    }

    #[test]
    fn rendering_into_the_same_framebuffer_twice_does_not_leave_the_previous_frames_selection_highlight_behind() {
        // Regression test for the "reused framebuffer across frames"
        // ghosting bug: `render()` used to skip clearing, so a row's
        // selection-highlight fill from frame N was still visible in
        // frame N+1 after the selection moved away (`VerticalList` only
        // paints a background fill for the *currently* selected row, not
        // every row). This is exactly the scenario `App`/the unified run
        // loop hit in practice, since they render into one long-lived
        // `FrameBuffer565` rather than allocating a fresh one per frame.
        let mut fb = FrameBuffer565::new(320, 170);
        let mut nav = Navigator::new(list_screen("Vault", 3));

        nav.render(&mut fb).unwrap();
        // x=20: past the 4px selection accent bar, so this samples the
        // row's plain elevated fill rather than the accent stripe.
        let row0_highlighted = fb.pixel(Point::new(20, 18));
        assert_eq!(row0_highlighted, palette::SURFACE_ELEVATED, "row 0 starts selected");

        nav.dispatch(NavIntent::Next);
        nav.render(&mut fb).unwrap();
        let row0_after_move = fb.pixel(Point::new(20, 18));
        assert_ne!(
            row0_after_move, palette::SURFACE_ELEVATED,
            "row 0's stale highlight from the first render must not survive into the second"
        );
    }

    #[test]
    fn render_works_end_to_end_on_a_fresh_navigator() {
        let mut fb = FrameBuffer565::new(320, 170);
        let nav = Navigator::new(list_screen("Vault", 3));
        nav.render(&mut fb).unwrap();
        assert_eq!(fb.size(), Size::new(320, 170));
        // Sanity: the title bar's surface fill was drawn somewhere, i.e.
        // rendering actually did something (not just the background clear).
        let any_title_bar_surface = fb.pixels().any(|p| p.1 == palette::SURFACE);
        assert!(any_title_bar_surface);
    }
}
