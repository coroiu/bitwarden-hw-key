//! The M1 visual design language: a semantic color palette, per-role
//! `u8g2-fonts` accessors, `open_iconic` icon codepoints, and two shared
//! drawing primitives (a chip and a full-width selection block).
//!
//! Approved by Uma (UX) + Andreas for the T-Embed's 320x170 ST7789 color
//! panel; see bead `ai-bitwarden-hw-key-0v8.8`. This module is the
//! *foundation* only — it swaps the render core's fonts/colors and
//! provides primitives for later beads (`ai-bitwarden-hw-key-0v8.5`'s list
//! chrome, `ai-bitwarden-hw-key-0v8.6`'s detail view) to compose; it does
//! not itself restyle spacing/layout.
//!
//! Reuses validated findings from the throwaway design-review spike
//! (`design-review-m1-mockups` branch, never merged):
//! `core/examples/design_review_m1.rs` for the palette's exact `Rgb565`
//! values and the font choice per role, and `core/examples/icon_probe.rs`
//! for the `shield`/`lock-locked`/`lock-unlocked`/`eye` `open_iconic_all`
//! codepoints (empirically probed there, since upstream u8g2 doesn't
//! document the mapping). `caret-right`'s codepoint was not covered by
//! that spike and was derived + probed fresh for this bead (see
//! `icon::CARET_RIGHT`'s doc comment).

use std::convert::Infallible;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{Point, Primitive, Size};
use embedded_graphics::primitives::{
    CornerRadiiBuilder, PrimitiveStyle, Rectangle, RoundedRectangle, StyledDrawable,
};
use embedded_graphics::Drawable;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::FontRenderer;

/// The semantic color palette. Every color used by the render core should
/// come from here, by role, rather than an ad-hoc `embedded_graphics`
/// `WebColors` constant — that's what "centralizing the theme" means: a
/// future palette tweak (or a light-mode variant, if one is ever wanted)
/// is a one-file change instead of a codebase-wide grep.
pub mod palette {
    use embedded_graphics::pixelcolor::Rgb565;

    /// Screen base fill (`#0B1120`): whatever isn't covered by a bar,
    /// card, or row — the "void" behind all content.
    pub const BACKGROUND: Rgb565 = Rgb565::new(1, 4, 4);
    /// Chrome bars (title/hint) and an unselected row's implicit
    /// backdrop (`#16213A`) — one step brighter than `BACKGROUND`.
    pub const SURFACE: Rgb565 = Rgb565::new(3, 8, 7);
    /// A selected row or focused field's backdrop (`#1E2A47`) — one step
    /// brighter than `SURFACE`, giving focus a visible lift without a
    /// literal raised/rounded card.
    pub const SURFACE_ELEVATED: Rgb565 = Rgb565::new(4, 10, 9);
    /// The brand fill color (`#175DDC`) — used for solid brand-colored
    /// shapes (e.g. the initial chip's background), not text or icons.
    pub const BRAND: Rgb565 = Rgb565::new(3, 23, 27);
    /// The brighter brand accent (`#3B82F6`) — selection accent bars,
    /// active/positive icon glyphs; reserved for things that should read
    /// as "brand, but louder" than a plain `BRAND` fill.
    pub const BRAND_BRIGHT: Rgb565 = Rgb565::new(7, 32, 30);
    /// Primary text and glyph color (`#F5F7FA`) — credential names,
    /// field values, chip initials.
    pub const TEXT_PRIMARY: Rgb565 = Rgb565::new(30, 61, 30);
    /// Secondary/muted text (`#9AA6BF`) — usernames, hints, field
    /// labels, the sync counter.
    pub const TEXT_SECONDARY: Rgb565 = Rgb565::new(19, 41, 23);
    /// Hairline separators between chrome and content, or between rows
    /// (`#22304F`).
    pub const DIVIDER: Rgb565 = Rgb565::new(4, 12, 10);
    /// A successful/positive status indicator (`#3DDC84`) — e.g. a
    /// sync-ok dot.
    pub const STATUS_SUCCESS: Rgb565 = Rgb565::new(7, 54, 16);
    /// An error/negative status indicator (`#FF5964`).
    pub const STATUS_ERROR: Rgb565 = Rgb565::new(31, 22, 12);
    /// A caution/attention status indicator (`#FFB020`) — e.g. "secret
    /// currently revealed."
    pub const STATUS_WARNING: Rgb565 = Rgb565::new(31, 44, 4);
}

