//! `SecretField`: masked/revealed rendering + reveal-state for a password
//! value, matching the approved M1 detail-view mockup (screens 02
//! "masked" / 03 "revealed"). Per Uma (UX) + Fern (B4); bead
//! `ai-bitwarden-hw-key-0v8.6`.
//!
//! A generic rendering primitive (like [`super::list::draw_row`] or
//! [`super::theme::draw_selection`]), not itself a [`super::widget::Widget`]
//! — the credential domain that owns the field ([`crate::credential_detail_view::CredentialDetailView`])
//! decides *when* this field is focused/activated and forwards that as a
//! [`FocusEvent`]; `SecretField` only owns the reveal/hide toggle and how
//! to draw it.
//!
//! Reusing [`FocusEvent`] here — a type whose other use is `Widget`-level
//! top-of-screen focus — for an *internal* field-focus concept is
//! deliberate, not a layering violation: `CredentialDetailView`'s field
//! navigation is internal to a single focusable widget (see that module's
//! doc for why), so there is no separate "internal focus" vocabulary to
//! reuse instead. `Gained`/`Lost`/`Activated` already mean exactly what's
//! needed ("this field just became the one with the user's attention" /
//! "it just stopped being that" / "the user pressed while it had
//! attention"), so introducing a parallel enum would only add a
//! translation layer with no new information.

use std::cell::Cell;
use std::convert::Infallible;

use embedded_graphics::draw_target::{DrawTarget, DrawTargetExt};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{Point, Size};
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

use super::theme::{font, icon, palette};
use super::widget::FocusEvent;

/// Number of mask glyphs drawn when hidden — fixed, *not* derived from the
/// real password's length. A masked secret must never leak how long the
/// real value is via the number of dots on screen; a fixed count is what
/// makes that true regardless of the credential's actual password length.
pub const MASK_GLYPH_COUNT: usize = 10;

/// The glyph repeated [`MASK_GLYPH_COUNT`] times when hidden. `*`, not a
/// bullet/dot codepoint — [`font::secret`]'s `profont17` has no bullet
/// glyph (see that accessor's doc comment on why `profont` was chosen:
/// monospacing, not glyph coverage), and `*` is available and reads
/// unambiguously as "masked" in a monospaced font.
const MASK_GLYPH: char = '*';

/// Horizontal space reserved at the field's right edge for the lock icon
/// (`~16px` per [`font::icon_2x`]'s doc comment, plus a small margin) —
/// subtracted from the value text's clip width so a long revealed password
/// can't draw underneath/through the icon.
const ICON_RESERVED_WIDTH: u32 = 22;

/// A password/secret detail field: owns only the reveal/hide toggle state.
/// See the module doc for the full design (why it reuses [`FocusEvent`],
/// why the mask is fixed-length).
#[derive(Debug, Default)]
pub struct SecretField {
    revealed: Cell<bool>,
}

impl SecretField {
    #[must_use]
    pub fn new() -> Self {
        Self { revealed: Cell::new(false) }
    }

    /// Whether the real value is currently showing (as opposed to masked).
    #[must_use]
    pub fn is_revealed(&self) -> bool {
        self.revealed.get()
    }

    /// Reacts to a focus-lifecycle event for this field, as forwarded by
    /// whatever owns it:
    /// - `Activated`: toggles reveal <-> mask (re-`Activate`ing a revealed
    ///   field re-masks it, per the bead spec).
    /// - `Lost`: forces the field back to hidden — the "auto re-mask on
    ///   blur" requirement. A secret should only ever be visible while the
    ///   user is deliberately looking at *this* field.
    /// - `Gained`: a no-op. A field always starts hidden when focus
    ///   arrives (it never "remembers" being revealed from a previous
    ///   visit — `Lost` already cleared that on the way out).
    ///
    /// Takes `&self` (not `&mut self`) — the reveal flag is a `Cell`, so
    /// this can be called from a caller's `&self` context (e.g. a
    /// `Widget::render`-time re-resolution), the same reason
    /// `CredentialListView`'s `selected_id`/`last_index` are `Cell`s
    /// rather than plain fields.
    pub fn on_focus(&self, event: FocusEvent) {
        match event {
            FocusEvent::Activated => self.revealed.set(!self.revealed.get()),
            FocusEvent::Lost => self.revealed.set(false),
            FocusEvent::Gained => {}
        }
    }

