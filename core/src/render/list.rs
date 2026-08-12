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
    draw_target::{DrawTarget, DrawTargetExt},
    pixelcolor::Rgb565,
    prelude::{Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
    Drawable,
};
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

use crate::input::NavIntent;

use super::framebuffer::FrameBuffer565;
use super::theme::{self, font, icon, palette};
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

/// Vertical padding above the name line and below the username line, and
/// the gap between the two lines.
///
/// Bumped from the original `1`/`1` (bead `ai-bitwarden-hw-key-0v8.8`'s
/// value, tuned only to fit the new theme fonts without overflow) per
/// Andreas's explicit design-review tweak: the approved mockup read as
/// cramped, both between a row's own name/username lines and between
/// consecutive rows (the bottom padding of row N plus the top padding of
/// row N+1 is what a user perceives as "space between rows"). Chosen to
/// visibly loosen both without shrinking the list to fewer than ~3 full
/// rows + a scroll peek on the 320x170 panel — see `ROW_HEIGHT`'s doc
/// comment for the resulting row budget.
const ROW_PADDING: i32 = 3;
const LINE_GAP: i32 = 2;

/// Worst-case pixel footprint (leading + ink height, including
/// descenders) of a single line rendered with [`font::name`]/
/// [`font::username`] from a `VerticalPosition::Top`-anchored position —
/// i.e. how much vertical room a `render_aligned(.., VerticalPosition::Top,
/// ..)` call at row-relative y=0 actually occupies in the worst case.
///
/// These are measured constants, not something `u8g2-fonts` can compute at
/// compile time (`FontRenderer::get_rendered_dimensions*` take `&self` and
/// aren't `const fn`) — derived once via a throwaway probe
/// (`core/examples/dim_probe.rs`, run manually, not part of the build)
/// against `"gjpqy"` (all five ASCII descenders) rendered in each font,
/// then hardcoded here the same way the retired `FONT_ASCENT`/
/// `FONT_DESCENT` constants were hardcoded from `FONT_6X10`'s metrics.
///
/// Using the worst case (not the specific credential name/username being
/// rendered) is deliberate: this guards every possible name/username the
/// same way `ROW_HEIGHT`'s previous, font-metric-derived formula did,
/// rather than only the non-descender demo strings the design-review
/// spike's mockup happened to use (its "+2/+15" row offsets were tuned
/// against `"GitHub"`/`"Amazon Web Services"`-style names with no
/// descenders, which this bead's real credential data can't guarantee).
const NAME_LINE_FOOTPRINT: i32 = 17;
const USERNAME_LINE_FOOTPRINT: i32 = 15;

/// Pixel height of a single row (padding + name line + gap + username
/// line + padding). Fixed, like the chrome bar heights in `chrome.rs` — a
/// pixel budget, not a screen-resolution assumption.
///
/// Grew from `FONT_6X10`'s 23px to accommodate the larger, proportional
/// `helvB12`/`helvR10` theme fonts (bead `ai-bitwarden-hw-key-0v8.8`), then
/// grew again (35px -> 40px) for `ROW_PADDING`/`LINE_GAP`'s bead-0v8.5
/// "more vertical air" bump. On the 320x170 panel this settles at 3 full
/// rows visible plus a partial fourth ("a scroll peek") in the content
/// area below the title/hint bars, rather than the tighter 4-rows-plus-a-
/// sliver the original, denser padding gave — the deliberate trade-off
/// Andreas asked for. Still derived so the two lines' worst-case ink
/// always fits entirely inside `[0, ROW_HEIGHT)`, the same guarantee the
/// original `ROW_HEIGHT` doc comment describes and `render_png_dump.rs`'s
/// `text_never_bleeds_past_a_rows_bottom_padding` test still enforces.
pub const ROW_HEIGHT: u32 =
    (ROW_PADDING + NAME_LINE_FOOTPRINT + LINE_GAP + USERNAME_LINE_FOOTPRINT + ROW_PADDING) as u32;

/// Left margin (px) from a row's left edge to its chip's left edge.
const CHIP_LEFT_MARGIN: i32 = 6;
/// Gap (px) between a chip's right edge and the row's text block.
const CHIP_TEXT_GAP: i32 = 8;
/// Right margin (px) reserved for the focused row's disclosure caret.
const CARET_RIGHT_MARGIN: i32 = 6;

/// Side length (px) of a row's chip: sized to the two-line name+username
/// text block's own height (`NAME_LINE_FOOTPRINT + LINE_GAP +
/// USERNAME_LINE_FOOTPRINT`, i.e. `ROW_HEIGHT` minus its top/bottom
/// `ROW_PADDING`) so the chip's vertical extent lines up with the text
/// beside it instead of bleeding into the row's padding — the same
/// "vertical padding is sacred" invariant `text_never_bleeds_past_a_rows_
/// bottom_padding` enforces for text.
pub(crate) fn chip_size() -> u32 {
    (NAME_LINE_FOOTPRINT + LINE_GAP + USERNAME_LINE_FOOTPRINT) as u32
}

/// Row-relative X offset where a row's text block (name/username) starts —
/// past the chip's left margin, its own width, and the chip-to-text gap.
/// `pub(crate)`: see [`name_top_offset`]'s doc comment for why
/// `credential_list_view.rs` needs to reuse layout constants like this one
/// rather than recomputing them independently.
pub(crate) fn text_left_offset() -> i32 {
    CHIP_LEFT_MARGIN + chip_size() as i32 + CHIP_TEXT_GAP
}

