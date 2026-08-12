//! `CredentialListView`: the real, store-backed credential list widget.
//!
//! Replaces `app.rs`'s M0-shortcut `StoreBackedCredentialList` (deleted by
//! this bead), which tracked selection by raw index and had no
//! empty/error/waiting content states. Per
//! `.planning/decisions/2026-08-12-m1-vault-store-data-ownership.md` and
//! Uma's UX + Fern's B2 framework design (bead `ai-bitwarden-hw-key-0v8.4`):
//!
//! - Reads [`VaultStore::items`] *live* at render time — never a cached
//!   snapshot.
//! - Tracks selection by [`VaultItem`] id, not index, so a background sync
//!   that reorders or inserts items doesn't jump the cursor (see
//!   [`CredentialListView::resolve_selection`]).
//! - Renders a distinct content state (waiting-for-first-sync / empty
//!   vault / error) when there are no items to show, and otherwise always
//!   renders the last-known-good list — even mid-`SyncStatus::Error` (per
//!   the ADR: an error must not blank a previously-populated list).
//! - Draws a right-edge scrollbar and a full-row focus block (shared
//!   [`crate::render::draw_focus_block`] helper) on the selected row.
//!
//! ## The "focus-init runs once" gotcha
//!
//! `Screen::initialize_focus` (see `render/screen.rs`) runs exactly once,
//! when a screen is pushed onto the `Navigator`. It checks
//! `Widget::is_focusable()` at that single moment to decide which widget
//! gets the initial focus highlight; there is no hook to re-run it later.
//! Bead `ai-bitwarden-hw-key-0v8.3`'s implementer flagged the resulting
//! trap: a widget that reports `is_focusable() == false` while its backing
//! store is empty, then later becomes non-empty via a background sync,
//! never gets that one-time focus check re-run — it would render its rows
//! but never gain the selection highlight, with no user input able to fix
//! it (there's nothing else on the screen to focus away from and back to).
//!
//! The fix here: [`CredentialListView::is_focusable`] unconditionally
//! returns `true`. This widget *always* owns focus once its screen is
//! pushed, regardless of how many items the store holds at that moment.
//! Selection state (`selected_id`/`last_index`) and rendering are written
//! to tolerate zero items (no row to highlight, no crash), so there is
//! nothing unsafe about being "focusable" with nothing focusable to select
//! yet — the moment items arrive, the very next render resolves a
//! selection and paints the highlight, with no extra focus-plumbing
//! required. See `focus_survives_the_empty_to_populated_transition` below
//! for the regression test.

// Identical allow (and rationale) as `bhk_core::render`: this module does
// the same `embedded-graphics` `Point`(i32)/`Size`(u32) coordinate math
// directly (row/scrollbar/message layout), so the same justification
// applies — no display this project targets is anywhere near large enough
// for these conversions to wrap, truncate, or lose a sign in practice.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::rc::Rc;

