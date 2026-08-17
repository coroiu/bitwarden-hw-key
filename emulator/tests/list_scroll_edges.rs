//! Headless, pixel-level proof of bead `ai-bitwarden-hw-key-47g`'s fix:
//! the credential list only scrolls when the selection would move
//! *outside* the visible window, not on every selection move (the old
//! `scroll_offset_for_selection` bug pinned the selected row's bottom to
//! the viewport bottom on every render, so moving the selection back up
//! into an already-visible row still re-scrolled the list).
//!
//! Drives a real `bhk_core::App` through `bhk_core::run::run` against a
//! real `emulator::platform::HeadlessSurface`, exactly as
//! `emulator/tests/headless_http_drive.rs` does (minus the HTTP layer,
//! which isn't what this test is about) -- `NavIntent`s are injected via a
//! small local queued `InputSource`, and the resulting frames are
//! inspected pixel-by-pixel via `HeadlessSurface::encode_png`.

use std::convert::Infallible;

use bhk_core::input::NavIntent;
use bhk_core::platform::{InputSource, Platform};
use bhk_core::render::chrome::TITLE_BAR_HEIGHT;
use bhk_core::render::theme::palette;
use bhk_core::render::ROW_HEIGHT;
use bhk_core::{run, App, SyncSource, VaultItem};
use embedded_graphics::prelude::RgbColor;
use emulator::platform::{FileStorage, HeadlessSurface, HostPlatform};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 170;

/// Mirrors `headless_http_drive.rs`'s `FixedSyncSource`: always returns
/// the same snapshot, so every re-render this test observes is driven
/// purely by the injected `NavIntent`s, not incidental sync churn.
struct FixedSyncSource(Vec<VaultItem>);
impl SyncSource for FixedSyncSource {
    type Error = Infallible;
    fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
        Ok(self.0.clone())
    }
}

/// A queued `InputSource`: each `poll()` call drains and returns the
/// next queued frame's intents (or an empty `Vec` once exhausted).
/// Mirrors `bhk_core::run`'s own test-only `QueuedInput` (core/src/run.rs)
/// -- duplicated here rather than shared, since it's a few lines and this
/// crate has no existing test-only `InputSource` of this exact shape
/// (`HttpInput` needs a real queue+mutex, `NoopInput` never produces
/// anything).
struct QueuedInput(Vec<Vec<NavIntent>>);
impl InputSource for QueuedInput {
    fn poll(&mut self) -> Vec<NavIntent> {
        if self.0.is_empty() {
            Vec::new()
        } else {
            self.0.remove(0)
        }
    }
}

fn vault_item(name: &str) -> VaultItem {
    VaultItem {
        id: uuid::Uuid::new_v4(),
        name: name.to_string(),
        username: String::new(),
        password: String::new(),
        uri: None,
        notes: None,
    }
}

/// Runs `frames` iterations of `run`, feeding it `queued` as the input
/// source, and returns the final `HeadlessSurface`'s PNG decoded to RGB8.
/// A fresh `HeadlessSurface`/`HostPlatform` per call (rather than one
/// long-lived platform threaded through multiple calls) keeps each
/// captured frame's provenance obvious: "the frame after these exact
/// intents, from a clean start" -- matching how `App`'s own persistent
/// `Cell`-based selection/scroll state (not `HostPlatform`) is what's
/// actually under test here.
fn render_after(app: &mut App, sync: &mut FixedSyncSource, queued: Vec<Vec<NavIntent>>) -> image::RgbImage {
    let frames = queued.len();
    let surface = HeadlessSurface::new();
    let storage_path = std::env::temp_dir().join(format!("bhk-list-scroll-edges-test-{}.json", uuid::Uuid::new_v4()));
    let storage = FileStorage::new(storage_path).expect("open a temp kv store");
    let mut platform = HostPlatform::new(surface, QueuedInput(queued), storage);

    let mut iterations = 0;
    run(&mut platform, app, sync, std::time::Duration::from_millis(0), || {
        iterations += 1;
        iterations <= frames
    });

    let png_bytes = platform.display().encode_png().expect("at least one frame was rendered/flushed");
    image::load_from_memory(&png_bytes).expect("PNG we just wrote must decode").to_rgb8()
}

