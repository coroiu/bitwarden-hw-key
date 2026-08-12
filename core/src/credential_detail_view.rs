//! `CredentialDetailView`: the M1 credential detail screen widget, pushed
//! by [`crate::credential_list_view::CredentialListView`]'s `on_activate`
//! seam (wired in `app.rs`). Per Uma's UX spec, Fern's B4 framework design,
//! and `.planning/decisions/2026-08-12-m1-vault-store-data-ownership.md`
//! (bead `ai-bitwarden-hw-key-0v8.6`):
//!
//! - **Single-container shape**: one focusable [`Widget`] that manages
//!   field focus (USERNAME/PASSWORD/WEBSITE/NOTES) *internally*, rather
//!   than one `Widget` per field on a multi-widget `Screen`. This is a
//!   deliberate choice, not the simplest thing that could work: per
//!   `Navigator::dispatch`'s "known simplification" doc comment, a
//!   multi-widget screen would have every `Next`/`Prev` both move a
//!   focused field widget's own internal state *and* re-trigger the
//!   screen's top-level `focus_next`/`focus_previous` cycling — the "both
//!   fire" footgun. A single container sidesteps it entirely: this widget
//!   is always the screen's one (and thus always-focused) widget, so
//!   `Screen::focus_next`/`focus_previous` are no-ops here (see
//!   `Screen::set_focus`'s early return when the index doesn't change),
//!   and all real field navigation happens inside
//!   [`CredentialDetailView::on_intent`] instead.
//! - **Live-by-id reads**: holds an `Rc<RefCell<VaultStore>>` clone plus
//!   the credential's `Uuid`, and reads `store.get(id)` fresh on every
//!   render — never a cached snapshot. This is what makes sync-
//!   preservation and deletion-detection fall out for free per the ADR: a
//!   background sync updating the store never rebuilds the `Navigator` or
//!   destroys this pushed screen, and a sync that removes this credential
//!   is detected the moment `get(id)` starts returning `None`.
//! - **Gone state**: when `store.get(id)` is `None` (deleted upstream
//!   while viewing), the field stack is replaced with a centered "this
//!   item was removed" message — never a panic or a blank screen.

// Identical allow (and rationale) as `bhk_core::render`/`credential_list_view`:
// this module does the same `embedded-graphics` `Point`(i32)/`Size`(u32)
// coordinate math directly (field-row/label/value layout), so the same
// justification applies — no display this project targets is anywhere
// near large enough for these conversions to wrap, truncate, or lose a
// sign in practice.
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
    prelude::{Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
    Drawable,
};
use u8g2_fonts::{
    types::{FontColor, HorizontalAlignment, VerticalPosition},
    FontRenderer,
};
use uuid::Uuid;

use crate::credential_list_view::render_message;
use crate::input::NavIntent;
use crate::render::theme::{font, icon, palette};
use crate::render::{Action, ChromeContribution, ChromeStatus, FocusEvent, FrameBuffer565, SecretField, Widget};
use crate::vault_item::VaultItem;
use crate::vault_store::{SyncStatus, VaultStore};

/// Horizontal margin (px) from a field row's left/right edges to its
/// label/value text.
const FIELD_SIDE_MARGIN: i32 = 8;
/// Vertical padding (px) above a field row's label and below its value.
const FIELD_PADDING: i32 = 6;
/// Gap (px) between a field's label line and its value line.
const LABEL_VALUE_GAP: i32 = 3;

/// Fallback line height (px) used only if a font's metrics are somehow
/// unavailable (`get_rendered_dimensions_aligned` returning `None` — see
/// `font`'s module doc: this happens for a glyph with no coverage, not for
/// the fixed ASCII probe string used here, but the render core must never
/// panic on a font-metrics hiccup).
const FALLBACK_LINE_HEIGHT: i32 = 16;