/// Per-role `u8g2-fonts` accessors. Each returns a fresh, independently
/// configured [`FontRenderer`] (cheap — it's a thin wrapper over a static
/// font table, not an allocation) with
/// [`with_ignore_unknown_chars`](FontRenderer::with_ignore_unknown_chars)
/// set: credential names/usernames are arbitrary user data (could contain
/// glyphs `helv`/`profont` don't cover), and this render core must never
/// panic on unusual input — better to silently skip an unrenderable
/// character than crash the render loop over it.
pub mod font {
    use u8g2_fonts::fonts;
    use u8g2_fonts::FontRenderer;

    /// Screen/chrome titles (the title bar).
    #[must_use]
    pub const fn title() -> FontRenderer {
        FontRenderer::new::<fonts::u8g2_font_helvB10_tf>().with_ignore_unknown_chars(true)
    }

    /// A credential's name — the bold, primary line of a list row (and
    /// the detail screen's title-bar text, via [`title`] instead — `name`
    /// is specifically the *list row* weight/size).
    #[must_use]
    pub const fn name() -> FontRenderer {
        FontRenderer::new::<fonts::u8g2_font_helvB12_tf>().with_ignore_unknown_chars(true)
    }

    /// A credential's username — the secondary line of a list row.
    #[must_use]
    pub const fn username() -> FontRenderer {
        FontRenderer::new::<fonts::u8g2_font_helvR10_tf>().with_ignore_unknown_chars(true)
    }

    /// A plain detail-field value (e.g. website/URI) — not the secret
    /// field, which uses [`secret`] instead.
    #[must_use]
    pub const fn value() -> FontRenderer {
        FontRenderer::new::<fonts::u8g2_font_helvR12_tf>().with_ignore_unknown_chars(true)
    }

    /// The password/secret field's value. Monospaced (`profont`) so
    /// every masked `*` and every revealed character occupies the same
    /// width — a proportional font would make a masked secret's length
    /// visually leak information a monospaced mask doesn't.
    #[must_use]
    pub const fn secret() -> FontRenderer {
        FontRenderer::new::<fonts::u8g2_font_profont17_mf>().with_ignore_unknown_chars(true)
    }

    /// A detail field's label (e.g. "Username"). Callers render the
    /// label text in uppercase themselves — this accessor only provides
    /// the font/weight, per Uma's spec (`helvB08`, small caps-style
    /// label above each field).
    #[must_use]
    pub const fn label() -> FontRenderer {
        FontRenderer::new::<fonts::u8g2_font_helvB08_tf>().with_ignore_unknown_chars(true)
    }

    /// The hint bar's control-legend text. One size down from
    /// `username`/the design-review mockup's `helvR10` (Andreas's
    /// feedback: the hint bar reads as decoration, not primary content,
    /// so it can afford to be the smallest text on screen) — `helvR08`,
    /// the next `helv` size down that's still legible on this panel.
    #[must_use]
    pub const fn hint() -> FontRenderer {
        FontRenderer::new::<fonts::u8g2_font_helvR08_tf>().with_ignore_unknown_chars(true)
    }

    /// `open_iconic` glyphs at 1x scale (roughly 8x8px) — for compact
    /// inline icons (e.g. a row's focus caret), and deliberately also the
    /// title bar's shield mark (bead `ai-bitwarden-hw-key-0v8.5`): the
    /// design-review mockup used `icon_2x` there, which — inside the fixed
    /// `TITLE_BAR_HEIGHT`-px bar — left the mark touching the bar's top/
    /// bottom edges with no breathing room; Andreas asked for a smaller
    /// mark with more air around it, and `icon_1x` is what leaves that air
    /// without shrinking `TITLE_BAR_HEIGHT` itself.
    #[must_use]
    pub const fn icon_1x() -> FontRenderer {
        FontRenderer::new::<fonts::u8g2_font_open_iconic_all_1x_t>()
    }

    /// `open_iconic` glyphs at 2x scale (roughly 16x16px) — for
    /// field-level status icons (e.g. the lock in a password field).
    #[must_use]
    pub const fn icon_2x() -> FontRenderer {
        FontRenderer::new::<fonts::u8g2_font_open_iconic_all_2x_t>()
    }

