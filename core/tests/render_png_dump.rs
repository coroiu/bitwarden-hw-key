//! Integration test proving the render core end-to-end, independent of
//! `examples/render_scene.rs` (which is for humans to eyeball; this is for
//! CI). Builds the same "titled screen + short vertical list" scene,
//! renders it into a `FrameBuffer565`, dumps it to a PNG in a scratch
//! directory (proving the framebuffer's pixel data round-trips through a
//! real image encoder, not just that `render()` returns `Ok`), and asserts
//! on specific pixel colors to prove the scene actually drew what it
//! should have — not merely that nothing panicked.

use bhk_core::input::NavIntent;
use bhk_core::render::{FrameBuffer565, ListItem, Navigator, Screen, VerticalList};
use embedded_graphics::pixelcolor::{Rgb565, WebColors};
use embedded_graphics::prelude::{Point, RgbColor};

fn build_scene() -> Navigator {
    let items = vec![
        ListItem::new("Bitwarden.com").with_sublabel("alice@example.com"),
        ListItem::new("GitHub").with_sublabel("alice-dev"),
        ListItem::new("AWS Console").with_sublabel("alice@corp.io"),
    ];
    let list = VerticalList::new(items);
    let root = Screen::new("Vault", vec![Box::new(list)]).with_hint("Next/Prev  Select  Back");
    Navigator::new(root)
}

#[test]
fn scene_renders_expected_chrome_colors() {
    let navigator = build_scene();
    let mut framebuffer = FrameBuffer565::new(320, 170);
    navigator.render(&mut framebuffer).expect("core DrawTarget is Infallible");

    // Title bar background, per Screen::render.
    assert_eq!(framebuffer.pixel(Point::new(0, 0)), Rgb565::CSS_MIDNIGHT_BLUE);
    assert_eq!(framebuffer.pixel(Point::new(319, 0)), Rgb565::CSS_MIDNIGHT_BLUE);

    // First row is selected by default (initialize_focus + selected == 0):
    // its background should be the selected-row fill, not plain black.
    let first_row_background = framebuffer.pixel(Point::new(2, 20));
    assert_eq!(first_row_background, Rgb565::CSS_DARK_SLATE_BLUE);

    // Somewhere below the last row (blank content area) should still be
    // black background — proves the list isn't painting outside its own
    // rows.
    assert_eq!(framebuffer.pixel(Point::new(2, 16 + 3 * 20 + 5)), Rgb565::BLACK);
}

#[test]
fn dispatching_next_moves_the_selection_highlight_down_one_row() {
    let mut navigator = build_scene();
    navigator.dispatch(NavIntent::Next);

    let mut framebuffer = FrameBuffer565::new(320, 170);
    navigator.render(&mut framebuffer).expect("core DrawTarget is Infallible");

    // Row 0's background is no longer highlighted...
    assert_ne!(framebuffer.pixel(Point::new(2, 20)), Rgb565::CSS_DARK_SLATE_BLUE);
    // ...row 1's is.
    assert_eq!(framebuffer.pixel(Point::new(2, 40)), Rgb565::CSS_DARK_SLATE_BLUE);
}

#[test]
fn framebuffer_round_trips_through_a_real_png_encoder() {
    let navigator = build_scene();
    let mut framebuffer = FrameBuffer565::new(320, 170);
    navigator.render(&mut framebuffer).expect("core DrawTarget is Infallible");

    let mut image = image::RgbImage::new(framebuffer.width(), framebuffer.height());
    for pixel in framebuffer.pixels() {
        let color = pixel.1;
        // See the identical comment in `examples/render_scene.rs`:
        // coordinates from this iterator are always non-negative.
        #[allow(clippy::cast_sign_loss)]
        image.put_pixel(
            pixel.0.x as u32,
            pixel.0.y as u32,
            image::Rgb([color.r() << 3, color.g() << 2, color.b() << 3]),
        );
    }

    let path = std::env::temp_dir().join("bhk-core-render-png-dump-test.png");
    image.save(&path).expect("failed to encode/write PNG");

    // Round-trip: decode what we just wrote and check the title bar pixel
    // survived encoding intact (Rgb565 CSS_MIDNIGHT_BLUE (25,25,112)
    // downsampled to 565 and back up to 8-bit-per-channel space).
    let decoded = image::open(&path).expect("failed to decode PNG we just wrote").to_rgb8();
    let title_pixel = decoded.get_pixel(0, 0);
    let expected = Rgb565::CSS_MIDNIGHT_BLUE;
    assert_eq!(
        *title_pixel,
        image::Rgb([expected.r() << 3, expected.g() << 2, expected.b() << 3])
    );

    std::fs::remove_file(&path).ok();
}