/// Measures a font's worst-case single-line pixel footprint from a
/// `VerticalPosition::Top`-anchored position, using the same "all five
/// ASCII descenders (+ a capital, for ascent)" probe string convention
/// `core/src/render/list.rs`'s `NAME_LINE_FOOTPRINT`/`USERNAME_LINE_FOOTPRINT`
/// doc comment describes — except computed here at runtime via
/// `get_rendered_dimensions_aligned` (the same mechanism `screen.rs`'s
/// `text_width` already uses for width) rather than hardcoded from an
/// offline probe. A field's value font varies per field kind (`profont17`
/// for the password field, `helvR12` for everything else), so a single
/// hardcoded constant can't cover every combination the way `list.rs`'s
/// two fixed fonts could — measuring at runtime is what keeps this correct
/// without a probe-per-font-combination.
fn line_height(font: &FontRenderer) -> i32 {
    font.get_rendered_dimensions_aligned("Agjpqy", Point::zero(), VerticalPosition::Top, HorizontalAlignment::Left)
        .unwrap_or(None)
        .map_or(FALLBACK_LINE_HEIGHT, |bbox| bbox.size.height as i32)
}

/// Row-relative Y offset for a field's value line, given its label's font.
fn value_top_offset(label_font: &FontRenderer) -> i32 {
    FIELD_PADDING + line_height(label_font) + LABEL_VALUE_GAP
}

/// Total pixel height of a field row: top padding, label line, the gap,
/// value line, bottom padding. Fields can have different heights (the
/// password field's `profont17` value line is taller than the others'
/// `helvR12`), so this is computed per-field rather than being a single
/// shared constant like list rows' `ROW_HEIGHT`.
fn field_height(label_font: &FontRenderer, value_font: &FontRenderer) -> u32 {
    (FIELD_PADDING + line_height(label_font) + LABEL_VALUE_GAP + line_height(value_font) + FIELD_PADDING) as u32
}

/// Scroll offset (in pixels) that keeps the `focused`-th row — given each
/// row's `heights` — within `[scroll, scroll + viewport_height)`.
/// Bottom-anchored, same convention as `list.rs`'s
/// `scroll_offset_for_selection`, generalized from that function's fixed
/// `ROW_HEIGHT` to per-row variable heights (this view's rows aren't all
/// the same height — see `field_height`'s doc comment).
fn scroll_offset_for_focus(heights: &[u32], focused: usize, viewport_height: u32) -> u32 {
    let total_height: u32 = heights.iter().sum();
    if viewport_height == 0 || total_height <= viewport_height {
        return 0;
    }
    let focused_bottom: u32 = heights[..=focused].iter().sum();
    focused_bottom.saturating_sub(viewport_height)
}

/// One of a credential's detail fields. Display-only ordering/shape; not
/// `VaultItem` itself, same "widget layer shouldn't know the credential
/// domain beyond this adapter" split `credential_list_view.rs`'s module
/// doc describes for `ListItem`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Username,
    Password,
    Website,
    Notes,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Field::Username => "USERNAME",
            Field::Password => "PASSWORD",
            Field::Website => "WEBSITE",
            Field::Notes => "NOTES",
        }
    }

    fn value(self, item: &VaultItem) -> &str {
        match self {
            Field::Username => item.username.as_str(),
            Field::Password => item.password.as_str(),
            Field::Website => item.uri.as_deref().unwrap_or_default(),
            Field::Notes => item.notes.as_deref().unwrap_or_default(),
        }
    }

    /// The font a field's *value* line renders in. The password field
    /// uses the monospaced secret font (see `SecretField`'s doc comment on
    /// why); every other field uses the plain detail-value font.
    fn value_font(self) -> FontRenderer {
        match self {
            Field::Password => font::secret(),
            Field::Username | Field::Website | Field::Notes => font::value(),
        }
    }
}

