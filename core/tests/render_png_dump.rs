//! Integration test proving the render core end-to-end, independent of
//! `examples/render_scene.rs` (which is for humans to eyeball; this is for
//! CI). Builds the same "titled screen + short vertical list" scene,
//! renders it into a `FrameBuffer565`, dumps it to a PNG in a scratch
//! directory (proving the framebuffer's pixel data round-trips through a
//! real image encoder, not just that `render()` returns `Ok`), and asserts
//! on specific pixel colors to prove the scene actually drew what it
//! should have — not merely that nothing panicked.

// See the identical allow (and rationale) on `bhk_core::render`: this test
// mirrors that module's `embedded-graphics` `Point`(i32)/`Size`(u32)
// coordinate math directly against real pixel data, so the same
// justification applies.
#![allow(clippy::cast_possible_wrap)]

use bhk_core::input::NavIntent;
use bhk_core::render::chrome::TITLE_BAR_HEIGHT;
use bhk_core::render::{FrameBuffer565, ListItem, Navigator, Screen, VerticalList, ROW_HEIGHT};
use embedded_graphics::pixelcolor::{Rgb565, WebColors};
use embedded_graphics::prelude::{Point, RgbColor};

const ITEM_COUNT: i32 = 3;

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

/// Absolute y of the top edge of row `index` (0-based) in `build_scene`'s
/// list, given the content region starts right below the title bar.
fn row_top(index: i32) -> i32 {
    TITLE_BAR_HEIGHT as i32 + index * ROW_HEIGHT as i32
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
    let first_row_background = framebuffer.pixel(Point::new(2, row_top(0) + 2));
    assert_eq!(first_row_background, Rgb565::CSS_DARK_SLATE_BLUE);

    // Below the last row (blank content area) should still be black
    // background — proves the list isn't painting outside its own rows.
    let below_last_row = row_top(ITEM_COUNT) + 5;
    assert_eq!(framebuffer.pixel(Point::new(2, below_last_row)), Rgb565::BLACK);
}

#[test]
fn dispatching_next_moves_the_selection_highlight_down_one_row() {
    let mut navigator = build_scene();
    navigator.dispatch(NavIntent::Next);

    let mut framebuffer = FrameBuffer565::new(320, 170);
    navigator.render(&mut framebuffer).expect("core DrawTarget is Infallible");

    // Row 0's background is no longer highlighted...
    assert_ne!(framebuffer.pixel(Point::new(2, row_top(0) + 2)), Rgb565::CSS_DARK_SLATE_BLUE);
    // ...row 1's is.
    assert_eq!(framebuffer.pixel(Point::new(2, row_top(1) + 2)), Rgb565::CSS_DARK_SLATE_BLUE);
}

/// Regression test for the row-overflow bug: `FONT_6X10` positions text at
/// its *baseline*, not its top-left, so the sublabel's descent used to
/// land 4px past `ROW_HEIGHT`'s old value — inside the padding gap this
/// test checks, and in the worst case inside the next row's opaque
/// selection-highlight fill (which sits on top in draw order, so it looks
/// like the highlight "ate" the previous row's sublabel tail).
///
/// This scans every row's bottom padding strip (the last pixel row of its
/// `ROW_HEIGHT` band, sized to `list.rs`'s `ROW_PADDING`) across the full
/// row width and asserts no label (white) or sublabel (gray) pixel appears
/// there. If a future change shrinks `ROW_HEIGHT` or grows the font
/// without updating the other, this fails loudly instead of silently
/// reintroducing the bleed.
const BOTTOM_PADDING_PX: i32 = 1;

#[test]
fn text_never_bleeds_past_a_rows_bottom_padding() {
    let navigator = build_scene();
    let mut framebuffer = FrameBuffer565::new(320, 170);
    navigator.render(&mut framebuffer).expect("core DrawTarget is Infallible");

    for index in 0..ITEM_COUNT {
        let top = row_top(index);
        let bottom = top + ROW_HEIGHT as i32;
        for y in (bottom - BOTTOM_PADDING_PX)..bottom {
            for x in 0..framebuffer.width() as i32 {
                let color = framebuffer.pixel(Point::new(x, y));
                assert_ne!(
                    color,
                    Rgb565::WHITE,
                    "row {index}'s label text bled into its bottom padding at ({x},{y})"
                );
                assert_ne!(
                    color,
                    Rgb565::CSS_GRAY,
                    "row {index}'s sublabel text bled into its bottom padding at ({x},{y})"
                );
            }
        }
    }
}

/// Companion to the padding-bleed check above: an unselected row's pixels
/// must be identical no matter which *other* row currently holds the
/// selection highlight. This is what "the selection box cuts through
/// text" (from the bug report) would look like as a test failure — row 0,
/// never selected in either frame here, would differ between frames only
/// if some other row's highlight (or its text) reached into row 0's band.
///
/// (Comparing "row 0 selected" vs. "row 1 selected" directly doesn't work:
/// row 0 *is* selected by default, so its own fill legitimately differs
/// between those two frames — that's correct behavior, not corruption.
/// Selecting row 1 and row 2 instead keeps row 0 unselected throughout, so
/// any difference in row 0's band is unambiguously a bug.)
#[test]
fn an_unselected_rows_pixels_do_not_depend_on_which_other_row_is_selected() {
    let mut selecting_row1 = build_scene();
    selecting_row1.dispatch(NavIntent::Next);
    let mut frame_row1_selected = FrameBuffer565::new(320, 170);
    selecting_row1.render(&mut frame_row1_selected).unwrap();

    let mut selecting_row2 = build_scene();
    selecting_row2.dispatch(NavIntent::Next);
    selecting_row2.dispatch(NavIntent::Next);
    let mut frame_row2_selected = FrameBuffer565::new(320, 170);
    selecting_row2.render(&mut frame_row2_selected).unwrap();

    let row0_top = row_top(0);
    let row0_bottom = row0_top + ROW_HEIGHT as i32;
    for y in row0_top..row0_bottom {
        for x in 0..frame_row1_selected.width() as i32 {
            let point = Point::new(x, y);
            assert_eq!(
                frame_row1_selected.pixel(point),
                frame_row2_selected.pixel(point),
                "row 0's pixel at ({x},{y}), never selected in either frame, \
                 differed depending on whether row 1 or row 2 was selected"
            );
        }
    }
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