use embedded_graphics::{
    draw_target::DrawTargetExt,
    pixelcolor::Rgb565,
    prelude::{Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
    Drawable,
};
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use uuid::Uuid;

use crate::input::NavIntent;
use crate::render::list::{draw_row, name_top_offset, scroll_offset_for_selection, username_top_offset};
use crate::render::theme::{font, icon, palette};
use crate::render::{Action, ChromeContribution, ChromeStatus, FocusEvent, FrameBuffer565, Widget, ROW_HEIGHT};
use crate::vault_item::VaultItem;
use crate::vault_store::{SyncStatus, VaultStore};

/// Width, in pixels, reserved at the content area's right edge for the
/// scrollbar track/thumb. Reserved unconditionally (not just when the list
/// overflows) so the row text's right boundary doesn't shift depending on
/// item count.
const SCROLLBAR_WIDTH: u32 = 3;

/// Minimum thumb height, so a very long list doesn't shrink the thumb to
/// an unclickable/invisible sliver.
const MIN_THUMB_HEIGHT: u32 = 4;

/// Extra top padding (beyond the label's own baseline offset) before the
/// first line of an empty/waiting/error content message, so it isn't
/// glued to the chrome's title-bar boundary.
const MESSAGE_TOP_PADDING: i32 = 16;

/// Approximate square footprint (px) of `font::icon_4x()` glyphs — u8g2
/// scales `open_iconic_all` in even multiples of its ~8px 1x unit, so 4x is
/// ~32px. Only used to budget vertical space before the headline in
/// [`render_message`]; a few pixels of slack either way just changes the
/// gap, not correctness.
const MESSAGE_ICON_SIZE: i32 = 32;

/// Gap (px) between [`render_message`]'s icon (when present) and its
/// headline text.
const MESSAGE_ICON_GAP: i32 = 12;

/// Callback invoked with the selected [`VaultItem`]'s id when the list is
/// activated (encoder short press / `NavIntent::Activate`) while focused.
/// Mirrors `VerticalList`'s `on_activate`, but keyed by id rather than by
/// `&ListItem` — a `CredentialListView` consumer needs the id to look the
/// credential back up in the store (e.g. to build a detail screen), not a
/// display-only row.
type OnActivate = Box<dyn Fn(Uuid) -> Action>;

/// The derived "what should the content region show" state, computed from
/// live item count + [`SyncStatus`] on every render — never cached.
#[derive(Debug, PartialEq, Eq)]
enum ContentState<'a> {
    /// At least one item: render the list, regardless of `SyncStatus`
    /// (including `Error` — the ADR's "keep last-good list rendered, do
    /// not blank" requirement).
    List,
    /// No items, no sync attempted yet (`VaultStore::status() == None`).
    Waiting,
    /// No items, last sync succeeded with an empty vault.
    Empty,
    /// No items, last sync failed. Distinct from `Empty` so the user can
    /// tell "your vault really is empty" from "we don't know, sync
    /// failed."
    Error(&'a str),
}

fn content_state(item_count: usize, status: Option<&SyncStatus>) -> ContentState<'_> {
    if item_count > 0 {
        return ContentState::List;
    }
    match status {
        None => ContentState::Waiting,
        Some(SyncStatus::Error(message)) => ContentState::Error(message),
        // `Synced` is not reachable here via `VaultStore::apply_sync_ok`
        // (0 items always derives `Empty`, never `Synced`), but the type
        // doesn't know that — fail toward the more informative "vault is
        // empty" state rather than an incorrect "waiting" state.
        Some(SyncStatus::Empty | SyncStatus::Synced) => ContentState::Empty,
    }
}

/// The root credential list screen's content widget: a thin, always-live
/// adapter over a shared [`VaultStore`]. See the module doc for the full
/// design (id-based selection, content states, the focus-init fix).
pub struct CredentialListView {
    store: Rc<RefCell<VaultStore>>,
    /// The currently selected credential's id, or `None` if nothing has
    /// been resolved yet (fresh widget) or the store is empty. `Cell`
    /// (not a plain field) because [`Widget::render`] takes `&self` but
    /// still needs to resolve/update the selection against the live store
    /// on every call — see [`CredentialListView::resolve_selection`].
    selected_id: Cell<Option<Uuid>>,
    /// The last resolved index, kept even when `selected_id` is `None` or
    /// stale, so that if the selected id vanishes (deleted upstream) the
    /// widget clamps to the item now nearest that position rather than
    /// jumping back to the top.
    last_index: Cell<usize>,
    focused: bool,
    on_activate: Option<OnActivate>,
}

impl CredentialListView {
    #[must_use]
    pub fn new(store: Rc<RefCell<VaultStore>>) -> Self {
        Self {
            store,
            selected_id: Cell::new(None),
            last_index: Cell::new(0),
            focused: false,
            on_activate: None,
        }
    }

    /// Registers a callback invoked with the selected credential's id when
    /// the list is activated while focused. Not wired by `App` yet — bead
    /// `ai-bitwarden-hw-key-0v8.4` (this one) has no detail screen to push;
    /// that is bead `ai-bitwarden-hw-key-0v8.6`'s job, which will call this
    /// with `|id| Action::PushView(Box::new(move || detail_screen(id,
    /// store)))` (or similar). Until then, [`Widget::on_focus`]'s
    /// `Activated` arm is a documented no-op, not a silent stub — see the
    /// module doc.
    #[must_use]
    pub fn on_activate(mut self, callback: impl Fn(Uuid) -> Action + 'static) -> Self {
        self.on_activate = Some(Box::new(callback));
        self
    }