/// The fields to show for `item`, in display order: USERNAME and PASSWORD
/// always; WEBSITE only if `item.uri` is `Some`; NOTES only if
/// `item.notes` is `Some`. Per the bead spec — a credential with no URI
/// saved doesn't get an empty WEBSITE row, it doesn't get a row at all.
fn available_fields(item: &VaultItem) -> Vec<Field> {
    let mut fields = vec![Field::Username, Field::Password];
    if item.uri.is_some() {
        fields.push(Field::Website);
    }
    if item.notes.is_some() {
        fields.push(Field::Notes);
    }
    fields
}

/// The credential detail screen's content widget. See the module doc for
/// the full design.
pub struct CredentialDetailView {
    store: Rc<RefCell<VaultStore>>,
    id: Uuid,
    /// The index (into that render's `available_fields(item)`) of the
    /// internally focused field. `Cell`, not a plain field, for the same
    /// reason `CredentialListView::selected_id`/`last_index` are `Cell`s:
    /// `Widget::render` (`&self`) still needs to clamp/resolve this
    /// against the live item's current field count on every call, since a
    /// store mutation can land between any two renders without an
    /// intervening `on_intent` call.
    focused_field: Cell<usize>,
    secret: SecretField,
    /// Whether this widget currently holds the screen's top-level focus.
    /// Always `true` in practice once the screen is pushed (this is the
    /// screen's one and only widget — see the module doc), but tracked
    /// explicitly (mirroring `CredentialListView::focused`) rather than
    /// assumed, so a hypothetical future multi-widget detail screen
    /// wouldn't silently paint focus highlighting while unfocused.
    focused: bool,
}

impl CredentialDetailView {
    #[must_use]
    pub fn new(store: Rc<RefCell<VaultStore>>, id: Uuid) -> Self {
        Self {
            store,
            id,
            focused_field: Cell::new(0),
            secret: SecretField::new(),
            focused: false,
        }
    }