    /// `open_iconic` glyphs at 4x scale (roughly 32x32px) — for a large,
    /// centered decorative mark (e.g. the empty-vault content state's
    /// shield), never for chrome (too big for the fixed-height title/hint
    /// bars).
    #[must_use]
    pub const fn icon_4x() -> FontRenderer {
        FontRenderer::new::<fonts::u8g2_font_open_iconic_all_4x_t>()
    }
}

/// `open_iconic_all_*` codepoints. Not documented upstream — u8g2's
/// `do_iconic.sh` build script assigns codepoints in the alphabetical
/// order of `github.com/iconic/open-iconic`'s `svg/` directory, starting
/// at `U+0040` (`-e 64`), which has to be derived (or probed) rather than
/// looked up.
pub mod icon {
    /// The Bitwarden/vault brand mark, for chrome (e.g. the title bar).
    /// Probed in the design-review spike's `core/examples/icon_probe.rs`.
    pub const SHIELD: char = '\u{FC}';
    /// A closed padlock — e.g. a masked secret field.
    /// Probed in the design-review spike's `core/examples/icon_probe.rs`.
    pub const LOCK_LOCKED: char = '\u{CA}';
    /// An open padlock — e.g. a revealed secret field.
    /// Probed in the design-review spike's `core/examples/icon_probe.rs`.
    pub const LOCK_UNLOCKED: char = '\u{CB}';
    /// An eye — an alternative "reveal" affordance to [`LOCK_UNLOCKED`],
    /// for a future field that prefers that convention.
    /// Probed in the design-review spike's `core/examples/icon_probe.rs`.
    pub const EYE: char = '\u{A5}';
    /// A right-pointing solid caret/triangle — a row/field's "this is
    /// selected, activate to go further" disclosure indicator.
    ///
    /// Not covered by the design-review spike (it drew this shape as a
    /// raw `Triangle` primitive instead). Derived for this bead from
    /// `iconic/open-iconic`'s `svg/` directory listing: `caret-right.svg`
    /// is the 48th file alphabetically (1-indexed) -> 0-based index 47
    /// -> codepoint `64 + 47 = 111 = 0x6F`. Cross-checked two ways before
    /// trusting it: (1) the same formula applied to `eye.svg` (102nd
    /// file -> codepoint `0xA5`) reproduces the spike's independently
    /// probed value for [`EYE`] exactly; (2) this bead's own
    /// `core/examples/caret_probe.rs` rendered `0x6D..=0x71` to a PNG and
    /// visually confirmed `0x6D`/`0x6E`/`0x6F`/`0x70` are down/left/
    /// right/up-pointing carets respectively, with `0x6C`
    /// (camera-slr) and `0x71` (cart) as sane neighbors either side.
    pub const CARET_RIGHT: char = '\u{6F}';
}

/// Corner radius, in pixels, [`draw_chip`] draws its background with.
pub const CHIP_CORNER_RADIUS: u32 = 4;