    /// The number of credentials currently in the store, read live.
    /// Exposed for chrome/readout consumers (e.g. bead
    /// `ai-bitwarden-hw-key-0v8.5`'s "N of M" indicator).
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.store.borrow().items().len()
    }

    /// The 0-based index of the currently selected row, live-resolved
    /// against the store. `None` if the store has no items.
    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        let store = self.store.borrow();
        self.resolve_selection(store.items())
    }

    /// Re-resolves `selected_id` against `items` (the *live* store
    /// contents passed in by the caller), returning the resolved index.
    ///
    /// - If `items` is empty: clears the selection, returns `None`.
    /// - If `selected_id` still exists in `items`: returns its (possibly
    ///   changed, e.g. after a reorder/insert) index. This is the crux of
    ///   "selection by id, not index" — a background sync that shuffles
    ///   or inserts items does not jump the cursor to a different
    ///   credential, because the id is what's tracked, not a position.
    /// - Otherwise (fresh widget with no selection yet, or the previously
    ///   selected id vanished — deleted upstream): clamps `last_index` to
    ///   the new bounds and adopts whatever item now sits at that
    ///   (possibly clamped) position as the new selection. This is the
    ///   "clamp to a valid neighbor" behavior: a deletion moves the
    ///   cursor to the item that's now nearest the old visual position,
    ///   not back to the top of the list.
    ///
    /// Takes `&self` (not `&mut self`) via `Cell` fields so both
    /// `Widget::render` (`&self`) and `Widget::on_intent`/`on_focus`
    /// (`&mut self`) can call it uniformly — resolution has to happen at
    /// render time too, since a store mutation (`App::step`) can land
    /// between any two renders without an intervening `on_intent` call.
    fn resolve_selection(&self, items: &[VaultItem]) -> Option<usize> {
        if items.is_empty() {
            self.selected_id.set(None);
            self.last_index.set(0);
            return None;
        }

        if let Some(id) = self.selected_id.get() {
            if let Some(index) = items.iter().position(|item| item.id == id) {
                self.last_index.set(index);
                return Some(index);
            }
        }

        let clamped = self.last_index.get().min(items.len() - 1);
        self.selected_id.set(Some(items[clamped].id));
        self.last_index.set(clamped);
        Some(clamped)
    }

    /// Moves the selection by `delta` rows, resolving against live
    /// `items` first (so a `Next`/`Prev` right after a background sync
    /// still starts from the correct, up-to-date position).
    fn move_selection(&self, items: &[VaultItem], delta: i32) {
        if items.is_empty() {
            return;
        }
        let current = self.resolve_selection(items).unwrap_or(0) as i32;
        let len = items.len() as i32;
        let next = (current + delta).clamp(0, len - 1) as usize;
        self.selected_id.set(Some(items[next].id));
        self.last_index.set(next);
    }

    fn render_list(&self, area: Rectangle, items: &[VaultItem], target: &mut FrameBuffer565) -> Result<(), Infallible> {
        let selected = self.resolve_selection(items);

        let rows_width = area.size.width.saturating_sub(SCROLLBAR_WIDTH);
        let rows_area = Rectangle::new(area.top_left, Size::new(rows_width, area.size.height));

        let scroll = selected.map_or(0, |index| scroll_offset_for_selection(index, area.size.height));

        {
            let mut clipped = target.clipped(&rows_area);

            for (index, item) in items.iter().enumerate() {
                let row_top = rows_area.top_left.y + (index as u32 * ROW_HEIGHT) as i32 - scroll as i32;

                if row_top + ROW_HEIGHT as i32 <= rows_area.top_left.y
                    || row_top >= rows_area.top_left.y + rows_area.size.height as i32
                {
                    continue;
                }

                let row_rect = Rectangle::new(
                    Point::new(rows_area.top_left.x, row_top),
                    Size::new(rows_area.size.width, ROW_HEIGHT),
                );

                let row_selected = self.focused && selected == Some(index);
                draw_row(
                    &mut clipped,
                    row_rect,
                    item.name.as_str(),
                    Some(item.username.as_str()),
                    row_selected,
                    !row_selected,
                )?;
            }
        }

        render_scrollbar(area, items.len(), area.size.height, scroll, target)
    }

    /// The right-aligned title-bar readout (e.g. `"2 / 5"`), or `None` when
    /// there's nothing to count (no items). 1-based for the numerator —
    /// "row 0 selected" should read "1 / 5", not "0 / 5".
    fn readout(&self) -> Option<String> {
        let count = self.item_count();
        if count == 0 {
            return None;
        }
        self.selected_index().map(|index| format!("{} / {count}", index + 1))
    }

    /// Maps the store's derived [`SyncStatus`] to the chrome's semantic
    /// [`ChromeStatus`], per Andreas's spec: `Synced` -> success (green),
    /// `Error` -> error (red/amber), `Empty` -> neutral. `None` (no sync
    /// attempted yet) also reads as neutral — there's no "bad" news yet,
    /// just no news.
    fn chrome_status(&self) -> ChromeStatus {
        match self.store.borrow().status() {
            Some(SyncStatus::Synced) => ChromeStatus::Success,
            Some(SyncStatus::Error(_)) => ChromeStatus::Error,
            Some(SyncStatus::Empty) | None => ChromeStatus::Neutral,
        }
    }
}