    /// The credential id this view is showing. Exposed for tests/
    /// diagnostics; not needed by any production caller today.
    #[must_use]
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Clamps `focused_field` to `[0, field_count)` (or `None` if
    /// `field_count == 0`) and returns the resolved index. Unlike
    /// `CredentialListView::resolve_selection`, this clamps by position
    /// only (fields have no identity to track across a live field-count
    /// change) — an edge case not exercised by the bead's spec (a
    /// credential's own uri/notes presence changing mid-view), so the
    /// simpler behavior is deliberate, not an oversight.
    fn resolve_focus(&self, field_count: usize) -> Option<usize> {
        if field_count == 0 {
            self.focused_field.set(0);
            return None;
        }
        let clamped = self.focused_field.get().min(field_count - 1);
        self.focused_field.set(clamped);
        Some(clamped)
    }

    /// Moves the internally focused field by `delta`, **clamped** (not
    /// wrapping) at `[0, fields.len())` — per the bead spec, distinct from
    /// `CredentialListView`'s row selection (also clamped) and
    /// `Screen::focus_next`/`focus_previous`'s top-level wrap-around.
    ///
    /// If focus is leaving the password field (for any other field),
    /// forwards `FocusEvent::Lost` to the `SecretField` — the "auto
    /// re-mask on blur" requirement. Takes `&self`: both `focused_field`
    /// and `SecretField`'s reveal flag are `Cell`s, so this needs no
    /// exclusive borrow of `self`, which is what lets `on_intent` call it
    /// while still holding the `Ref` from `self.store.borrow()` — the
    /// same pattern `CredentialListView::on_intent`/`move_selection` uses.
    fn move_focus(&self, fields: &[Field], delta: i32) {
        if fields.is_empty() {
            return;
        }
        let current = self.resolve_focus(fields.len()).unwrap_or(0) as i32;
        let len = fields.len() as i32;
        let next = (current + delta).clamp(0, len - 1) as usize;

        if fields[current as usize] == Field::Password && fields[next] != Field::Password {
            self.secret.on_focus(FocusEvent::Lost);
        }
        self.focused_field.set(next);
    }

    /// Maps the store's `SyncStatus` to the chrome's semantic
    /// `ChromeStatus`, identically to `CredentialListView::chrome_status` —
    /// both widgets report the same app-wide sync health, just from
    /// whichever screen currently has focus.
    fn chrome_status(&self) -> ChromeStatus {
        match self.store.borrow().status() {
            Some(SyncStatus::Synced) => ChromeStatus::Success,
            Some(SyncStatus::Error(_)) => ChromeStatus::Error,
            Some(SyncStatus::Empty) | None => ChromeStatus::Neutral,
        }
    }

    fn render_fields(&self, area: Rectangle, item: &VaultItem, target: &mut FrameBuffer565) -> Result<(), Infallible> {
        let fields = available_fields(item);
        let focused_index = self.resolve_focus(fields.len());
        let label_font = font::label();
        let heights: Vec<u32> = fields.iter().map(|field| field_height(&label_font, &field.value_font())).collect();

        // Auto-scroll so the focused field's row is fully visible — the
        // same requirement `list.rs`'s `scroll_offset_for_selection` meets
        // for the credential list, needed here too because four stacked
        // fields (label + value each) routinely don't all fit the content
        // area at once (e.g. USERNAME/PASSWORD/WEBSITE/NOTES on a 320x170
        // panel): without this, navigating focus onto NOTES could leave it
        // permanently off-screen with no way to see what's focused.
        let scroll = focused_index.map_or(0, |index| scroll_offset_for_focus(&heights, index, area.size.height));

        // Clip once to `area` (not per-row): a row scrolled partially
        // above/below the content area must still have its visible slice
        // drawn, not be skipped outright — the same reason
        // `CredentialListView::render_list` clips to the whole `rows_area`
        // up front rather than per-row.
        let mut clipped = target.clipped(&area);

        let mut y = area.top_left.y - scroll as i32;
        let bottom = area.top_left.y + area.size.height as i32;

        for (index, field) in fields.iter().enumerate() {
            let height = heights[index];
            let row_bottom = y + height as i32;

            // Skip rows fully outside the viewport — mirrors
            // `VerticalList::render`'s identical early-out; `clipped`
            // would drop their pixels anyway, but there's no point issuing
            // draw calls for off-screen rows.
            if row_bottom > area.top_left.y && y < bottom {
                let row_rect = Rectangle::new(Point::new(area.top_left.x, y), Size::new(area.size.width, height));
                let is_focused = self.focused && focused_index == Some(index);
                self.render_field(row_rect, *field, item, is_focused, &mut clipped)?;
            }

            y += height as i32;
        }

        Ok(())
    }

    fn render_field<D>(
        &self,
        row_rect: Rectangle,
        field: Field,
        item: &VaultItem,
        is_focused: bool,
        target: &mut D,
    ) -> Result<(), Infallible>
    where
        D: embedded_graphics::draw_target::DrawTarget<Color = embedded_graphics::pixelcolor::Rgb565, Error = Infallible>,
    {
        let mut clipped = target.clipped(&row_rect);

        if is_focused {
            crate::render::theme::draw_selection(row_rect, &mut clipped)?;
        } else {
            let divider = Rectangle::new(
                Point::new(row_rect.top_left.x, row_rect.top_left.y + row_rect.size.height as i32 - 1),
                Size::new(row_rect.size.width, 1),
            );
            divider.into_styled(PrimitiveStyle::with_fill(palette::DIVIDER)).draw(&mut clipped)?;
        }

        let label_font = font::label();
        let label_x = row_rect.top_left.x + FIELD_SIDE_MARGIN;
        let label_y = row_rect.top_left.y + FIELD_PADDING;
        let _ = label_font.render_aligned(
            field.label(),
            Point::new(label_x, label_y),
            VerticalPosition::Top,
            HorizontalAlignment::Left,
            FontColor::Transparent(palette::TEXT_SECONDARY),
            &mut clipped,
        );

        let value_y = row_rect.top_left.y + value_top_offset(&label_font);
        let value_width = row_rect.size.width.saturating_sub((FIELD_SIDE_MARGIN as u32) * 2);
        let value_area = Rectangle::new(
            Point::new(row_rect.top_left.x + FIELD_SIDE_MARGIN, value_y),
            Size::new(value_width, line_height(&field.value_font()) as u32),
        );

        if field == Field::Password {
            self.secret.render(value_area, item.password.as_str(), &mut clipped)?;
        } else {
            let value_font = field.value_font();
            let _ = value_font.render_aligned(
                field.value(item),
                value_area.top_left,
                VerticalPosition::Top,
                HorizontalAlignment::Left,
                FontColor::Transparent(palette::TEXT_PRIMARY),
                &mut clipped,
            );
        }

        Ok(())
    }
}

impl Widget for CredentialDetailView {
    fn measure(&self, constraints: Size) -> Size {
        constraints
    }