/// Draws a brand-colored rounded-square "chip" filling `rect`, with
/// `initial` centered in it on *both* axes.
///
/// Uses [`font::name`] for the glyph (matching the design-review mockup),
/// but centers it by measuring the glyph's actual rendered ink
/// bounding box via
/// [`get_rendered_dimensions_aligned`](FontRenderer::get_rendered_dimensions_aligned)
/// and aligning *that* box's center to `rect`'s center — not by asking
/// `render_aligned` for `VerticalPosition::Center`/`HorizontalAlignment::Center`
/// directly. That naive approach is what the design-review mockup used,
/// and Andreas flagged the resulting chip letters as visibly off-center:
/// `u8g2-fonts`' `Center` positioning centers on the font's *line metrics*
/// (ascent/descent from the baseline), not a specific glyph's ink — for a
/// single flat-topped capital like "G" sitting inside a font whose descent
/// budget accounts for glyphs like "g"/"y" that this one character doesn't
/// use, that mismatch reads as "pushed up" from the box's true center.
/// Measuring this specific glyph's own ink box and centering *that*
/// sidesteps the whole line-metrics-vs-ink distinction.
///
/// Falls back to `rect`'s geometric center (no visible ink to align) if
/// `initial` has no glyph in [`font::name`] — this can't happen for the
/// ASCII/Latin-1 initials `char::to_ascii_uppercase` produces, but
/// `font::name` is configured to ignore unknown glyphs rather than error,
/// so this stays a graceful no-glyph-drawn case instead of a panic for a
/// non-Latin initial.
///
/// # Errors
///
/// Returns `Infallible`'s uninhabited variant in practice — see
/// [`super::widget::Widget::render`]'s doc comment for why the `Result`
/// return exists at all.
pub fn draw_chip<D>(target: &mut D, rect: Rectangle, initial: char) -> Result<(), Infallible>
where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    let radii = CornerRadiiBuilder::new().all(Size::new_equal(CHIP_CORNER_RADIUS)).build();
    RoundedRectangle::new(rect, radii)
        .draw_styled(&PrimitiveStyle::with_fill(palette::BRAND), target)?;

    let glyph_font: FontRenderer = font::name();
    let mut buf = [0_u8; 4];
    let text: &str = initial.encode_utf8(&mut buf);

    let ink_bbox = glyph_font
        .get_rendered_dimensions_aligned(
            text,
            Point::zero(),
            VerticalPosition::Top,
            HorizontalAlignment::Left,
        )
        .unwrap_or(None);

    let render_pos = match ink_bbox {
        Some(ink) => {
            let ink_center = ink.center();
            let target_center = rect.center();
            Point::new(target_center.x - ink_center.x, target_center.y - ink_center.y)
        }
        None => return Ok(()),
    };

    let _ = glyph_font.render_aligned(
        text,
        render_pos,
        VerticalPosition::Top,
        HorizontalAlignment::Left,
        FontColor::Transparent(palette::TEXT_PRIMARY),
        target,
    );

    Ok(())
}

/// Width, in pixels, of the left accent bar [`draw_selection`] draws.
pub const SELECTION_ACCENT_WIDTH: u32 = 4;