/// Draws the right-edge scrollbar (thin track + proportional thumb) for a
/// list of `item_count` rows scrolled by `scroll` pixels within a
/// `viewport_height`-px-tall `area`. A no-op if the whole list already
/// fits the viewport — there's nothing to scroll, so no thumb (or even an
/// empty track) is drawn.
fn render_scrollbar(
    area: Rectangle,
    item_count: usize,
    viewport_height: u32,
    scroll: u32,
    target: &mut FrameBuffer565,
) -> Result<(), Infallible> {
    let total_height = item_count as u32 * ROW_HEIGHT;
    if viewport_height == 0 || total_height <= viewport_height {
        return Ok(());
    }

    let track_x = area.top_left.x + area.size.width as i32 - SCROLLBAR_WIDTH as i32;

    let track = Rectangle::new(Point::new(track_x, area.top_left.y), Size::new(SCROLLBAR_WIDTH, viewport_height));
    track.into_styled(PrimitiveStyle::with_fill(palette::DIVIDER)).draw(target)?;

    let thumb_height = (u64::from(viewport_height) * u64::from(viewport_height) / u64::from(total_height))
        .max(u64::from(MIN_THUMB_HEIGHT)) as u32;
    let thumb_height = thumb_height.min(viewport_height);

    let max_thumb_top = viewport_height - thumb_height;
    let scrollable_range = total_height - viewport_height;
    let thumb_top = if max_thumb_top == 0 || scrollable_range == 0 {
        0
    } else {
        (u64::from(scroll) * u64::from(max_thumb_top) / u64::from(scrollable_range)) as u32
    };

    let thumb = Rectangle::new(
        Point::new(track_x, area.top_left.y + thumb_top as i32),
        Size::new(SCROLLBAR_WIDTH, thumb_height),
    );
    thumb.into_styled(PrimitiveStyle::with_fill(palette::TEXT_SECONDARY)).draw(target)
}