/// Row-relative Y offset for the name line's
/// `render_aligned(.., VerticalPosition::Top, ..)` call.
///
/// `pub(crate)`: shared with `credential_list_view.rs`, which draws its own
/// rows (scrollbar + selection accent) rather than delegating to
/// `VerticalList`, but must reuse this exact offset to avoid reintroducing
/// the row-overflow bug `ROW_HEIGHT`'s doc comment describes. Plain
/// layout constants now, not baseline-derived math — `u8g2-fonts`'
/// `VerticalPosition::Top` does the ascent/descent arithmetic internally,
/// which is the whole point of retiring the old `FONT_ASCENT`/
/// `FONT_DESCENT`/baseline-offset math this replaces.
pub(crate) const fn name_top_offset() -> i32 {
    ROW_PADDING
}

/// Row-relative Y offset for the username line's `render_aligned` call —
/// directly below the name line's worst-case footprint, plus `LINE_GAP`.
/// `pub(crate)`: see [`name_top_offset`]'s doc comment.
pub(crate) const fn username_top_offset() -> i32 {
    name_top_offset() + NAME_LINE_FOOTPRINT + LINE_GAP
}

/// Scroll offset (in pixels) that keeps row `selected` within
/// `[scroll, scroll + viewport_height)`. Extracted as a free function
/// (rather than kept solely as `VerticalList::scroll_for_viewport`'s
/// private method body) so `credential_list_view.rs`'s hand-rolled row
/// rendering can reuse the identical auto-scroll math instead of
/// re-deriving it.
pub(crate) fn scroll_offset_for_selection(selected: usize, viewport_height: u32) -> u32 {
    if viewport_height == 0 {
        return 0;
    }
    let selected_bottom = (selected as u32 + 1) * ROW_HEIGHT;
    selected_bottom.saturating_sub(viewport_height)
}

/// Draws one list row's shared visual language: an optional hairline
/// bottom divider, the chip, the bold name line, the muted username line,
/// and — for the focused/selected row — the full-width selection fill
/// (via [`theme::draw_selection`]) plus a right-edge disclosure caret.
///
/// Extracted as a free function (rather than duplicated inside both
/// `VerticalList::render` and `credential_list_view.rs`'s hand-rolled row
/// loop, per [`name_top_offset`]'s doc comment on why that duplication
/// exists at all) so the two independent row-rendering call sites cannot
/// visually drift apart — a design tweak here lands in both places by
/// construction, not by remembering to update both.
///
/// `draw_divider` is the caller's decision, not derived here: a caller
/// iterating its own rows top-to-bottom knows whether *this* row is
/// selected, which is the only input the divider rule needs (see the call
/// sites) — drawn only below an *unselected* row, since a following
/// selected row's own full-width fill (drawn on top, after, when that next
/// row is rendered) already paints over/replaces it, and a divider
/// directly below a *selected* row would fight the selection block's own
/// bottom edge instead of reading as a plain row separator.
///
/// Generic over `D: DrawTarget<Color = Rgb565, Error = Infallible>` for
/// the same reason [`theme::draw_selection`] is — callers pass a
/// `DrawTargetExt::clipped()` sub-region directly.
///
/// # Errors
///
/// Returns `Infallible`'s uninhabited variant in practice — see
/// [`super::widget::Widget::render`]'s doc comment for why the `Result`
/// return exists at all.
pub(crate) fn draw_row<D>(
    target: &mut D,
    row_rect: Rectangle,
    name: &str,
    username: Option<&str>,
    selected: bool,
    draw_divider: bool,
) -> Result<(), Infallible>
where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    if selected {
        theme::draw_selection(row_rect, target)?;
    } else if draw_divider {
        let divider = Rectangle::new(
            Point::new(row_rect.top_left.x, row_rect.top_left.y + row_rect.size.height as i32 - 1),
            Size::new(row_rect.size.width, 1),
        );
        divider.into_styled(PrimitiveStyle::with_fill(palette::DIVIDER)).draw(target)?;
    }

    let initial = name.chars().next().map_or('#', |c| c.to_ascii_uppercase());
    let chip_rect = Rectangle::new(
        Point::new(row_rect.top_left.x + CHIP_LEFT_MARGIN, row_rect.top_left.y + ROW_PADDING),
        Size::new_equal(chip_size()),
    );
    theme::draw_chip(target, chip_rect, initial)?;

    let text_x = row_rect.top_left.x + text_left_offset();

    let _ = font::name().render_aligned(
        name,
        Point::new(text_x, row_rect.top_left.y + name_top_offset()),
        VerticalPosition::Top,
        HorizontalAlignment::Left,
        FontColor::Transparent(palette::TEXT_PRIMARY),
        target,
    );

    if let Some(username) = username {
        let _ = font::username().render_aligned(
            username,
            Point::new(text_x, row_rect.top_left.y + username_top_offset()),
            VerticalPosition::Top,
            HorizontalAlignment::Left,
            FontColor::Transparent(palette::TEXT_SECONDARY),
            target,
        );
    }

    if selected {
        let mut buf = [0_u8; 4];
        let caret: &str = icon::CARET_RIGHT.encode_utf8(&mut buf);
        let caret_x = row_rect.top_left.x + row_rect.size.width as i32 - CARET_RIGHT_MARGIN;
        let caret_y = row_rect.top_left.y + row_rect.size.height as i32 / 2;
        let _ = font::icon_1x().render_aligned(
            caret,
            Point::new(caret_x, caret_y),
            VerticalPosition::Center,
            HorizontalAlignment::Right,
            FontColor::Transparent(palette::TEXT_PRIMARY),
            target,
        );
    }

    Ok(())
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
        scroll_offset_for_selection(self.selected, viewport_height)
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

            let selected = self.focused && index == self.selected;
            draw_row(
                &mut clipped,
                row_rect,
                item.label.as_str(),
                item.sublabel.as_deref(),
                selected,
                !selected,
            )?;
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
