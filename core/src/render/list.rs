//! A scrolling vertical list widget: the one content widget this bead
//! needs to prove the render core end-to-end (see the ADR's component
//! library list — `VerticalMenu` is the salvaged concept, reimplemented on
//! `embedded-graphics` instead of the retired RGBA rasterizer).
//!
//! [`ListItem`] is a display-only row shape, deliberately not
//! `VaultItem` — the widget layer shouldn't know about the credential
//! domain model. Call sites (e.g. a future `CredentialListView`) map
//! `VaultItem -> ListItem`.

use std::convert::Infallible;

use embedded_graphics::{
    draw_target::DrawTargetExt,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::{Rgb565, WebColors},
    prelude::{Point, Primitive, RgbColor, Size},
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
    Drawable,
};

use crate::input::NavIntent;

use super::framebuffer::FrameBuffer565;
use super::widget::{Action, FocusEvent, Widget};

/// A single displayable row. Display-only: no identifiers, no domain
/// fields — just what a `VerticalList` needs to draw a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub label: String,
    pub sublabel: Option<String>,
}

impl ListItem {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            sublabel: None,
        }
    }

    #[must_use]
    pub fn with_sublabel(mut self, sublabel: impl Into<String>) -> Self {
        self.sublabel = Some(sublabel.into());
        self
    }
}

/// `MonoTextStyle`/`Text` position the glyph at its *baseline* by default
/// (`Baseline::Alphabetic`), not its top-left. For `FONT_6X10`,
/// `baseline == 7`: the glyph box is 10px tall, split 7px above the
/// baseline (ascent) and `10 - 7 = 3px` below it (descent). Getting this
/// wrong is exactly what caused the row-overflow bug this module was
/// fixed for (see `ROW_HEIGHT`'s doc comment) — computed from the font's
/// own metrics here instead of re-deriving/hardcoding it at each call
/// site.
const FONT_ASCENT: u32 = FONT_6X10.baseline;
const FONT_DESCENT: u32 = FONT_6X10.character_size.height - FONT_6X10.baseline;

/// Vertical padding above the label and below the sublabel, and the gap
/// between the two lines.
const ROW_PADDING: u32 = 1;
const LINE_GAP: u32 = 1;

/// Pixel height of a single row (padding + label line + gap + sublabel
/// line + padding). Fixed, like the chrome bar heights in `chrome.rs` — a
/// pixel budget, not a screen-resolution assumption.
///
/// Derived from `FONT_6X10`'s real metrics so the two text baselines
/// (`label_baseline_offset`/`sublabel_baseline_offset` below) always fit
/// entirely inside `[0, ROW_HEIGHT)`. A previous version hardcoded `20`
/// with baselines at `+12`/`+21`, which put the sublabel's descent 4px
/// past the row's bottom edge — visually, the *next* row's selection
/// highlight (opaque, drawn on top) ate the tail of this row's sublabel.
/// `render_png_dump.rs`'s `text_never_bleeds_past_a_rows_bottom_padding`
/// test guards against this regressing.
pub const ROW_HEIGHT: u32 =
    ROW_PADDING + FONT_ASCENT + FONT_DESCENT + LINE_GAP + FONT_ASCENT + FONT_DESCENT + ROW_PADDING;

/// Baseline y-offset (from a row's top edge) for the label line.
fn label_baseline_offset() -> i32 {
    (ROW_PADDING + FONT_ASCENT) as i32
}

/// Baseline y-offset (from a row's top edge) for the sublabel line —
/// directly below the label line's descent, plus `LINE_GAP`.
fn sublabel_baseline_offset() -> i32 {
    label_baseline_offset() + (FONT_DESCENT + LINE_GAP + FONT_ASCENT) as i32
}

/// Callback invoked with the selected item on activation. Named as a type
/// alias purely to keep `VerticalList`'s field type readable.
type OnActivate = Box<dyn Fn(&ListItem) -> Action>;

/// A focusable, scrollable vertical list of [`ListItem`]s. Moves its
/// internal selection in response to `NavIntent::{Next,Prev,NextN}` via
/// `Widget::on_intent`, auto-scrolling to keep the selection visible; fires
/// its `on_activate` callback (if any) when activated while focused.
pub struct VerticalList {
    items: Vec<ListItem>,
    selected: usize,
    focused: bool,
    on_activate: Option<OnActivate>,
}