/// Draws a centered content message — an optional large icon, a headline,
/// and an optional subline — for the non-list content states (waiting/
/// empty/error). Not list-specific, but kept private to this module
/// rather than promoted to `render::theme` (unlike `draw_selection`) —
/// nothing outside `CredentialListView` needs it yet, and it has no
/// framework-level generality (it's a fixed layout, not a general text
/// component).
///
/// Horizontally centered (`HorizontalAlignment::Center`) rather than the
/// original left-aligned layout: per the approved M1 design language, this
/// reads as a deliberate "nothing to show here" state, not a truncated
/// list row, and centering is what sells that — most visibly for the
/// `icon` case (`ContentState::Empty`'s shield + "No credentials yet"),
/// but applied uniformly so waiting/error don't look like a different
/// design language from empty.
///
/// Returns `()`, not `Result<(), Infallible>` like most of this module's
/// draw helpers: every draw call inside deliberately discards
/// `render_aligned`'s `Result` (see `font`'s module doc — unrenderable
/// glyphs are skipped, not propagated as an error), so there is truly
/// nothing left that can fail here to report. Callers in `Widget::render`
/// wrap this call with an explicit `Ok(())` to match the trait's
/// signature.
fn render_message(
    area: Rectangle,
    icon: Option<char>,
    headline: &str,
    headline_color: Rgb565,
    subline: Option<&str>,
    target: &mut FrameBuffer565,
) {
    let mut clipped = target.clipped(&area);
    let center_x = area.top_left.x + area.size.width as i32 / 2;

    let text_top = area.top_left.y
        + MESSAGE_TOP_PADDING
        + if icon.is_some() { MESSAGE_ICON_SIZE + MESSAGE_ICON_GAP } else { 0 };

    if let Some(icon_char) = icon {
        let mut buf = [0_u8; 4];
        let icon_str: &str = icon_char.encode_utf8(&mut buf);
        let _ = font::icon_4x().render_aligned(
            icon_str,
            Point::new(center_x, area.top_left.y + MESSAGE_TOP_PADDING),
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::Transparent(palette::BRAND_BRIGHT),
            &mut clipped,
        );
    }

    let _ = font::name().render_aligned(
        headline,
        Point::new(center_x, text_top + name_top_offset()),
        VerticalPosition::Top,
        HorizontalAlignment::Center,
        FontColor::Transparent(headline_color),
        &mut clipped,
    );

    if let Some(subline) = subline {
        let _ = font::username().render_aligned(
            subline,
            Point::new(center_x, text_top + username_top_offset()),
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::Transparent(palette::TEXT_SECONDARY),
            &mut clipped,
        );
    }
}

impl Widget for CredentialListView {
    fn measure(&self, constraints: Size) -> Size {
        constraints
    }