#[test]
fn moving_selection_up_within_the_visible_window_does_not_scroll_the_list_headless() {
    // 10 items on a 320x170 screen: content height is 170 -
    // TITLE_BAR_HEIGHT(16) - HINT_BAR_HEIGHT(18) = 136px, and
    // ROW_HEIGHT is 40px, so exactly 3 rows are fully visible (136/40 =
    // 3, with a 16px partial-row peek) -- the "~10 items -> ~3 visible
    // rows" setup bead 47g's fix needs to prove against.
    let items: Vec<VaultItem> = (0..10).map(|i| vault_item(&format!("item-{i}"))).collect();
    let mut app = App::new(WIDTH, HEIGHT, items.clone());
    let mut sync = FixedSyncSource(items);

    // Frame 1 (no input, initial render) is implicit -- `App::new`
    // starts dirty. Frames 2-5: four `Next` intents, one per frame,
    // mirroring exactly how the real run loop delivers input (one
    // `NavIntent` batch per render), which is what makes the
    // `reconcile_top_index` trace below (0 -> 0 -> 0 -> 1 -> 2)
    // meaningful: it depends on `render` being called after each
    // individual move, not just once at the very end.
    //
    // Selected ends at index 4; per `core/src/render/list.rs`'s
    // `reconcile_top_index`, the window's top index settles at 2 (rows
    // 2, 3, 4 visible -- 4 is the new last visible row).
    let scrolled_down = render_after(&mut app, &mut sync, vec![vec![NavIntent::Next]; 4]);

    // Row positions with top=2: screen row 0 = item 2, screen row 1 =
    // item 3, screen row 2 = item 4 (selected).
    let row_item2_y = TITLE_BAR_HEIGHT + 2;
    let row_item3_y = TITLE_BAR_HEIGHT + ROW_HEIGHT + 2;
    let row_item4_y = TITLE_BAR_HEIGHT + 2 * ROW_HEIGHT + 2;
    let sample_x = 250; // past the accent bar and these short labels' text, per headless_http_drive.rs's identical convention
    let highlight = palette::SURFACE_ELEVATED;
    let highlight_rgb8 = image::Rgb([highlight.r() << 3, highlight.g() << 2, highlight.b() << 3]);

    assert_eq!(*scrolled_down.get_pixel(sample_x, row_item4_y), highlight_rgb8, "item 4 (the current selection) should be highlighted");
    assert_ne!(*scrolled_down.get_pixel(sample_x, row_item3_y), highlight_rgb8, "item 3 should not be highlighted yet");

    // One more frame: a single `Prev`. Selected moves to item 3, which
    // is still inside the current [2, 5) window -- per bead 47g's fix,
    // the list must NOT scroll again.
    let after_prev = render_after(&mut app, &mut sync, vec![vec![NavIntent::Prev]]);

    // The bug, as a pixel test: a whole horizontal slice of item 2's row
    // (non-selected in both frames, and it's the row bead 47g's repro
    // says shouldn't move) must be byte-for-byte identical before and
    // after the Prev -- not just one lucky "text" pixel, but the entire
    // row, which is what "the list didn't scroll, only the highlight
    // moved" actually means pixel-for-pixel.
    let row_item2_before: Vec<image::Rgb<u8>> = (0..WIDTH).map(|x| *scrolled_down.get_pixel(x, row_item2_y)).collect();
    let row_item2_after: Vec<image::Rgb<u8>> = (0..WIDTH).map(|x| *after_prev.get_pixel(x, row_item2_y)).collect();
    assert_eq!(
        row_item2_before, row_item2_after,
        "item 2's row (non-selected, still visible) must not shift when the selection moves within the window"
    );

    // Meanwhile the highlight itself DID move: off item 4, onto item 3.
    assert_ne!(*after_prev.get_pixel(sample_x, row_item4_y), highlight_rgb8, "item 4 should no longer be highlighted");
    assert_eq!(*after_prev.get_pixel(sample_x, row_item3_y), highlight_rgb8, "item 3 should now be highlighted -- the selection moved, the list did not scroll");
}
