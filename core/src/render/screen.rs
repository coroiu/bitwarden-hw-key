//! `Screen`: one entry in the `Navigator`'s stack — a title, a set of
//! content widgets stacked vertically in the chrome's content region, and
//! this screen's own focus memory. Salvaged from
//! `simple_gui::document::Document`'s `ViewStackEntry` (title + components
//! + `focused_index`), reimplemented on `embedded-graphics`.

use std::convert::Infallible;

use embedded_graphics::{
    draw_target::DrawTargetExt,
    prelude::{Point, Primitive, Size},
    primitives::{Circle, PrimitiveStyle, Rectangle},
    Drawable,
};
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::FontRenderer;

use crate::input::NavIntent;

use super::chrome::ChromeLayout;
use super::framebuffer::FrameBuffer565;
use super::theme::{font, icon, palette};
use super::widget::{Action, ChromeContribution, ChromeStatus, FocusEvent, Widget};

/// Margin (px) from the title bar's left/right edges to its shield mark /
/// status dot — the "more air" half of Andreas's title-bar tweak (the
/// other half, using `font::icon_1x` instead of `icon_2x` for the mark
/// itself, lives in `theme.rs`).
const TITLE_SIDE_MARGIN: i32 = 6;
/// Gap (px) between adjacent title-bar elements (shield -> title text,
/// readout -> status dot).
const TITLE_ELEMENT_GAP: i32 = 6;
/// Diameter (px) of the title bar's sync-status dot.
const STATUS_DOT_DIAMETER: u32 = 6;
/// Left margin (px) for hint-bar text — bumped from the original `4` per
/// Andreas's "more padding" tweak; the hint font is already the smallest
/// on screen (bead `ai-bitwarden-hw-key-0v8.8`), so it can afford a wider
/// margin than body text without feeling squeezed against the edge.
const HINT_SIDE_MARGIN: i32 = 8;