    /// Always `true` — see the module doc's "focus-init runs once" section
    /// for why this can't be `!self.store.borrow().items().is_empty()`
    /// (that was the M0 `StoreBackedCredentialList` behavior, and the bug
    /// this bead fixes).
    fn is_focusable(&self) -> bool {
        true
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
                let store = self.store.borrow();
                let items = store.items();
                let index = self.resolve_selection(items);
                let id = index.map(|i| items[i].id);
                drop(store);

                match (id, &self.on_activate) {
                    (Some(id), Some(callback)) => callback(id),
                    _ => Action::None,
                }
            }
        }
    }

    fn on_intent(&mut self, intent: NavIntent) -> Action {
        let store = self.store.borrow();
        let items = store.items();
        match intent {
            NavIntent::Next => self.move_selection(items, 1),
            NavIntent::Prev => self.move_selection(items, -1),
            NavIntent::NextN(n) => self.move_selection(items, i32::from(n)),
            NavIntent::Activate | NavIntent::Back => {}
        }
        Action::None
    }

    fn render(&self, area: Rectangle, target: &mut FrameBuffer565) -> Result<(), Infallible> {
        let store = self.store.borrow();
        let items = store.items().to_vec();
        let status = store.status().cloned();
        drop(store);

        match content_state(items.len(), status.as_ref()) {
            ContentState::List => self.render_list(area, &items, target),
            ContentState::Waiting => {
                render_message(area, None, "Waiting for sync...", palette::TEXT_PRIMARY, None, target);
                Ok(())
            }
            ContentState::Empty => {
                render_message(
                    area,
                    Some(icon::SHIELD),
                    "No credentials yet",
                    palette::TEXT_PRIMARY,
                    Some("Sync from your companion app"),
                    target,
                );
                Ok(())
            }
            ContentState::Error(message) => {
                render_message(area, None, "Sync error", palette::STATUS_ERROR, Some(message), target);
                Ok(())
            }
        }
    }

    /// Always contributes: a right-aligned "N / M" readout (when there are
    /// items to count), a title-bar status dot derived from the store's
    /// [`SyncStatus`], and contextual hint text that differs between "there
    /// is a list to browse" and "there is nothing to browse, go back." This
    /// widget is unconditionally focusable (see `is_focusable`'s doc
    /// comment) and is the only widget on its screen, so its contribution
    /// is shown on every frame regardless of item count/content state.
    fn chrome_contribution(&self) -> Option<ChromeContribution> {
        let has_items = self.item_count() > 0;
        let hint = if has_items {
            "Rotate to browse - Press to open"
        } else {
            "Hold to go back"
        };

        Some(ChromeContribution {
            title: None,
            readout: self.readout(),
            hint: Some(hint.to_string()),
            status: Some(self.chrome_status()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::chrome::compute_chrome;
    use crate::render::Navigator;
    use embedded_graphics::prelude::OriginDimensions;

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

    fn store_with(items: Vec<VaultItem>) -> Rc<RefCell<VaultStore>> {
        let store = Rc::new(RefCell::new(VaultStore::new()));
        store.borrow_mut().apply_sync_ok(items);
        store
    }

    const AREA: Rectangle = Rectangle::new(Point::new(0, 0), Size::new(320, 150));

    #[test]
    fn is_always_focusable_even_when_the_store_is_empty() {
        let view = CredentialListView::new(store_with(vec![]));
        assert!(view.is_focusable());
    }

    #[test]
    fn a_fresh_view_resolves_selection_to_the_first_item() {
        let view = CredentialListView::new(store_with(vec![item("GitHub"), item("AWS")]));
        assert_eq!(view.selected_index(), Some(0));
    }

    #[test]
    fn empty_store_resolves_no_selection() {
        let view = CredentialListView::new(store_with(vec![]));
        assert_eq!(view.selected_index(), None);
    }

    #[test]
    fn next_and_prev_move_selection_and_clamp() {
        let mut view = CredentialListView::new(store_with(vec![item("A"), item("B"), item("C")]));
        assert_eq!(view.selected_index(), Some(0));

        view.on_intent(NavIntent::Prev); // clamp below zero
        assert_eq!(view.selected_index(), Some(0));

        view.on_intent(NavIntent::Next);
        assert_eq!(view.selected_index(), Some(1));
        view.on_intent(NavIntent::Next);
        assert_eq!(view.selected_index(), Some(2));
        view.on_intent(NavIntent::Next); // clamp at the end
        assert_eq!(view.selected_index(), Some(2));
    }

    #[test]
    fn selection_by_id_survives_a_reorder_and_insert() {
        let a = item("A");
        let b = item("B");
        let c = item("C");
        let b_id = b.id;
        let store = store_with(vec![a.clone(), b.clone(), c.clone()]);
        let mut view = CredentialListView::new(Rc::clone(&store));

        view.on_intent(NavIntent::Next); // select B (index 1)
        assert_eq!(view.selected_index(), Some(1));

        // A background sync reorders and inserts a new item ahead of B.
        let d = item("D");
        store.borrow_mut().apply_sync_ok(vec![c, d, a, b.clone()]);

        // Selection follows B's id to its new index (3), not "index 1"
        // (which is now a different credential, D).
        assert_eq!(view.selected_index(), Some(3));
        assert_eq!(
            store.borrow().items()[view.selected_index().unwrap()].id,
            b_id,
            "the resolved index must still point at the same credential"
        );
    }

    #[test]
    fn a_deleted_selected_id_clamps_to_a_neighbor_instead_of_jumping_to_the_top() {
        let items: Vec<VaultItem> = ["A", "B", "C"].iter().map(|n| item(n)).collect();
        let store = store_with(items.clone());
        let mut view = CredentialListView::new(Rc::clone(&store));

        view.on_intent(NavIntent::Next);
        view.on_intent(NavIntent::Next);
        assert_eq!(view.selected_index(), Some(2)); // C selected

        // C is deleted upstream; A and B remain.
        store.borrow_mut().apply_sync_ok(vec![items[0].clone(), items[1].clone()]);

        // Clamped to the last valid index (1 = B), not reset to 0.
        assert_eq!(view.selected_index(), Some(1));
    }

    #[test]
    fn resolving_with_no_items_clears_the_selection() {
        let store = store_with(vec![item("A")]);
        let view = CredentialListView::new(Rc::clone(&store));
        assert_eq!(view.selected_index(), Some(0));

        store.borrow_mut().apply_sync_ok(vec![]);
        assert_eq!(view.selected_index(), None);

        // And a subsequent re-population starts fresh at index 0 again.
        store.borrow_mut().apply_sync_ok(vec![item("B"), item("C")]);
        assert_eq!(view.selected_index(), Some(0));
    }

    #[test]
    fn focus_survives_the_empty_to_populated_transition() {
        // The regression test for the "focus-init runs once" gotcha
        // flagged by 0v8.3: a screen built while the store is empty, then
        // populated after the fact (simulating a background sync landing
        // after `Navigator::new`'s one-time `initialize_focus` call), must
        // still show a selection highlight once items exist — with no
        // further input required.
        let store = store_with(vec![]);
        let view = CredentialListView::new(Rc::clone(&store));
        let screen = crate::render::Screen::new("Vault", vec![Box::new(view)]);
        let mut navigator = Navigator::new(screen); // initialize_focus runs here, store is empty

        store.borrow_mut().apply_sync_ok(vec![item("GitHub"), item("AWS")]);

        let mut fb = FrameBuffer565::new(320, 170);
        navigator.render(&mut fb).unwrap();

        let chrome = compute_chrome(fb.size());
        // x=250: past the 4px selection accent bar and past these short
        // labels' text, so it samples the row's plain elevated fill.
        let row0_highlight = fb.pixel(Point::new(250, chrome.content.top_left.y + 2));
        assert_eq!(
            row0_highlight,
            palette::SURFACE_ELEVATED,
            "row 0 should be highlighted as selected immediately once items exist, \
             with no extra focus-cycling input needed"
        );

        // dispatch is exercised too, proving the widget still responds
        // normally to input after the empty->populated transition.
        navigator.dispatch(NavIntent::Next);
        navigator.render(&mut fb).unwrap();
        let row0_after_move = fb.pixel(Point::new(250, chrome.content.top_left.y + 2));
        assert_ne!(row0_after_move, palette::SURFACE_ELEVATED);
    }

    #[test]
    fn waiting_state_renders_when_no_sync_has_happened_yet() {
        let store = Rc::new(RefCell::new(VaultStore::new()));
        let view = CredentialListView::new(store);
        let mut fb = FrameBuffer565::new(320, 170);
        view.render(AREA, &mut fb).unwrap();

        // No list rows: no selection-fill pixel anywhere, but the message
        // did draw something (primary-text-colored ink) into the content
        // area.
        let any_selected_fill = fb.pixels().any(|p| p.1 == palette::SURFACE_ELEVATED);
        assert!(!any_selected_fill);
        let any_headline_ink = fb.pixels().any(|p| p.1 == palette::TEXT_PRIMARY);
        assert!(any_headline_ink, "the waiting message's headline text should have drawn something");
    }

    #[test]
    fn empty_vault_state_differs_from_waiting_state() {
        let never_synced = Rc::new(RefCell::new(VaultStore::new()));
        let view_waiting = CredentialListView::new(never_synced);
        let mut fb_waiting = FrameBuffer565::new(320, 170);
        view_waiting.render(AREA, &mut fb_waiting).unwrap();

        let view_empty = CredentialListView::new(store_with(vec![]));
        let mut fb_empty = FrameBuffer565::new(320, 170);
        view_empty.render(AREA, &mut fb_empty).unwrap();

        let waiting_pixels: Vec<Rgb565> = fb_waiting.pixels().map(|p| p.1).collect();
        let empty_pixels: Vec<Rgb565> = fb_empty.pixels().map(|p| p.1).collect();
        assert_ne!(waiting_pixels, empty_pixels, "\"waiting\" and \"empty\" are different messages");
    }

    #[test]
    fn error_state_with_no_prior_items_renders_distinctly_from_empty_state() {
        let store = Rc::new(RefCell::new(VaultStore::new()));
        store.borrow_mut().apply_sync_err("network unreachable".to_string());
        let view = CredentialListView::new(store);
        let mut fb = FrameBuffer565::new(320, 170);
        view.render(AREA, &mut fb).unwrap();

        let any_error_color = fb.pixels().any(|p| p.1 == palette::STATUS_ERROR);
        assert!(any_error_color, "the error headline should render in its distinct color");
    }

    #[test]
    fn a_sync_error_with_existing_items_keeps_rendering_the_last_good_list_unchanged() {
        // The ADR's "keep last-good list rendered, do not blank" behavior,
        // proved pixel-for-pixel: an Error status must not change a single
        // pixel of the list rendering as long as the items themselves are
        // unchanged.
        let items = vec![item("GitHub"), item("AWS")];
        let store = store_with(items.clone());
        let view = CredentialListView::new(Rc::clone(&store));

        let mut fb_synced = FrameBuffer565::new(320, 170);
        view.render(AREA, &mut fb_synced).unwrap();

        store.borrow_mut().apply_sync_err("network unreachable".to_string());
        let mut fb_error = FrameBuffer565::new(320, 170);
        view.render(AREA, &mut fb_error).unwrap();

        let synced_pixels: Vec<Rgb565> = fb_synced.pixels().map(|p| p.1).collect();
        let error_pixels: Vec<Rgb565> = fb_error.pixels().map(|p| p.1).collect();
        assert_eq!(synced_pixels, error_pixels, "a sync error must not blank or alter the last-good list");
    }

    #[test]
    fn live_update_after_sync_is_visible_on_the_very_next_render() {
        let store = store_with(vec![]);
        let view = CredentialListView::new(Rc::clone(&store));

        let mut fb_before = FrameBuffer565::new(320, 170);
        view.render(AREA, &mut fb_before).unwrap();

        store.borrow_mut().apply_sync_ok(vec![item("GitHub")]);

        let mut fb_after = FrameBuffer565::new(320, 170);
        view.render(AREA, &mut fb_after).unwrap();

        let before: Vec<Rgb565> = fb_before.pixels().map(|p| p.1).collect();
        let after: Vec<Rgb565> = fb_after.pixels().map(|p| p.1).collect();
        assert_ne!(before, after, "the newly synced item must be visible on the very next render");
    }

    #[test]
    fn activate_without_a_registered_callback_is_a_documented_noop() {
        let mut view = CredentialListView::new(store_with(vec![item("GitHub")]));
        let action = view.on_focus(FocusEvent::Activated);
        assert!(matches!(action, Action::None));
    }

    #[test]
    fn activate_with_a_registered_callback_invokes_it_with_the_selected_id() {
        let item = item("GitHub");
        let expected_id = item.id;
        let store = store_with(vec![item]);
        let mut view = CredentialListView::new(store).on_activate(move |id| {
            assert_eq!(id, expected_id);
            Action::PopView
        });
        let action = view.on_focus(FocusEvent::Activated);
        assert!(matches!(action, Action::PopView));
    }

    #[test]
    fn render_reserves_a_right_edge_scrollbar_gutter_for_a_long_list() {
        let items: Vec<VaultItem> = (0..30).map(|i| item(&format!("item-{i}"))).collect();
        let view = CredentialListView::new(store_with(items));
        let mut fb = FrameBuffer565::new(320, 170);
        // A tall-enough list that it overflows the viewport (30 rows, well
        // over what a ~150px content area fits) should paint a thumb
        // somewhere in the rightmost `SCROLLBAR_WIDTH` columns.
        view.render(AREA, &mut fb).unwrap();

        let scrollbar_x = (AREA.size.width - 1) as i32;
        let any_scrollbar_pixel = (AREA.top_left.y..(AREA.top_left.y + AREA.size.height as i32))
            .map(|y| fb.pixel(Point::new(scrollbar_x, y)))
            .any(|color| color == palette::DIVIDER || color == palette::TEXT_SECONDARY);
        assert!(any_scrollbar_pixel, "a long list should paint a scrollbar track/thumb at the right edge");
    }

    #[test]
    fn render_does_not_panic_for_a_small_viewport_with_more_rows_than_fit() {
        let items: Vec<VaultItem> = (0..50).map(|i| item(&format!("item-{i}"))).collect();
        let view = CredentialListView::new(store_with(items));
        let mut fb = FrameBuffer565::new(64, 40);
        let area = Rectangle::new(Point::new(0, 0), Size::new(64, 40));
        view.render(area, &mut fb).unwrap();
    }
}