    /// The contextual hint text for the current reveal state, per Uma's
    /// spec: "Press to reveal" while masked, "Press to hide" once revealed.
    #[must_use]
    pub fn hint(&self) -> &'static str {
        if self.revealed.get() {
            "Press to hide"
        } else {
            "Press to reveal"
        }
    }

    /// Draws this field's value line — the masked-dots-or-real-password
    /// text plus the lock status icon — into `area`. Does not draw a
    /// label or any focus/selection background; that's the caller's job
    /// (mirrors [`super::list::draw_row`]'s label-less counterparts and
    /// [`super::theme::draw_selection`]'s "just the fill, decorations are
    /// the caller's job" split).
    ///
    /// Generic over `D: DrawTarget<Color = Rgb565, Error = Infallible>`
    /// (not the concrete `FrameBuffer565`) so a caller drawing into a
    /// `DrawTargetExt::clipped()` sub-region (e.g. a focused field's row
    /// clip) can pass that clipped target directly.
    ///
    /// # Errors
    ///
    /// Returns `Infallible`'s uninhabited variant in practice — see
    /// [`super::widget::Widget::render`]'s doc comment for why the
    /// `Result` return exists at all.
    pub fn render<D>(&self, area: embedded_graphics::primitives::Rectangle, password: &str, target: &mut D) -> Result<(), Infallible>
    where
        D: DrawTarget<Color = Rgb565, Error = Infallible>,
    {
        let icon_font = font::icon_2x();
        let icon_char = if self.is_revealed() { icon::LOCK_UNLOCKED } else { icon::LOCK_LOCKED };
        // Muted while masked ("nothing to see"), amber `STATUS_WARNING`
        // once revealed ("caution: this is currently exposed") — per the
        // bead spec.
        let icon_color = if self.is_revealed() { palette::STATUS_WARNING } else { palette::TEXT_SECONDARY };
        let mut icon_buf = [0_u8; 4];
        let icon_str: &str = icon_char.encode_utf8(&mut icon_buf);
        let _ = icon_font.render_aligned(
            icon_str,
            Point::new(area.top_left.x + area.size.width as i32, area.top_left.y),
            VerticalPosition::Top,
            HorizontalAlignment::Right,
            FontColor::Transparent(icon_color),
            target,
        );

        let text_width = area.size.width.saturating_sub(ICON_RESERVED_WIDTH);
        let text_area = embedded_graphics::primitives::Rectangle::new(area.top_left, Size::new(text_width, area.size.height));
        let mut text_target = target.clipped(&text_area);

        // Both masked and revealed text render in `TEXT_PRIMARY` — per the
        // bead spec, only the icon's color signals reveal state; the value
        // text itself is styled the same as any other field's value.
        let value_font = font::secret();
        if self.is_revealed() {
            let _ = value_font.render_aligned(
                password,
                text_area.top_left,
                VerticalPosition::Top,
                HorizontalAlignment::Left,
                FontColor::Transparent(palette::TEXT_PRIMARY),
                &mut text_target,
            );
        } else {
            let mask: String = std::iter::repeat(MASK_GLYPH).take(MASK_GLYPH_COUNT).collect();
            let _ = value_font.render_aligned(
                mask.as_str(),
                text_area.top_left,
                VerticalPosition::Top,
                HorizontalAlignment::Left,
                FontColor::Transparent(palette::TEXT_PRIMARY),
                &mut text_target,
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::framebuffer::FrameBuffer565;
    use embedded_graphics::prelude::Point as EgPoint;
    use embedded_graphics::primitives::Rectangle;

    const AREA: Rectangle = Rectangle::new(EgPoint::new(0, 0), Size::new(200, 20));

    #[test]
    fn is_masked_by_default() {
        let field = SecretField::new();
        assert!(!field.is_revealed());
        assert_eq!(field.hint(), "Press to reveal");
    }

    #[test]
    fn activate_reveals_and_activate_again_re_masks() {
        let field = SecretField::new();
        field.on_focus(FocusEvent::Activated);
        assert!(field.is_revealed());
        assert_eq!(field.hint(), "Press to hide");

        field.on_focus(FocusEvent::Activated);
        assert!(!field.is_revealed());
        assert_eq!(field.hint(), "Press to reveal");
    }

    #[test]
    fn losing_focus_forces_a_re_mask() {
        let field = SecretField::new();
        field.on_focus(FocusEvent::Activated);
        assert!(field.is_revealed());

        field.on_focus(FocusEvent::Lost);
        assert!(!field.is_revealed());
    }

    #[test]
    fn gaining_focus_does_not_auto_reveal() {
        let field = SecretField::new();
        field.on_focus(FocusEvent::Gained);
        assert!(!field.is_revealed());
    }

    #[test]
    fn masked_rendering_is_identical_regardless_of_the_real_passwords_length() {
        // The "no length leak" proof: two wildly different password
        // lengths must render pixel-for-pixel identically while masked —
        // both the mask-glyph count and the closed-lock icon are fixed,
        // independent of `password`'s content entirely.
        let short = SecretField::new();
        let mut fb_short = FrameBuffer565::new(320, 170);
        short.render(AREA, "a", &mut fb_short).unwrap();

        let long = SecretField::new();
        let mut fb_long = FrameBuffer565::new(320, 170);
        long.render(AREA, "a-very-long-password-indeed-1234567890", &mut fb_long).unwrap();

        let short_pixels: Vec<_> = fb_short.pixels().map(|p| p.1).collect();
        let long_pixels: Vec<_> = fb_long.pixels().map(|p| p.1).collect();
        assert_eq!(short_pixels, long_pixels, "masked rendering must not depend on the real password's length");
    }

    #[test]
    fn revealed_rendering_differs_from_masked_rendering() {
        let field = SecretField::new();
        let mut fb_masked = FrameBuffer565::new(320, 170);
        field.render(AREA, "hunter2", &mut fb_masked).unwrap();

        field.on_focus(FocusEvent::Activated);
        let mut fb_revealed = FrameBuffer565::new(320, 170);
        field.render(AREA, "hunter2", &mut fb_revealed).unwrap();

        let masked_pixels: Vec<_> = fb_masked.pixels().map(|p| p.1).collect();
        let revealed_pixels: Vec<_> = fb_revealed.pixels().map(|p| p.1).collect();
        assert_ne!(masked_pixels, revealed_pixels);
    }

    #[test]
    fn revealed_icon_uses_the_warning_color_and_masked_icon_uses_the_muted_color() {
        let field = SecretField::new();
        let mut fb_masked = FrameBuffer565::new(320, 170);
        field.render(AREA, "hunter2", &mut fb_masked).unwrap();
        let any_muted_icon_pixel = fb_masked.pixels().any(|p| p.1 == palette::TEXT_SECONDARY);
        assert!(any_muted_icon_pixel, "masked lock icon should draw in the muted color");
        let any_warning_pixel_masked = fb_masked.pixels().any(|p| p.1 == palette::STATUS_WARNING);
        assert!(!any_warning_pixel_masked, "masked state must not show the warning color anywhere");

        field.on_focus(FocusEvent::Activated);
        let mut fb_revealed = FrameBuffer565::new(320, 170);
        field.render(AREA, "hunter2", &mut fb_revealed).unwrap();
        let any_warning_pixel_revealed = fb_revealed.pixels().any(|p| p.1 == palette::STATUS_WARNING);
        assert!(any_warning_pixel_revealed, "revealed lock icon should draw in the warning (amber) color");
    }
}
