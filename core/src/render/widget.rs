//! The retained-mode `Widget` trait and the two return-value vocabularies
//! it uses: `Action` (what the navigation stack should do next) and
//! `FocusEvent` (what happened to a widget's focus state).
//!
//! Salvaged concepts, reimplemented cleanly on `embedded-graphics` (see
//! `.planning/decisions/2026-08-11-ui-framework-reuse-vs-rewrite.md`):
//! - `simple_gui::components::Component` -> [`Widget`]
//! - `simple_gui::components::ComponentAction` -> [`Action`]
//! - `simple_gui::components::FocusEvent` -> [`FocusEvent`] (unchanged
//!   shape; already formalized upstream in
//!   `.planning/decisions/2026-01-21-focus-management-system.md`)
//!
//! `Widget::render` takes a concrete `&mut FrameBuffer565` rather than a
//! generic `&mut impl DrawTarget`. This is deliberate, not a simplification
//! we'd like to undo later: `embedded_graphics::draw_target::DrawTarget`
//! has generic methods (`fill_contiguous`, `draw_iter`, ...), so it is not
//! object-safe, and `Box<dyn Widget>` (needed for a heterogeneous screen
//! stack) requires every trait method to be dyn-compatible. A widget that
//! wants real clipping still gets it — it calls
//! `target.clipped(&some_sub_area)` *inside* its own `render`, using the
//! `area` it was handed, per `DrawTargetExt::clipped()`.

use std::convert::Infallible;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{Primitive, Size};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::Drawable;

use crate::input::NavIntent;

use super::framebuffer::FrameBuffer565;
use super::screen::Screen;

/// High-level focus state transitions, decoupled from whatever transport
/// triggered them (encoder, keyboard, headless HTTP injection — see
/// `.planning/decisions/2026-08-11-rotary-encoder-input-model.md`). The
/// `Navigator` fires these on a widget when its focus state changes or when
/// it is activated while focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusEvent {
    /// The widget gained focus (e.g. the user navigated to it).
    Gained,
    /// The widget lost focus (e.g. the user navigated away).
    Lost,
    /// The widget was activated while focused (encoder short press, Enter,
    /// headless `Activate` intent).
    Activated,
}

/// What a widget wants the navigation stack to do in response to a
/// `FocusEvent` or `NavIntent`. Returned rather than mutating the stack
/// directly, so widgets never need a reference back to the `Navigator`
/// that owns them.
#[derive(Default)]
pub enum Action {
    /// Push a new screen. Boxed as `FnOnce` (not `Fn`): a push is a single
    /// one-shot construction, not a repeatable template.
    PushView(Box<dyn FnOnce() -> Screen>),
    /// Pop the current screen (e.g. "selection complete, return to the
    /// list that opened me").
    PopView,
    /// Semantic back-navigation, as distinct from `PopView`: a widget may
    /// want to signal "back" for reasons other than "I'm done" (e.g.
    /// cancelling an in-progress action). The `Navigator` currently treats
    /// both identically (pop the stack); kept as a separate variant per
    /// the frozen `Action` shape so the two intents don't have to be
    /// conflated if a future widget needs to distinguish them.
    Back,
    /// No navigation-stack action.
    #[default]
    None,
}

/// Width, in pixels, of the left accent bar [`draw_focus_block`] draws on
/// top of its full-area fill.
pub const FOCUS_ACCENT_WIDTH: u32 = 3;

/// Draws the shared "this is the focused thing" visual: a full-`area` fill
/// in `fill_color`, then a [`FOCUS_ACCENT_WIDTH`]-px accent bar in
/// `accent_color` along `area`'s left edge.
///
/// Extracted here (rather than inlined once in `credential_list_view.rs`)
/// because it is deliberately *not* list-row-specific: `CredentialListView`
/// uses it for a focused row per bead `ai-bitwarden-hw-key-0v8.4`, and bead
/// `ai-bitwarden-hw-key-0v8.6`'s detail view is expected to reuse it for a
/// focused field, so the "focused" visual language stays identical across
/// both screens rather than drifting into two hand-rolled variants.
///
/// Generic over `D: DrawTarget<Color = Rgb565, Error = Infallible>` (rather
/// than the concrete `FrameBuffer565`) so a caller drawing into a
/// `DrawTargetExt::clipped()` sub-region (as `CredentialListView` does, to
/// clip a row to its list's content width) can pass that clipped target
/// directly, the same way `Drawable::draw` calls do elsewhere in this
/// module.
///
/// # Errors
///
/// Returns `Infallible`'s uninhabited variant in practice — see
/// `Widget::render`'s doc comment for why the `Result` return exists at
/// all.
pub fn draw_focus_block<D>(
    area: Rectangle,
    fill_color: Rgb565,
    accent_color: Rgb565,
    target: &mut D,
) -> Result<(), Infallible>
where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    area.into_styled(PrimitiveStyle::with_fill(fill_color)).draw(target)?;

    let accent_width = FOCUS_ACCENT_WIDTH.min(area.size.width);
    let accent = Rectangle::new(area.top_left, Size::new(accent_width, area.size.height));
    accent.into_styled(PrimitiveStyle::with_fill(accent_color)).draw(target)?;

    Ok(())
}

/// A retained-mode UI element. Implementors own their own state (selection
/// index, scroll offset, ...) and are told their assigned screen-space
/// `area` at render/measure time — nothing in a `Widget` impl should assume
/// or hardcode a specific display resolution.
pub trait Widget {
    /// Reports how much space this widget wants, given the space on offer.
    /// Screens use this to stack widgets vertically in the content region
    /// (see `Screen::render`); a widget is free to request less than
    /// `constraints` (e.g. a single-line label) or all of it (e.g. a list
    /// that should fill the remaining content area).
    fn measure(&self, constraints: Size) -> Size;

    /// Draws into `target`, constrained to `area`. Implementations that
    /// need to guard against overdraw (text overflow, an oversized row)
    /// should call `target.clipped(&area)` (or a sub-rectangle of it) and
    /// draw into that, per `DrawTargetExt::clipped()` — this is the *real*
    /// clipping mechanism the old character-skip marquee code is retired
    /// in favor of.
    ///
    /// # Errors
    ///
    /// Returns `Infallible`'s uninhabited variant in practice: the core's
    /// `DrawTarget` (`FrameBuffer565`) can never fail to draw. The `Result`
    /// return exists only to match `Drawable`/`DrawTarget`'s signature so
    /// widget impls can use `?` freely when calling into embedded-graphics
    /// primitives.
    fn render(&self, area: Rectangle, target: &mut FrameBuffer565) -> Result<(), Infallible>;

    /// Whether this widget can receive focus. Defaults to `false` (e.g.
    /// static labels, dividers).
    fn is_focusable(&self) -> bool {
        false
    }

    /// Called by the `Navigator` when this widget's focus state changes,
    /// or when it is activated while focused. Only meaningful if
    /// `is_focusable()` is `true`.
    fn on_focus(&mut self, _event: FocusEvent) -> Action {
        Action::None
    }

    /// Called by the `Navigator` with `Next`/`Prev`/`NextN` while this
    /// widget is focused, giving it a chance to react internally (e.g. a
    /// list moving its selected row) before/alongside the `Navigator`'s
    /// own top-level focus cycling — see `Navigator::dispatch` for the
    /// exact interleaving and its known limitation for multi-widget
    /// screens.
    fn on_intent(&mut self, _intent: NavIntent) -> Action {
        Action::None
    }
}