impl VerticalList {
    #[must_use]
    pub fn new(items: Vec<ListItem>) -> Self {
        Self {
            items,
            selected: 0,
            focused: false,
            on_activate: None,
        }
    }

    /// Registers a callback invoked with the selected `ListItem` when the
    /// list is activated (encoder short press / `NavIntent::Activate`)
    /// while focused. Typically used to return `Action::PushView(...)`.
    #[must_use]
    pub fn on_activate(mut self, callback: impl Fn(&ListItem) -> Action + 'static) -> Self {
        self.on_activate = Some(Box::new(callback));
        self
    }

    /// Sets the initially selected row, clamped to the item list's bounds.
    ///
    /// Used by store-backed widgets (see `App`'s root credential list) that
    /// rebuild a fresh `VerticalList` from live data on every render call
    /// but need to carry forward the persistent selection they track
    /// themselves — `VerticalList::new` alone always starts at `0`.
    #[must_use]
    pub fn with_selected(mut self, selected: usize) -> Self {
        self.selected = selected.min(self.items.len().saturating_sub(1));
        self
    }

    /// Sets the initial focus-highlight state. Same rationale as
    /// `with_selected`: a caller that rebuilds this widget fresh per render
    /// still needs to carry forward focus state it tracks itself.
    #[must_use]
    pub fn with_focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    fn move_selection(&mut self, delta: i32) {
        if self.items.is_empty() {
            return;
        }
        let len = self.items.len() as i32;
        let next = (self.selected as i32 + delta).clamp(0, len - 1);
        self.selected = next as usize;
    }

    /// Scroll offset (in pixels) that keeps the selected row within
    /// `[scroll, scroll + viewport_height)`, per the salvaged
    /// `VerticalMenu::auto_scroll` concept — reimplemented as a pure
    /// function of `selected` and the viewport height on offer at render
    /// time, rather than mutable state threaded through `on_intent`
    /// (`Widget::render` takes `&self`, so there's nowhere to persist a
    /// running scroll offset across frames without interior mutability;
    /// a pure recomputation avoids needing it).
    fn scroll_for_viewport(&self, viewport_height: u32) -> u32 {
        if viewport_height == 0 {
            return 0;
        }
        let selected_bottom = (self.selected as u32 + 1) * ROW_HEIGHT;
        selected_bottom.saturating_sub(viewport_height)
    }
}

impl Widget for VerticalList {
    fn measure(&self, constraints: Size) -> Size {
        // A list fills whatever vertical space its screen gives it; it
        // manages overflow itself via scrolling, not by requesting more
        // height than is on offer.
        constraints
    }

    fn is_focusable(&self) -> bool {
        !self.items.is_empty()
    }

    fn on_focus(&mut self, event: FocusEvent) -> Action {
        match event {
            FocusEvent::Gained => {
                self.focused = true;
                Action::None
            }
            FocusEvent::Lost => {
                self.focused = false;
                Action::None
            }
            FocusEvent::Activated => {
                if let (Some(callback), Some(item)) =
                    (&self.on_activate, self.items.get(self.selected))
                {
                    callback(item)
                } else {
                    Action::None
                }
            }
        }
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
        // Real clipping (DrawTargetExt::clipped), not the retired
        // character-skip marquee: anything a row draws outside `area` —
        // an over-long label, a row scrolled partway off the top/bottom —
        // is simply dropped by the clip, not manually truncated by the
        // widget.
        let mut clipped = target.clipped(&area);

        let scroll = self.scroll_for_viewport(area.size.height);
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        let sub_style = MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_GRAY);
        let selected_fill = PrimitiveStyle::with_fill(Rgb565::CSS_DARK_SLATE_BLUE);

