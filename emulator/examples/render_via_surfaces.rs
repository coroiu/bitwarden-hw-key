//! Verifiable deliverable for W4: renders the same demo scene as
//! `bhk-core`'s `render_scene` example through both host `DisplaySurface`s
//! and proves they show the same thing.
//!
//! By default (no arguments) this only exercises `HeadlessSurface`: it
//! renders, flushes, and writes `headless_scene.png` to the current
//! directory — safe to run in CI/sandboxes with no display server. Pass
//! `--window` to additionally open a real `minifb` window with
//! `MinifbSurface` (at a visible upscale) so a human can eyeball it against
//! the PNG; the window stays open until closed.
//!
//! This is deliberately not a full app loop (that's W7): it renders one
//! static scene once per surface, it doesn't wire up `InputSource` polling
//! or re-render on intent.
//!
//! Run with:
//!   `cargo run -p emulator --example render_via_surfaces --target <host-triple>`
//!   `cargo run -p emulator --example render_via_surfaces --target <host-triple> -- --window`

use std::cell::RefCell;
use std::rc::Rc;

use bhk_core::input::NavIntent;
use bhk_core::platform::DisplaySurface;
use bhk_core::render::{FrameBuffer565, ListItem, Navigator, Screen, VerticalList};
use emulator::platform::{HeadlessSurface, MinifbSurface};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 170;
const WINDOW_SCALE: u32 = 3;

fn build_scene() -> Navigator {
    let items = vec![
        ListItem::new("Bitwarden.com").with_sublabel("alice@example.com"),
        ListItem::new("GitHub").with_sublabel("alice-dev"),
        ListItem::new("AWS Console").with_sublabel("alice@corp.io"),
        ListItem::new("Postgres (prod)").with_sublabel("svc-account"),
    ];
    let list = VerticalList::new(items);
    let root = Screen::new("Vault", vec![Box::new(list)]).with_hint("Next/Prev  Select  Back");
    let mut navigator = Navigator::new(root);
    navigator.dispatch(NavIntent::Next);
    navigator
}

fn main() {
    let open_window = std::env::args().any(|arg| arg == "--window");

    let navigator = build_scene();
    let mut framebuffer = FrameBuffer565::new(WIDTH, HEIGHT);
    navigator.render(&mut framebuffer).expect("core DrawTarget is Infallible");

    let mut headless = HeadlessSurface::new();
    headless.flush(&framebuffer).expect("HeadlessSurface::flush is Infallible");
    let path = "headless_scene.png";
    headless.save_png(path).expect("failed to write PNG");
    println!("wrote {path} ({WIDTH}x{HEIGHT}) via HeadlessSurface");

    if !open_window {
        println!("(pass --window to also open a live MinifbSurface window for visual comparison)");
        return;
    }

    let window = minifb::Window::new(
        "bhk W4 surface parity check (close window to exit)",
        (WIDTH * WINDOW_SCALE) as usize,
        (HEIGHT * WINDOW_SCALE) as usize,
        minifb::WindowOptions::default(),
    )
    .expect("failed to open minifb window");
    let window = Rc::new(RefCell::new(window));

    let mut minifb_surface = MinifbSurface::new(Rc::clone(&window), WIDTH, HEIGHT, WINDOW_SCALE);
    minifb_surface.flush(&framebuffer).expect("failed to flush to minifb window");
    println!("MinifbSurface window open at {WINDOW_SCALE}x scale ({WIDTH}x{HEIGHT} source) — close it to exit.");
    println!("Compare against {path}: they must show the same layout, colors, and text.");

    while window.borrow().is_open() {
        // Re-present the same static framebuffer every frame; minifb needs
        // `update`/`update_with_buffer` calls to keep pumping the OS event
        // loop (window close, etc.) even though nothing changes.
        minifb_surface.flush(&framebuffer).expect("failed to flush to minifb window");
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}