/// The horizontal pixel footprint `u8g2-fonts`' `render_aligned` would give
/// `text` in `font` — used to right-align/clip chrome elements without
/// hardcoding a per-character pixel width (the thing this bead's title-bar
/// rework is specifically meant to survive the later `u8g2-fonts` font
/// swap without, per the bead description).
fn text_width(font: &FontRenderer, text: &str) -> u32 {
    font.get_rendered_dimensions_aligned(text, Point::zero(), VerticalPosition::Top, HorizontalAlignment::Left)
        .unwrap_or(None)
        .map_or(0, |bbox| bbox.size.width)
}

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

    /// The currently focused widget's [`ChromeContribution`], if any.
    /// Consulting *only* the focused widget (not e.g. merging every
    /// widget's contribution) is deliberate: on every screen this bead
    /// builds there's exactly one widget anyway, and for a future
    /// multi-widget screen "whichever thing has the user's attention
    /// decides what the chrome says" is the same rule per-screen focus
    /// memory already uses for input.
    pub(super) fn chrome_contribution(&self) -> Option<ChromeContribution> {
        self.focused_index.and_then(|index| self.widgets[index].chrome_contribution())
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

    /// Draws the title bar (shield mark, title, position readout, sync
    /// status dot), the content widgets (stacked vertically, sized via
    /// `Widget::measure`), and the hint bar — pulling live overrides from
    /// the focused widget's [`ChromeContribution`] (see
    /// [`Self::chrome_contribution`]) over this screen's static
    /// `title`/`hint` wherever the widget supplies one.
    pub(super) fn render(
        &self,
        chrome: &ChromeLayout,
        target: &mut FrameBuffer565,
    ) -> Result<(), Infallible> {
        chrome.title.into_styled(PrimitiveStyle::with_fill(palette::SURFACE)).draw(target)?;

        // Hairline dividers along the title bar's bottom edge and the hint
        // bar's top edge — the same `palette::DIVIDER` hairline the list
        // rows use between unfocused rows, so chrome and content read as
        // one consistent visual language rather than content borrowing a
        // rule chrome doesn't also follow.
        if chrome.title.size.height > 0 {
            let divider = Rectangle::new(
                Point::new(chrome.title.top_left.x, chrome.title.top_left.y + chrome.title.size.height as i32 - 1),
                Size::new(chrome.title.size.width, 1),
            );
            divider.into_styled(PrimitiveStyle::with_fill(palette::DIVIDER)).draw(target)?;
        }
        if chrome.hint.size.height > 0 {
            let divider = Rectangle::new(chrome.hint.top_left, Size::new(chrome.hint.size.width, 1));
            divider.into_styled(PrimitiveStyle::with_fill(palette::DIVIDER)).draw(target)?;
        }

        let contribution = self.chrome_contribution();
        let title_text = contribution.as_ref().and_then(|c| c.title.as_deref()).unwrap_or(self.title.as_str());
        let readout_text = contribution.as_ref().and_then(|c| c.readout.as_deref());
        let status = contribution.as_ref().and_then(|c| c.status);
        let hint_text = contribution.as_ref().and_then(|c| c.hint.as_deref()).unwrap_or(self.hint.as_str());

        // Vertically centered in the title bar via `VerticalPosition::Center`
        // rather than a hand-picked baseline offset (the "+11" this
        // retires) — `u8g2-fonts` derives the correct baseline from the
        // font's own ascent/descent metrics for us.
        let title_mid_y = chrome.title.top_left.y + chrome.title.size.height as i32 / 2;

        // Shield mark: `icon_1x`, not `icon_2x` — see that accessor's doc
        // comment for why (Andreas's "smaller, more air" tweak).
        let shield_font = font::icon_1x();
        let mut shield_buf = [0_u8; 4];
        let shield_str: &str = icon::SHIELD.encode_utf8(&mut shield_buf);
        let shield_x = chrome.title.top_left.x + TITLE_SIDE_MARGIN;
        let _ = shield_font.render_aligned(
            shield_str,
            Point::new(shield_x, title_mid_y),
            VerticalPosition::Center,
            HorizontalAlignment::Left,
            FontColor::Transparent(palette::BRAND_BRIGHT),
            target,
        );
        let title_text_x = shield_x + text_width(&shield_font, shield_str) as i32 + TITLE_ELEMENT_GAP;

        // Right side, built right-to-left so the status dot and readout
        // can each be omitted independently: status dot first (rightmost),
        // then the readout to its left.
        let mut right_cursor = chrome.title.top_left.x + chrome.title.size.width as i32 - TITLE_SIDE_MARGIN;

        if let Some(status) = status {
            let dot_color = match status {
                ChromeStatus::Success => palette::STATUS_SUCCESS,
                ChromeStatus::Error => palette::STATUS_ERROR,
                ChromeStatus::Neutral => palette::TEXT_SECONDARY,
            };
            let dot_center = Point::new(right_cursor - STATUS_DOT_DIAMETER as i32 / 2, title_mid_y);
            Circle::with_center(dot_center, STATUS_DOT_DIAMETER)
                .into_styled(PrimitiveStyle::with_fill(dot_color))
                .draw(target)?;
            right_cursor -= STATUS_DOT_DIAMETER as i32 + TITLE_ELEMENT_GAP;
        }

        let readout_font = font::title();
        if let Some(readout) = readout_text {
            let _ = readout_font.render_aligned(
                readout,
                Point::new(right_cursor, title_mid_y),
                VerticalPosition::Center,
                HorizontalAlignment::Right,
                FontColor::Transparent(palette::TEXT_SECONDARY),
                target,
            );
            right_cursor -= text_width(&readout_font, readout) as i32 + TITLE_ELEMENT_GAP;
        }

        // Title text: clipped to `[title_text_x, right_cursor)` so a long
        // title can never bleed into the readout/status dot — retiring the
        // old un-clipped single-blob title draw this bead's description
        // calls out.
        let title_clip_width = (right_cursor - title_text_x).max(0) as u32;
        let title_rect = Rectangle::new(
            Point::new(title_text_x, chrome.title.top_left.y),
            Size::new(title_clip_width, chrome.title.size.height),
        );
        let mut title_target = target.clipped(&title_rect);
        let _ = font::title().render_aligned(
            title_text,
            Point::new(title_text_x, title_mid_y),
            VerticalPosition::Center,
            HorizontalAlignment::Left,
            FontColor::Transparent(palette::TEXT_PRIMARY),
            &mut title_target,
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
                hint_text,
                Point::new(chrome.hint.top_left.x + HINT_SIDE_MARGIN, hint_mid_y),
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