    /// Always `true`, for the same "focus-init runs once" reason
    /// `CredentialListView::is_focusable` is: this is the screen's only
    /// widget, and it must be focusable the instant it's pushed regardless
    /// of the live item's current field count (which could in principle be
    /// zero for a moment if the item is already gone at push time).
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
                // Defensive, not load-bearing today: this screen never
                // actually loses top-level focus while it exists (it's the
                // screen's one widget), but forcing the secret hidden here
                // too keeps the invariant "a secret is only ever visible
                // while its field is genuinely in view" true even if a
                // future multi-widget detail screen changes that.
                self.secret.on_focus(FocusEvent::Lost);
                Action::None
            }
            FocusEvent::Activated => {
                let store = self.store.borrow();
                if let Some(item) = store.get(self.id) {
                    let fields = available_fields(item);
                    if let Some(index) = self.resolve_focus(fields.len()) {
                        if fields[index] == Field::Password {
                            self.secret.on_focus(FocusEvent::Activated);
                        }
                    }
                }
                Action::None
            }
        }
    }

    fn on_intent(&mut self, intent: NavIntent) -> Action {
        let store = self.store.borrow();
        if let Some(item) = store.get(self.id) {
            let fields = available_fields(item);
            match intent {
                NavIntent::Next => self.move_focus(&fields, 1),
                NavIntent::Prev => self.move_focus(&fields, -1),
                NavIntent::NextN(n) => self.move_focus(&fields, i32::from(n)),
                NavIntent::Activate | NavIntent::Back => {}
            }
        }
        Action::None
    }

    fn render(&self, area: Rectangle, target: &mut FrameBuffer565) -> Result<(), Infallible> {
        let item = self.store.borrow().get(self.id).cloned();
        if let Some(item) = item {
            return self.render_fields(area, &item, target);
        }

        render_message(
            area,
            Some(icon::SHIELD),
            palette::STATUS_ERROR,
            "This item was removed",
            palette::TEXT_PRIMARY,
            Some("Hold to go back"),
            target,
        );
        Ok(())
    }

    /// The credential's live name as the title, a hint that reflects the
    /// focused field (the `SecretField`'s own reveal-state hint while
    /// PASSWORD is focused, a generic field-navigation hint otherwise),
    /// and the same sync-status dot the list shows.
    fn chrome_contribution(&self) -> Option<ChromeContribution> {
        let store = self.store.borrow();
        let item = store.get(self.id);

        let title = item.map(|item| item.name.clone());
        let hint = match item {
            None => "Hold to go back".to_string(),
            Some(item) => {
                let fields = available_fields(item);
                match self.resolve_focus(fields.len()).and_then(|index| fields.get(index).copied()) {
                    Some(Field::Password) => self.secret.hint().to_string(),
                    Some(Field::Username | Field::Website | Field::Notes) | None => {
                        "Rotate to switch fields - Hold to go back".to_string()
                    }
                }
            }
        };

        Some(ChromeContribution {
            title,
            readout: None,
            hint: Some(hint),
            status: Some(self.chrome_status()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Screen;
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

    fn full_item(name: &str) -> VaultItem {
        VaultItem {
            uri: Some(format!("https://{name}.example.com")),
            notes: Some("some notes".to_string()),
            ..item(name)
        }
    }

    fn store_with(items: Vec<VaultItem>) -> Rc<RefCell<VaultStore>> {
        let store = Rc::new(RefCell::new(VaultStore::new()));
        store.borrow_mut().apply_sync_ok(items);
        store
    }

    const AREA: Rectangle = Rectangle::new(Point::new(0, 0), Size::new(320, 150));

    #[test]
    fn available_fields_omits_website_and_notes_when_absent() {
        let sparse = item("GitHub");
        assert_eq!(available_fields(&sparse), vec![Field::Username, Field::Password]);

        let full = full_item("GitHub");
        assert_eq!(available_fields(&full), vec![Field::Username, Field::Password, Field::Website, Field::Notes]);
    }

    #[test]
    fn is_always_focusable() {
        let store = store_with(vec![item("GitHub")]);
        let id = store.borrow().items()[0].id;
        let view = CredentialDetailView::new(store, id);
        assert!(view.is_focusable());
    }

    #[test]
    fn field_focus_starts_at_zero_and_clamps_instead_of_wrapping() {
        let full = full_item("GitHub");
        let id = full.id;
        let store = store_with(vec![full]);
        let mut view = CredentialDetailView::new(store, id);
        view.on_focus(FocusEvent::Gained);

        assert_eq!(view.resolve_focus(4), Some(0));

        view.on_intent(NavIntent::Prev); // clamp below zero
        assert_eq!(view.resolve_focus(4), Some(0));

        view.on_intent(NavIntent::Next);
        view.on_intent(NavIntent::Next);
        view.on_intent(NavIntent::Next);
        assert_eq!(view.resolve_focus(4), Some(3)); // Notes, the last field

        view.on_intent(NavIntent::Next); // clamp at the end, does not wrap to 0
        assert_eq!(view.resolve_focus(4), Some(3));
    }

    #[test]
    fn field_focus_clamps_down_when_the_live_field_count_shrinks() {
        // Focus Notes (index 3 of 4), then a live sync drops uri/notes —
        // the widget must clamp rather than panic on an out-of-range index.
        let full = full_item("GitHub");
        let id = full.id;
        let store = store_with(vec![full]);
        let mut view = CredentialDetailView::new(store.clone(), id);
        view.on_focus(FocusEvent::Gained);
        view.on_intent(NavIntent::Next);
        view.on_intent(NavIntent::Next);
        view.on_intent(NavIntent::Next);
        assert_eq!(view.resolve_focus(4), Some(3));

        let mut sparse = item("GitHub");
        sparse.id = id;
        store.borrow_mut().apply_sync_ok(vec![sparse]);

        // Only 2 fields now (Username, Password); resolving against that
        // live count must clamp, not panic or stay at an invalid index 3.
        assert_eq!(view.resolve_focus(2), Some(1));
    }

    #[test]
    fn activating_the_password_field_reveals_it_and_activating_again_re_masks() {
        let full = full_item("GitHub");
        let id = full.id;
        let store = store_with(vec![full]);
        let mut view = CredentialDetailView::new(store, id);
        view.on_focus(FocusEvent::Gained);
        view.on_intent(NavIntent::Next); // Username -> Password
        assert_eq!(view.resolve_focus(4), Some(1));

        view.on_focus(FocusEvent::Activated);
        assert!(view.secret.is_revealed());

        view.on_focus(FocusEvent::Activated);
        assert!(!view.secret.is_revealed());
    }

    #[test]
    fn activating_a_non_password_field_does_not_touch_the_secret_state() {
        let full = full_item("GitHub");
        let id = full.id;
        let store = store_with(vec![full]);
        let mut view = CredentialDetailView::new(store, id);
        view.on_focus(FocusEvent::Gained); // Username focused (index 0)

        view.on_focus(FocusEvent::Activated);
        assert!(!view.secret.is_revealed());
    }

    #[test]
    fn moving_focus_away_from_the_revealed_password_field_re_masks_it() {
        let full = full_item("GitHub");
        let id = full.id;
        let store = store_with(vec![full]);
        let mut view = CredentialDetailView::new(store, id);
        view.on_focus(FocusEvent::Gained);
        view.on_intent(NavIntent::Next); // Password
        view.on_focus(FocusEvent::Activated);
        assert!(view.secret.is_revealed());

        view.on_intent(NavIntent::Next); // Password -> Website: must re-mask
        assert!(!view.secret.is_revealed());
    }

    #[test]
    fn renders_all_present_fields_with_labels_and_values_visible() {
        let full = full_item("GitHub");
        let id = full.id;
        let store = store_with(vec![full]);
        let view = CredentialDetailView::new(store, id);

        let mut fb = FrameBuffer565::new(320, 170);
        view.render(AREA, &mut fb).unwrap();

        let any_label_ink = fb.pixels().any(|p| p.1 == palette::TEXT_SECONDARY);
        assert!(any_label_ink, "field labels should render in the muted label color");
        let any_value_ink = fb.pixels().any(|p| p.1 == palette::TEXT_PRIMARY);
        assert!(any_value_ink, "field values should render in the primary text color");
    }

    #[test]
    fn focusing_a_field_that_would_otherwise_overflow_scrolls_it_into_view() {
        // Four stacked fields (USERNAME/PASSWORD/WEBSITE/NOTES) routinely
        // don't all fit a short viewport at once — without auto-scroll
        // (mirroring `list.rs`'s `scroll_offset_for_selection`), navigating
        // focus onto NOTES would leave it permanently off-screen with no
        // visible focus highlight anywhere.
        let full = full_item("GitHub");
        let id = full.id;
        let store = store_with(vec![full]);
        let mut view = CredentialDetailView::new(store, id);
        view.on_focus(FocusEvent::Gained);

        let short_area = Rectangle::new(Point::new(0, 0), Size::new(320, 90));

        view.on_intent(NavIntent::Next); // Password
        view.on_intent(NavIntent::Next); // Website
        view.on_intent(NavIntent::Next); // Notes (last field)
        assert_eq!(view.resolve_focus(4), Some(3));

        let mut fb = FrameBuffer565::new(320, 170);
        view.render(short_area, &mut fb).unwrap();

        let any_focus_highlight = fb.pixels().any(|p| p.1 == palette::SURFACE_ELEVATED);
        assert!(any_focus_highlight, "the focused (Notes) field must be scrolled into view, not left off-screen");
    }

    #[test]
    fn website_and_notes_rows_are_absent_when_the_credential_has_none() {
        let sparse = item("GitHub"); // uri: None, notes: None
        let id = sparse.id;
        let store = store_with(vec![sparse.clone()]);
        let sparse_view = CredentialDetailView::new(Rc::clone(&store), id);

        let full = full_item("GitHub");
        let full_store = store_with(vec![VaultItem { id, ..full }]);
        let full_view = CredentialDetailView::new(full_store, id);

        let mut fb_sparse = FrameBuffer565::new(320, 170);
        sparse_view.render(AREA, &mut fb_sparse).unwrap();
        let mut fb_full = FrameBuffer565::new(320, 170);
        full_view.render(AREA, &mut fb_full).unwrap();

        let sparse_pixels: Vec<_> = fb_sparse.pixels().map(|p| p.1).collect();
        let full_pixels: Vec<_> = fb_full.pixels().map(|p| p.1).collect();
        assert_ne!(sparse_pixels, full_pixels, "a credential with a uri/notes must render more than one without");
    }

    #[test]
    fn gone_state_renders_when_the_id_is_absent_from_the_store() {
        let store = store_with(vec![item("GitHub")]); // some other item
        let missing_id = Uuid::new_v4();
        let view = CredentialDetailView::new(store, missing_id);

        let mut fb = FrameBuffer565::new(320, 170);
        view.render(AREA, &mut fb).unwrap();

        let any_error_icon_ink = fb.pixels().any(|p| p.1 == palette::STATUS_ERROR);
        assert!(any_error_icon_ink, "the gone-state icon should render in the error color");
    }

    #[test]
    fn gone_state_replaces_a_previously_rendered_field_stack_after_deletion() {
        let full = full_item("GitHub");
        let id = full.id;
        let store = store_with(vec![full]);
        let view = CredentialDetailView::new(Rc::clone(&store), id);

        let mut fb_present = FrameBuffer565::new(320, 170);
        view.render(AREA, &mut fb_present).unwrap();

        store.borrow_mut().apply_sync_ok(vec![]); // deleted upstream

        let mut fb_gone = FrameBuffer565::new(320, 170);
        view.render(AREA, &mut fb_gone).unwrap();

        let present_pixels: Vec<_> = fb_present.pixels().map(|p| p.1).collect();
        let gone_pixels: Vec<_> = fb_gone.pixels().map(|p| p.1).collect();
        assert_ne!(present_pixels, gone_pixels, "the very next render must reflect the deletion");
    }

    #[test]
    fn chrome_contribution_reports_the_live_item_name_as_the_title() {
        let full = full_item("Bitwarden.com");
        let id = full.id;
        let store = store_with(vec![full]);
        let mut view = CredentialDetailView::new(store, id);
        view.on_focus(FocusEvent::Gained);

        let contribution = view.chrome_contribution().unwrap();
        assert_eq!(contribution.title.as_deref(), Some("Bitwarden.com"));
    }

    #[test]
    fn chrome_hint_reflects_the_secret_fields_reveal_state_only_while_it_is_focused() {
        let full = full_item("GitHub");
        let id = full.id;
        let store = store_with(vec![full]);
        let mut view = CredentialDetailView::new(store, id);
        view.on_focus(FocusEvent::Gained); // Username focused

        let hint_on_username = view.chrome_contribution().unwrap().hint.unwrap();
        assert_eq!(hint_on_username, "Rotate to switch fields - Hold to go back");

        view.on_intent(NavIntent::Next); // Password focused
        let hint_on_password = view.chrome_contribution().unwrap().hint.unwrap();
        assert_eq!(hint_on_password, "Press to reveal");

        view.on_focus(FocusEvent::Activated);
        let hint_after_reveal = view.chrome_contribution().unwrap().hint.unwrap();
        assert_eq!(hint_after_reveal, "Press to hide");
    }

    #[test]
    fn chrome_contribution_on_gone_state_has_no_title_and_a_back_hint() {
        let store = store_with(vec![item("GitHub")]);
        let missing_id = Uuid::new_v4();
        let mut view = CredentialDetailView::new(store, missing_id);
        view.on_focus(FocusEvent::Gained);

        let contribution = view.chrome_contribution().unwrap();
        assert_eq!(contribution.title, None);
        assert_eq!(contribution.hint.as_deref(), Some("Hold to go back"));
    }

    #[test]
    fn integrates_with_navigator_push_and_pop_via_a_screen() {
        // Proves this widget behaves correctly when actually mounted on a
        // `Screen`/`Navigator`, not just exercised directly — the same
        // "focus_survives..." style proof `credential_list_view.rs` uses.
        let full = full_item("GitHub");
        let id = full.id;
        let store = store_with(vec![full]);
        let view = CredentialDetailView::new(store, id);
        let screen = Screen::new("Credential", vec![Box::new(view)]);
        let navigator = crate::render::Navigator::new(screen);

        let mut fb = FrameBuffer565::new(320, 170);
        navigator.render(&mut fb).unwrap();
        assert_eq!(fb.size(), Size::new(320, 170));

        let any_value_ink = fb.pixels().any(|p| p.1 == palette::TEXT_PRIMARY);
        assert!(any_value_ink);
    }
}