        for (index, item) in self.items.iter().enumerate() {
            let row_top =
                area.top_left.y + (index as u32 * ROW_HEIGHT) as i32 - scroll as i32;

            // Skip rows fully outside the viewport — a cheap early-out;
            // `clipped` would drop their pixels anyway, but there's no
            // point building draw commands for off-screen rows.
            if row_top + ROW_HEIGHT as i32 <= area.top_left.y
                || row_top >= area.top_left.y + area.size.height as i32
            {
                continue;
            }

            let row_rect = Rectangle::new(
                Point::new(area.top_left.x, row_top),
                Size::new(area.size.width, ROW_HEIGHT),
            );

            if self.focused && index == self.selected {
                row_rect.into_styled(selected_fill).draw(&mut clipped)?;
            }

            Text::new(
                &item.label,
                Point::new(area.top_left.x + 4, row_top + label_baseline_offset()),
                text_style,
            )
            .draw(&mut clipped)?;

            if let Some(sublabel) = &item.sublabel {
                Text::new(
                    sublabel,
                    Point::new(area.top_left.x + 4, row_top + sublabel_baseline_offset()),
                    sub_style,
                )
                .draw(&mut clipped)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(n: usize) -> Vec<ListItem> {
        (0..n).map(|i| ListItem::new(format!("item-{i}"))).collect()
    }

    #[test]
    fn empty_list_is_not_focusable() {
        let list = VerticalList::new(vec![]);
        assert!(!list.is_focusable());
    }

    #[test]
    fn non_empty_list_is_focusable() {
        let list = VerticalList::new(items(3));
        assert!(list.is_focusable());
    }

    #[test]
    fn next_and_prev_move_selection_and_clamp_at_the_ends() {
        let mut list = VerticalList::new(items(3));
        assert_eq!(list.selected_index(), 0);

        list.on_intent(NavIntent::Prev); // clamp below zero
        assert_eq!(list.selected_index(), 0);

        list.on_intent(NavIntent::Next);
        assert_eq!(list.selected_index(), 1);
        list.on_intent(NavIntent::Next);
        assert_eq!(list.selected_index(), 2);
        list.on_intent(NavIntent::Next); // clamp at the end
        assert_eq!(list.selected_index(), 2);

        list.on_intent(NavIntent::Prev);
        assert_eq!(list.selected_index(), 1);
    }

    #[test]
    fn next_n_jumps_and_clamps() {
        let mut list = VerticalList::new(items(10));
        list.on_intent(NavIntent::NextN(4));
        assert_eq!(list.selected_index(), 4);
        list.on_intent(NavIntent::NextN(20));
        assert_eq!(list.selected_index(), 9);
    }

    #[test]
    fn intent_on_empty_list_does_not_panic() {
        let mut list = VerticalList::new(vec![]);
        let action = list.on_intent(NavIntent::Next);
        assert!(matches!(action, Action::None));
    }

    #[test]
    fn activate_without_callback_is_a_noop() {
        let mut list = VerticalList::new(items(2));
        let action = list.on_focus(FocusEvent::Activated);
        assert!(matches!(action, Action::None));
    }

    #[test]
    fn activate_with_callback_invokes_it_with_the_selected_item() {
        let mut list = VerticalList::new(items(3)).on_activate(|item| {
            assert_eq!(item.label, "item-1");
            Action::PopView
        });
        list.on_intent(NavIntent::Next); // select index 1
        let action = list.on_focus(FocusEvent::Activated);
        assert!(matches!(action, Action::PopView));
    }

    #[test]
    fn gained_and_lost_toggle_is_focused() {
        let mut list = VerticalList::new(items(1));
        assert!(!list.is_focused());
        list.on_focus(FocusEvent::Gained);
        assert!(list.is_focused());
        list.on_focus(FocusEvent::Lost);
        assert!(!list.is_focused());
    }

    #[test]
    fn scroll_for_viewport_keeps_the_selection_visible() {
        let mut list = VerticalList::new(items(10));
        let viewport_height = 3 * ROW_HEIGHT; // fits 3 rows

        // Selection within the first screenful: no scroll needed.
        assert_eq!(list.scroll_for_viewport(viewport_height), 0);

        // Selecting row 4 (0-indexed) means rows 0-3 no longer all fit;
        // scroll should reveal it as the bottom-most visible row.
        for _ in 0..4 {
            list.on_intent(NavIntent::Next);
        }
        let scroll = list.scroll_for_viewport(viewport_height);
        assert_eq!(scroll, (5 * ROW_HEIGHT) - viewport_height);

        // The selected row's own top and bottom must both land inside
        // [scroll, scroll + viewport_height).
        let selected_top = 4 * ROW_HEIGHT;
        let selected_bottom = selected_top + ROW_HEIGHT;
        assert!(selected_top >= scroll);
        assert!(selected_bottom <= scroll + viewport_height);
    }

    #[test]
    fn render_does_not_panic_for_a_small_viewport_with_more_rows_than_fit() {
        let list = VerticalList::new(items(50));
        let mut fb = FrameBuffer565::new(64, 40);
        let area = Rectangle::new(Point::new(0, 0), Size::new(64, 40));
        list.render(area, &mut fb).unwrap();
    }
}
