//! Proves the core deliverable of W4: `HeadlessSurface` and `MinifbSurface`
//! flushing the *same* `FrameBuffer565` present the *same* pixels. This is
//! the whole point of the presentation-surface ADR (`.planning/decisions/
//! 2026-08-11-presentation-surface-run-mode-seam.md`) — a headless
//! screenshot must be trustworthy evidence of what the windowed emulator
//! (and eventually the real T-Embed) would show.
//!
//! `MinifbSurface::flush` itself needs a real `minifb::Window`, which this
//! test deliberately does not create (opening an OS window from a test
//! binary is unreliable in headless CI/sandbox environments with no
//! display server). Instead this test calls `rasterize_scaled` directly —
//! the exact, sole pixel-conversion function `MinifbSurface::flush` calls
//! before handing the buffer to `minifb` — so what's under test is real
//! production code, just exercised without the window I/O side effect.
//!
//! Builds the identical scene `core/tests/render_png_dump.rs` uses (titled
//! screen + 3-item vertical list, one `NavIntent::Next` dispatched so the
//! selection highlight isn't just sitting on row 0 by coincidence), for the
//! same reason that test gives: proves real widget rendering, not a blank
//! or synthetic frame.

use bhk_core::input::NavIntent;
use bhk_core::platform::DisplaySurface;
use bhk_core::render::{FrameBuffer565, ListItem, Navigator, Screen, VerticalList};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::RgbColor;
use emulator::platform::minifb_surface::rasterize_scaled;
use emulator::platform::HeadlessSurface;

/// Arbitrary scale > 1 for the full-sweep parity test, so it also proves
/// upscaling doesn't desync the two surfaces' colors (only replicates them
/// into blocks).
const PARITY_TEST_SCALE: u32 = 3;

fn build_scene() -> Navigator {
    let items = vec![
        ListItem::new("Bitwarden.com").with_sublabel("alice@example.com"),
        ListItem::new("GitHub").with_sublabel("alice-dev"),
        ListItem::new("AWS Console").with_sublabel("alice@corp.io"),
    ];
    let list = VerticalList::new(items);
    let root = Screen::new("Vault", vec![Box::new(list)]).with_hint("Next/Prev  Select  Back");
    let mut navigator = Navigator::new(root);
    navigator.dispatch(NavIntent::Next);
    navigator
}

/// Extracts the (r, g, b) 8-bit-per-channel triple minifb packs into an
/// ARGB8888 `u32` (0x00RRGGBB — `minifb` ignores the top byte).
fn unpack_argb(pixel: u32) -> (u8, u8, u8) {
    let r = ((pixel >> 16) & 0xFF) as u8;
    let g = ((pixel >> 8) & 0xFF) as u8;
    let b = (pixel & 0xFF) as u8;
    (r, g, b)
}

#[test]
fn headless_png_and_minifb_buffer_agree_on_every_pixel() {
    let navigator = build_scene();
    let mut framebuffer = FrameBuffer565::new(320, 170);
    navigator.render(&mut framebuffer).expect("core DrawTarget is Infallible");

    // Headless side: flush, encode, decode back to RGB8 pixels.
    let mut headless = HeadlessSurface::new();
    headless.flush(&framebuffer).expect("HeadlessSurface::flush is Infallible");
    let png_bytes = headless.encode_png().expect("frame was flushed");
    let decoded = image::load_from_memory(&png_bytes).expect("PNG we just wrote must decode").to_rgb8();
    assert_eq!(decoded.width(), 320);
    assert_eq!(decoded.height(), 170);

    // Windowed side: the same rasterize_scaled MinifbSurface::flush calls.
    let scaled_width = 320 * PARITY_TEST_SCALE;
    let mut minifb_buffer = vec![0u32; (scaled_width * 170 * PARITY_TEST_SCALE) as usize];
    rasterize_scaled(&framebuffer, PARITY_TEST_SCALE, &mut minifb_buffer);

    let mut mismatches = Vec::new();
    for y in 0..170u32 {
        for x in 0..320u32 {
            let png_pixel = decoded.get_pixel(x, y);
            let expected = (png_pixel[0], png_pixel[1], png_pixel[2]);

            // Any pixel within this source pixel's scale x scale block
            // should carry the same color; sampling the top-left corner
            // of the block is sufficient (a separate test already proves
            // the whole block is uniform).
            let block_index = (y * PARITY_TEST_SCALE * scaled_width + x * PARITY_TEST_SCALE) as usize;
            let actual = unpack_argb(minifb_buffer[block_index]);

            if actual != expected {
                mismatches.push((x, y, expected, actual));
            }
        }
    }

    assert!(
        mismatches.is_empty(),
        "{} of {} pixels differed between HeadlessSurface's PNG and MinifbSurface's buffer; \
         first mismatch: {:?}",
        mismatches.len(),
        320 * 170,
        mismatches.first()
    );
}

#[test]
fn known_chrome_colors_match_between_surfaces_at_scale_one() {
    // Companion sanity check at scale 1 (no block expansion at all), and
    // pinned to concrete expected colors (not just "the two surfaces
    // agree with each other", which could pass if both were wrong in the
    // same way). Mirrors the assertions in `core/tests/render_png_dump.rs`.
    use embedded_graphics::pixelcolor::WebColors;

    let navigator = build_scene();
    let mut framebuffer = FrameBuffer565::new(320, 170);
    navigator.render(&mut framebuffer).unwrap();

    let mut headless = HeadlessSurface::new();
    headless.flush(&framebuffer).unwrap();
    let decoded = image::load_from_memory(&headless.encode_png().unwrap()).unwrap().to_rgb8();

    let mut minifb_buffer = vec![0u32; (320 * 170) as usize];
    rasterize_scaled(&framebuffer, 1, &mut minifb_buffer);

    let title_bar = Rgb565::CSS_MIDNIGHT_BLUE;
    let expected_title = (title_bar.r() << 3, title_bar.g() << 2, title_bar.b() << 3);

    let png_title = decoded.get_pixel(0, 0);
    assert_eq!((png_title[0], png_title[1], png_title[2]), expected_title);
    assert_eq!(unpack_argb(minifb_buffer[0]), expected_title);
}
