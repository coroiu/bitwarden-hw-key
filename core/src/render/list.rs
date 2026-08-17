//! A scrolling vertical list widget: the one content widget this bead
//! needs to prove the render core end-to-end (see the ADR's component
//! library list — `VerticalMenu` is the salvaged concept, reimplemented on
//! `embedded-graphics` instead of the retired RGBA rasterizer).
//!
//! [`ListItem`] is a display-only row shape, deliberately not
//! `VaultItem` — the widget layer shouldn't know about the credential
//! domain model. Call sites (e.g. a future `CredentialListView`) map
//! `VaultItem -> ListItem`.

use std::cell::Cell;
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

/// Reconciles a list's scroll-top **row index** against a newly resolved
/// selection — the "only scroll at the viewport edges" rule (bead
/// `ai-bitwarden-hw-key-47g`), replacing the retired
/// `scroll_offset_for_selection`/`VerticalList::scroll_for_viewport`
/// (see their old doc comments' git history for why this file used to
/// pin the selected row to the viewport's bottom edge on every render —
/// a `selected`-only pure function had no way to know the list *hadn't*
/// scrolled off past that row, only where it currently is).
///
/// Index-based (rows, not pixels) so it's resolution-independent — the
/// caller multiplies by whatever its row height is (`ROW_HEIGHT` here,
/// a pixel-range variant for `credential_detail_view.rs`'s per-field
/// bead `ai-bitwarden-hw-key-8kx`).
///
/// # Why this runs at render time, not in `on_intent`
///
/// `on_intent` only knows the selection *delta* (`NavIntent::Next`/
/// `Prev`/`NextN`) — it has no idea how many rows the viewport can
/// currently show (`area.size.height` is a render-time input, passed to
/// `Widget::render`, never to `Widget::on_intent`). This function is
/// therefore called from `render`, fed whatever `top_index` was
/// persisted from the *previous* render — **not** a design smell to
/// "fix" by threading `visible_rows` through `on_intent` instead: the
/// rule below is an idempotent clamp (calling it twice in a row with
/// the same inputs returns the same `top`), so re-running it every
/// render is exactly as correct as running it once per intent would be,
/// just simpler (one call site, no risk of `on_intent` and `render`
/// disagreeing about `visible_rows` if the viewport is ever resized).
/// **Do not** move this into `on_intent` — that would reintroduce the
/// "no viewport dimensions available" problem this design sidesteps.
///
/// # The rule
///
/// - No items: top is always `0`.
/// - `prev_top` is first clamped to `max_top` (`item_count -
///   visible_rows`, floored at `0`) — handles the list having shrunk
///   since the last render (e.g. a deletion), so a stale `top` can't
///   leave blank space below the last row.
/// - If `selected` is above the current window (`selected < top`):
///   scroll up exactly enough to make it the *first* visible row.
/// - If `selected` is below the current window (`selected >= top +
///   visible_rows`): scroll down exactly enough to make it the *last*
///   visible row.
/// - Otherwise (`selected` is already somewhere inside `[top, top +
///   visible_rows)`): **`top` is left unchanged.** This is the actual
///   fix — the old pin-to-bottom formula recomputed a fresh scroll
///   position from `selected` alone on every call, so moving the
///   selection *up* while it was still fully visible re-pinned it to
///   the viewport's bottom edge anyway (see bead 47g's repro: move
///   down, then back up one row — the row above was already on
///   screen, yet the old code scrolled the list to redraw it at the
///   bottom).
#[must_use]
pub(crate) fn reconcile_top_index(prev_top: usize, selected: usize, visible_rows: usize, item_count: usize) -> usize {
    if item_count == 0 {
        return 0;
    }
    let visible_rows = visible_rows.max(1);
    let max_top = item_count.saturating_sub(visible_rows);
    let mut top = prev_top.min(max_top);

    if selected < top {
        top = selected;
    } else if selected >= top + visible_rows {
        top = selected - visible_rows + 1;
    }
    // else: selected is already visible within the current window --
    // leave `top` unchanged. THE FIX.

    top.min(max_top)
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
/// `Widget::on_intent`, auto-scrolling to keep the selection visible (only
/// at the viewport edges — see [`reconcile_top_index`]); fires its
/// `on_activate` callback (if any) when activated while focused.
pub struct VerticalList {
    items: Vec<ListItem>,
    selected: usize,
    /// The row index currently scrolled to the top of the viewport.
    /// `Cell`, not a plain field, for the same reason
    /// `CredentialListView`'s `selected_id`/`last_index` are: `Widget::
    /// render` takes `&self` but still needs to persist this across
    /// calls (see [`reconcile_top_index`]'s doc comment on why the
    /// reconciliation runs in `render`, not `on_intent`).
    top_index: Cell<usize>,
    focused: bool,
    on_activate: Option<OnActivate>,
}

impl VerticalList {
    #[must_use]
    pub fn new(items: Vec<ListItem>) -> Self {
        Self {
            items,
            selected: 0,
            top_index: Cell::new(0),
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

        // "Only scroll at the viewport edges" (bead 47g) -- see
        // `reconcile_top_index`'s doc comment for the full rule and why
        // this reconciliation happens here, in `render`, rather than in
        // `on_intent`.
        let visible_rows = (area.size.height / ROW_HEIGHT).max(1) as usize;
        let top = reconcile_top_index(self.top_index.get(), self.selected, visible_rows, self.items.len());
        self.top_index.set(top);
        let scroll = top as u32 * ROW_HEIGHT;

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
        // Updated to bead 47g's "only scroll at the viewport edges" rule:
        // `VerticalList` now persists `top_index` across `render` calls
        // instead of recomputing a fresh scroll position from `selected`
        // alone every time (the retired `scroll_offset_for_selection`'s
        // pin-to-bottom behavior).
        let mut list = VerticalList::new(items(10));
        let visible_rows = 3;
        let viewport_height = visible_rows as u32 * ROW_HEIGHT; // fits 3 rows
        let area = Rectangle::new(Point::new(0, 0), Size::new(320, viewport_height));
        let mut fb = FrameBuffer565::new(320, viewport_height);

        // Selection within the first screenful: no scroll needed.
        list.render(area, &mut fb).unwrap();
        assert_eq!(list.top_index.get(), 0);

        // Selecting row 4 (0-indexed) means rows 0-3 no longer all fit;
        // top should advance just enough to make row 4 the last visible
        // row (top=2: rows 2,3,4 visible), not "selected's own bottom
        // pinned to the viewport bottom" (the old, buggy rule).
        for _ in 0..4 {
            list.on_intent(NavIntent::Next);
        }
        list.render(area, &mut fb).unwrap();
        assert_eq!(list.top_index.get(), 2);

        // The selected row's own top and bottom must both land inside
        // the visible window.
        let scroll = list.top_index.get() as u32 * ROW_HEIGHT;
        let selected_top = 4 * ROW_HEIGHT;
        let selected_bottom = selected_top + ROW_HEIGHT;
        assert!(selected_top >= scroll);
        assert!(selected_bottom <= scroll + viewport_height);
    }

    #[test]
    fn moving_up_while_still_visible_does_not_re_pin_to_the_viewport_edge() {
        // The bead 47g repro, exercised through the actual widget (not
        // just `reconcile_top_index` directly, below): move down enough
        // to scroll, then back up ONE row that was already visible --
        // the list must not scroll again.
        let mut list = VerticalList::new(items(10));
        let visible_rows = 3;
        let viewport_height = visible_rows as u32 * ROW_HEIGHT;
        let area = Rectangle::new(Point::new(0, 0), Size::new(320, viewport_height));
        let mut fb = FrameBuffer565::new(320, viewport_height);

        for _ in 0..4 {
            list.on_intent(NavIntent::Next);
        }
        list.render(area, &mut fb).unwrap(); // selected=4, top settles at 2
        let top_after_scrolling_down = list.top_index.get();
        assert_eq!(top_after_scrolling_down, 2);

        list.on_intent(NavIntent::Prev); // selected=3, still within [2, 5)
        list.render(area, &mut fb).unwrap();
        assert_eq!(
            list.top_index.get(),
            top_after_scrolling_down,
            "moving up into an already-visible row must not scroll the list"
        );
    }

    mod reconcile_top_index_tests {
        use super::super::reconcile_top_index;

        // Fern's traces (bead 47g design), each checked independently
        // against the pure function rather than through the widget, so
        // the exact rule is pinned down unambiguously.

        #[test]
        fn a_big_jump_scrolls_down_just_enough_to_reveal_the_new_selection() {
            // NextN(20) from top=0, visible_rows=3, 10 items clamps
            // selection to the last item (9), and top should land at 7
            // (rows 7,8,9 visible -- 9 is the new last visible row).
            assert_eq!(reconcile_top_index(0, 9, 3, 10), 7);
        }

        #[test]
        fn moving_above_the_window_scrolls_up_to_make_it_the_first_row() {
            // Continuing from top=7: selecting row 6 (above the current
            // [7, 10) window) scrolls up so it becomes the top row.
            assert_eq!(reconcile_top_index(7, 6, 3, 10), 6);
        }

        #[test]
        fn moving_within_the_current_window_leaves_top_unchanged() {
            // THE FIX: with top=6 (window [6, 9)), selecting 6, 7, or 8
            // must never change top -- this is exactly what the old
            // pin-to-bottom formula got wrong (it would re-pin on every
            // move regardless of whether the row was already visible).
            assert_eq!(reconcile_top_index(6, 6, 3, 10), 6);
            assert_eq!(reconcile_top_index(6, 7, 3, 10), 6);
            assert_eq!(reconcile_top_index(6, 8, 3, 10), 6);
        }

        #[test]
        fn a_shrinking_list_clamps_top_so_there_is_no_blank_space_below_the_last_row() {
            // The list shrank to 4 items (visible_rows still 3) while top
            // was still 6 from a longer list -- max_top is now 4-3=1, so
            // top must clamp down to 1, not leave rows 6.. rendering
            // nothing while the viewport has unused space.
            assert_eq!(reconcile_top_index(6, 3, 3, 4), 1);
        }

        #[test]
        fn zero_items_always_reports_top_zero() {
            assert_eq!(reconcile_top_index(5, 0, 3, 0), 0);
        }

        #[test]
        fn visible_rows_of_zero_is_treated_as_one_not_a_division_by_zero() {
            // `visible_rows.max(1)` in the implementation -- a
            // zero-height viewport must not panic or produce a
            // nonsensical max_top.
            assert_eq!(reconcile_top_index(0, 0, 0, 5), 0);
        }

        #[test]
        fn regression_47g_moving_below_the_window_then_back_up_one_row_leaves_top_unchanged() {
            // The exact bug bead 47g reported, reproduced against the
            // pure function directly (the widget-level equivalent is
            // `moving_up_while_still_visible_does_not_re_pin_to_the_
            // viewport_edge` above): the selection moves down far enough
            // that the window has to scroll (top advances from 0 to 2 as
            // selected goes 0->4, one `Next` at a time, mirroring what
            // `VerticalList::render` actually does on every frame), then
            // Prev moves the selection back up ONE row that is still
            // inside that window -- top must stay exactly where it was,
            // not recompute a fresh position from the new `selected`
            // alone (the retired `scroll_offset_for_selection`'s bug:
            // it would have put top back at 1, not 2, because it never
            // looked at where the window currently was).
            let visible_rows = 3;
            let item_count = 10;

            let mut top = 0;
            for selected in 0..=4 {
                top = reconcile_top_index(top, selected, visible_rows, item_count);
            }
            assert_eq!(top, 2, "sanity check: the window should have scrolled to keep row 4 visible");

            let top_before_prev = top;
            let top_after_prev = reconcile_top_index(top, 3, visible_rows, item_count);
            assert_eq!(
                top_after_prev, top_before_prev,
                "moving back into an already-visible row must not scroll the list"
            );
        }
    }

    #[test]
    fn render_does_not_panic_for_a_small_viewport_with_more_rows_than_fit() {
        let list = VerticalList::new(items(50));
        let mut fb = FrameBuffer565::new(64, 40);
        let area = Rectangle::new(Point::new(0, 0), Size::new(64, 40));
        list.render(area, &mut fb).unwrap();
    }
}