/// Draws the shared "this is the selected/focused thing" visual across
/// `area`: a full-`area`, edge-to-edge fill in [`palette::SURFACE_ELEVATED`],
/// then a [`SELECTION_ACCENT_WIDTH`]-px accent bar in
/// [`palette::BRAND_BRIGHT`] along `area`'s left edge.
///
/// Replaces `render::widget::draw_focus_block` (this bead retires it):
/// same shape (full-area fill + left accent bar, no inset/rounding/fake
/// elevation), but the colors are no longer caller-supplied parameters —
/// they're always the theme's selection colors, so every selected row or
/// focused field in the app looks identical by construction instead of by
/// convention. Per Andreas's explicit direction, this draws *only* the
/// fill + accent bar; a caret (see [`icon::CARET_RIGHT`]) or any other
/// per-row/per-field decoration is the caller's job, drawn on top of (or
/// clipped within) the same `area` after this returns.
///
/// Generic over `D: DrawTarget<Color = Rgb565, Error = Infallible>`
/// (rather than the concrete `FrameBuffer565`) so a caller drawing into a
/// `DrawTargetExt::clipped()` sub-region can pass that clipped target
/// directly — same rationale as the function this replaces.
///
/// # Errors
///
/// Returns `Infallible`'s uninhabited variant in practice — see
/// [`super::widget::Widget::render`]'s doc comment for why the `Result`
/// return exists at all.
pub fn draw_selection<D>(area: Rectangle, target: &mut D) -> Result<(), Infallible>
where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    area.into_styled(PrimitiveStyle::with_fill(palette::SURFACE_ELEVATED)).draw(target)?;

    let accent_width = SELECTION_ACCENT_WIDTH.min(area.size.width);
    let accent = Rectangle::new(area.top_left, Size::new(accent_width, area.size.height));
    accent.into_styled(PrimitiveStyle::with_fill(palette::BRAND_BRIGHT)).draw(target)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::FrameBuffer565;

    #[test]
    fn draw_selection_fills_the_full_area_and_paints_a_left_accent_bar() {
        let mut fb = FrameBuffer565::new(40, 20);
        let area = Rectangle::new(Point::new(0, 0), Size::new(40, 20));
        draw_selection(area, &mut fb).unwrap();

        // Left edge: the accent bar.
        assert_eq!(fb.pixel(Point::new(0, 10)), palette::BRAND_BRIGHT);
        assert_eq!(
            fb.pixel(Point::new(SELECTION_ACCENT_WIDTH as i32 - 1, 10)),
            palette::BRAND_BRIGHT
        );
        // Just past the accent bar, and the far right edge: the fill —
        // full width, no inset.
        assert_eq!(fb.pixel(Point::new(SELECTION_ACCENT_WIDTH as i32, 10)), palette::SURFACE_ELEVATED);
        assert_eq!(fb.pixel(Point::new(39, 10)), palette::SURFACE_ELEVATED);
        // Full height, no vertical inset either.
        assert_eq!(fb.pixel(Point::new(20, 0)), palette::SURFACE_ELEVATED);
        assert_eq!(fb.pixel(Point::new(20, 19)), palette::SURFACE_ELEVATED);
    }

    #[test]
    fn draw_selection_clamps_the_accent_bar_to_a_narrower_area() {
        let mut fb = FrameBuffer565::new(2, 10);
        let area = Rectangle::new(Point::new(0, 0), Size::new(2, 10));
        // Must not panic even though `area` is narrower than
        // `SELECTION_ACCENT_WIDTH`.
        draw_selection(area, &mut fb).unwrap();
        assert_eq!(fb.pixel(Point::new(0, 5)), palette::BRAND_BRIGHT);
        assert_eq!(fb.pixel(Point::new(1, 5)), palette::BRAND_BRIGHT);
    }

    #[test]
    fn draw_chip_paints_brand_background_and_a_centered_glyph() {
        let mut fb = FrameBuffer565::new(30, 30);
        let rect = Rectangle::new(Point::new(4, 4), Size::new(22, 22));
        draw_chip(&mut fb, rect, 'G').unwrap();

        // A corner (outside the rounded radius, outside the glyph) is the
        // brand fill.
        assert_eq!(fb.pixel(Point::new(5, 5)), palette::BRAND);
        // Somewhere in the chip drew primary-text-colored ink (the glyph)
        // — proves a glyph was actually rasterized, not just the
        // background.
        let any_glyph_ink =
            (rect.top_left.x..rect.top_left.x + rect.size.width as i32).any(|x| {
                (rect.top_left.y..rect.top_left.y + rect.size.height as i32)
                    .any(|y| fb.pixel(Point::new(x, y)) == palette::TEXT_PRIMARY)
            });
        assert!(any_glyph_ink, "draw_chip should have rasterized the initial's glyph ink");
    }

    #[test]
    fn draw_chip_centers_the_glyphs_ink_bounding_box_on_the_rects_center() {
        // Render 'G' via draw_chip, then independently recompute where
        // font::name()'s *ink bounding box* for "G" landed, and assert its
        // center matches rect.center() exactly. This is the specific
        // Andreas-flagged bug (naive Center/Center alignment centers on
        // font line-metrics, not glyph ink) this primitive exists to fix.
        let mut fb = FrameBuffer565::new(30, 30);
        let rect = Rectangle::new(Point::new(4, 4), Size::new(22, 22));
        draw_chip(&mut fb, rect, 'G').unwrap();

        let mut min = Point::new(i32::MAX, i32::MAX);
        let mut max = Point::new(i32::MIN, i32::MIN);
        let mut found_any = false;
        for x in rect.top_left.x..rect.top_left.x + rect.size.width as i32 {
            for y in rect.top_left.y..rect.top_left.y + rect.size.height as i32 {
                if fb.pixel(Point::new(x, y)) == palette::TEXT_PRIMARY {
                    found_any = true;
                    min.x = min.x.min(x);
                    min.y = min.y.min(y);
                    max.x = max.x.max(x);
                    max.y = max.y.max(y);
                }
            }
        }
        assert!(found_any);
        let ink_center = Point::new((min.x + max.x) / 2, (min.y + max.y) / 2);
        let rect_center = rect.center();
        // Within 1px on each axis: the ink box's own width/height parity
        // (odd vs even pixel count) can make an exact-integer center land
        // a half-pixel either side of rect_center depending on rounding.
        assert!(
            (ink_center.x - rect_center.x).abs() <= 1,
            "chip glyph should be horizontally centered: ink_center={ink_center:?} rect_center={rect_center:?}"
        );
        assert!(
            (ink_center.y - rect_center.y).abs() <= 1,
            "chip glyph should be vertically centered: ink_center={ink_center:?} rect_center={rect_center:?}"
        );
    }
}
