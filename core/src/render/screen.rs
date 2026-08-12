//! `Screen`: one entry in the `Navigator`'s stack — a title, a set of
//! content widgets stacked vertically in the chrome's content region, and
//! this screen's own focus memory. Salvaged from
//! `simple_gui::document::Document`'s `ViewStackEntry` (title + components
//! + `focused_index`), reimplemented on `embedded-graphics`.

use std::convert::Infallible;

use embedded_graphics::{
    prelude::{Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
    Drawable,
};
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

use crate::input::NavIntent;

use super::chrome::ChromeLayout;
use super::framebuffer::FrameBuffer565;
use super::theme::{font, palette};
use super::widget::{Action, FocusEvent, Widget};

pub struct Screen {
    pub title: String,
    /// Static hint text drawn in the hint bar, e.g. control legends. Not a
    /// `Widget` — chrome furniture is intentionally simpler than the
    /// content widget model.
    pub hint: String,
    widgets: Vec<Box<dyn Widget>>,
    focused_index: Option<usize>,
}

impl Screen {
    #[must_use]
    pub fn new(title: impl Into<String>, widgets: Vec<Box<dyn Widget>>) -> Self {
        Self {
            title: title.into(),
            hint: String::new(),
            widgets,
            focused_index: None,
        }
    }

    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    #[must_use]
    pub fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    #[must_use]
    pub fn widgets(&self) -> &[Box<dyn Widget>] {
        &self.widgets
    }

    /// Focuses the first focusable widget, if none is focused yet. Called
    /// when a screen is first pushed onto the stack. A no-op if focus was
    /// already established (which is how per-screen focus memory works:
    /// a screen re-visited via `pop` still has its old `focused_index`,
    /// so this does nothing and the old focus is preserved).
    pub(super) fn initialize_focus(&mut self) {
        if self.focused_index.is_some() {
            return;
        }
        for index in 0..self.widgets.len() {
            if self.widgets[index].is_focusable() {
                self.set_focus(Some(index));
                return;
            }
        }
    }

    fn set_focus(&mut self, new_index: Option<usize>) {
        if self.focused_index == new_index {
            return;
        }
        if let Some(old) = self.focused_index {
            self.widgets[old].on_focus(FocusEvent::Lost);
        }
        if let Some(new) = new_index {
            self.widgets[new].on_focus(FocusEvent::Gained);
        }
        self.focused_index = new_index;
    }

    /// Moves top-level focus to the next focusable widget, wrapping
    /// around. Salvaged from `Document::focus_next`.
    pub(super) fn focus_next(&mut self) {
        if self.widgets.is_empty() {
            return;
        }
        let len = self.widgets.len();
        let start = self.focused_index.map_or(0, |i| (i + 1) % len);
        for step in 0..len {
            let index = (start + step) % len;
            if self.widgets[index].is_focusable() {
                self.set_focus(Some(index));
                return;
            }
        }
    }

    /// Moves top-level focus to the previous focusable widget, wrapping
    /// around. Salvaged from `Document::focus_previous`.
    pub(super) fn focus_previous(&mut self) {
        if self.widgets.is_empty() {
            return;
        }
        let len = self.widgets.len();
        let start = self.focused_index.unwrap_or(0);
        for step in 0..len {
            let index = (start + len - 1 - step) % len;
            if self.widgets[index].is_focusable() {
                self.set_focus(Some(index));
                return;
            }
        }
    }

    /// Forwards a navigation intent to the currently focused widget (if
    /// any), for `Next`/`Prev`/`NextN` — see `Navigator::dispatch` for how
    /// this interacts with `focus_next`/`focus_previous`.
    pub(super) fn forward_to_focused(&mut self, intent: NavIntent) -> Action {
        match self.focused_index {
            Some(index) => self.widgets[index].on_intent(intent),
            None => Action::None,
        }
    }

    /// Activates the currently focused widget (`NavIntent::Activate`).
    pub(super) fn activate_focused(&mut self) -> Action {
        match self.focused_index {
            Some(index) => self.widgets[index].on_focus(FocusEvent::Activated),
            None => Action::None,
        }
    }

    /// Draws the title bar, the content widgets (stacked vertically,
    /// sized via `Widget::measure`), and the hint bar.
    pub(super) fn render(
        &self,
        chrome: &ChromeLayout,
        target: &mut FrameBuffer565,
    ) -> Result<(), Infallible> {
        chrome.title.into_styled(PrimitiveStyle::with_fill(palette::SURFACE)).draw(target)?;

        // Vertically centered in the title bar via `VerticalPosition::Center`
        // rather than a hand-picked baseline offset (the "+11" this
        // retires) — `u8g2-fonts` derives the correct baseline from the
        // font's own ascent/descent metrics for us.
        let title_mid_y = chrome.title.top_left.y + chrome.title.size.height as i32 / 2;
        let _ = font::title().render_aligned(
            self.title.as_str(),
            Point::new(chrome.title.top_left.x + 4, title_mid_y),
            VerticalPosition::Center,
            HorizontalAlignment::Left,
            FontColor::Transparent(palette::TEXT_PRIMARY),
            target,
        );

        let mut y = chrome.content.top_left.y;
        let bottom = chrome.content.top_left.y + chrome.content.size.height as i32;
        for widget in &self.widgets {
            if y >= bottom {
                break;
            }
            let available = Size::new(chrome.content.size.width, (bottom - y) as u32);
            let requested = widget.measure(available);
            let height = requested.height.min(available.height);

            let area = Rectangle::new(Point::new(chrome.content.top_left.x, y), Size::new(chrome.content.size.width, height));
            widget.render(area, target)?;
            y += height as i32;
        }

        if chrome.hint.size.height > 0 {
            let hint_mid_y = chrome.hint.top_left.y + chrome.hint.size.height as i32 / 2;
            let _ = font::hint().render_aligned(
                self.hint.as_str(),
                Point::new(chrome.hint.top_left.x + 4, hint_mid_y),
                VerticalPosition::Center,
                HorizontalAlignment::Left,
                FontColor::Transparent(palette::TEXT_SECONDARY),
                target,
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::list::{ListItem, VerticalList};

    fn list_screen(n: usize) -> Screen {
        let items = (0..n).map(|i| ListItem::new(format!("item-{i}"))).collect();
        Screen::new("Test", vec![Box::new(VerticalList::new(items))])
    }

    #[test]
    fn initialize_focus_focuses_the_first_focusable_widget() {
        let mut screen = list_screen(3);
        assert_eq!(screen.focused_index(), None);
        screen.initialize_focus();
        assert_eq!(screen.focused_index(), Some(0));
    }

    #[test]
    fn initialize_focus_on_a_screen_with_no_focusable_widgets_is_a_noop() {
        let mut screen = list_screen(0);
        screen.initialize_focus();
        assert_eq!(screen.focused_index(), None);
    }

    #[test]
    fn focus_next_wraps_around_a_single_widget() {
        let mut screen = list_screen(3);
        screen.initialize_focus();
        screen.focus_next();
        // Only one focusable widget on this screen: wraps back to itself.
        assert_eq!(screen.focused_index(), Some(0));
    }

    #[test]
    fn render_does_not_panic_and_writes_into_the_provided_chrome_regions() {
        let mut screen = list_screen(5);
        screen.initialize_focus();
        let chrome = super::super::chrome::compute_chrome(Size::new(320, 170));
        let mut fb = FrameBuffer565::new(320, 170);
        screen.render(&chrome, &mut fb).unwrap();
        // Title bar was filled with its background color.
        assert_eq!(fb.pixel(Point::new(0, 0)), palette::SURFACE);
    }
}
