//! The `desktop` binary: the emulator entry point for the two host run
//! modes (windowed, headless) per
//! `.planning/decisions/2026-08-11-three-mode-testability.md`. Both modes
//! share the exact same `bhk_core::App` + `bhk_core::run` loop; they differ
//! only in which concrete `DisplaySurface`/`InputSource` they hand to a
//! `platform::HostPlatform` (see the presentation-surface ADR).
//!
//! Replaces the old 128x32 `simple_gui` pipeline this file used to run
//! directly (retired in W7 — see `lib.rs`).
//!
//! # Usage
//!
//! Windowed (default): `cargo run --bin desktop --target <host-triple>`
//!
//! Headless: `cargo run --bin desktop --target <host-triple> -- --headless
//! [--dump-png PATH] [--frames N]`. `--dump-png` writes the framebuffer as
//! a PNG after `N` frames (default 1) and exits — the fast path for
//! automated/agent verification. Without `--dump-png`, headless mode just
//! runs the loop indefinitely (there is no headless input yet — the HTTP
//! `NavIntent` injection + screenshot protocol is bead W5 — but the
//! `InputSource` seam is already there for W5 to plug into: see
//! `platform::NoopInput`).
//!
//! The HTTP push server (`POST /api/sync`, `/api/status`, `/api/clear`,
//! `/api/shutdown`) keeps running in both modes exactly as before — it's
//! how a companion (or `curl`, or the Web Vault dev harness) gets
//! credentials onto the device; `PushSyncSource` wraps it as the app's
//! `SyncSource`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bhk_core::platform::Platform;
use bhk_core::{run, App, SyncSource};
use emulator::desktop::{DesktopStorage, PushSyncSource, SyncServer};
use emulator::platform::{FileStorage, HeadlessSurface, HostPlatform, MinifbSurface, NoopInput, WindowedInput};
use minifb::{Window, WindowOptions};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 170;
const WINDOW_SCALE: u32 = 3;
/// ~30fps: generous for a credential list (no animation), light on CPU for
/// a background/agent-driven headless run.
const FRAME_BUDGET: Duration = Duration::from_millis(33);

struct Args {
    headless: bool,
    dump_png: Option<String>,
    frames: u32,
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().collect();
    let headless = raw.iter().any(|a| a == "--headless");
    let dump_png = raw
        .iter()
        .position(|a| a == "--dump-png")
        .and_then(|i| raw.get(i + 1))
        .cloned();
    let frames = raw
        .iter()
        .position(|a| a == "--frames")
        .and_then(|i| raw.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    Args { headless, dump_png, frames }
}

fn main() {
    let args = parse_args();

    println!("Starting desktop emulator ({} mode)...", if args.headless { "headless" } else { "windowed" });

    let storage_backend = Arc::new(Mutex::new(DesktopStorage::new().expect("Failed to create credential storage")));
    let server = SyncServer::new("127.0.0.1:8080", storage_backend).expect("Failed to start HTTP server");
    let credentials_ref = server.get_credentials_ref();
    let shutdown_signal = server.get_shutdown_signal();

    std::thread::spawn(move || {
        println!("HTTP server running on http://127.0.0.1:8080");
        println!("Endpoints:");
        println!("  POST /api/sync - Sync credentials (CBOR)");
        println!("  GET  /api/status - Get server status");
        println!("  POST /api/clear - Clear credentials");
        println!("  POST /api/shutdown - Shutdown emulator");
        loop {
            if let Err(e) = server.handle_request() {
                eprintln!("HTTP server error: {e}");
            }
        }
    });

    let kv_storage = FileStorage::new_default().expect("Failed to open kv store");
    let mut sync_source = PushSyncSource::new(credentials_ref);
    let initial_items = sync_source.sync().expect("PushSyncSource::sync is Infallible");

    let mut app = App::new(WIDTH, HEIGHT, initial_items);

    if args.headless {
        run_headless(&mut app, &mut sync_source, kv_storage, &shutdown_signal, &args);
    } else {
        run_windowed(&mut app, &mut sync_source, kv_storage, &shutdown_signal);
    }

    println!("Emulator closed.");
}

fn run_headless(
    app: &mut App,
    sync_source: &mut PushSyncSource,
    storage: FileStorage,
    shutdown_signal: &Arc<std::sync::atomic::AtomicBool>,
    args: &Args,
) {
    let mut platform = HostPlatform::new(HeadlessSurface::new(), NoopInput::new(), storage);

    if let Some(path) = &args.dump_png {
        // Bounded run for automated/agent verification: N frames, then dump
        // and exit, rather than idling forever waiting for input that (until
        // W5's HTTP injection lands) will never arrive in this mode.
        let mut frame = 0u32;
        run(&mut platform, app, sync_source, FRAME_BUDGET, || {
            frame += 1;
            frame <= args.frames
        });
        platform.display().save_png(path).expect("failed to save headless PNG");
        println!("Wrote headless screenshot to {path} ({WIDTH}x{HEIGHT}, {} frame(s))", args.frames);
    } else {
        println!("Headless mode running (no --dump-png given; running until HTTP shutdown).");
        println!("Real headless input (HTTP NavIntent injection) lands in bead W5; this run mode currently only observes SyncSource updates.");
        run(&mut platform, app, sync_source, FRAME_BUDGET, || !shutdown_signal.load(Ordering::Relaxed));
    }
}

fn run_windowed(
    app: &mut App,
    sync_source: &mut PushSyncSource,
    storage: FileStorage,
    shutdown_signal: &Arc<std::sync::atomic::AtomicBool>,
) {
    println!("Controls: Arrow Up/Down (Prev/Next), Enter (Activate), Backspace/Esc (Back)");
    println!("Window size: {}x{} ({WINDOW_SCALE}x scale)", WIDTH * WINDOW_SCALE, HEIGHT * WINDOW_SCALE);

    let mut window = Window::new(
        "Bitwarden HW Key - Desktop Emulator",
        (WIDTH * WINDOW_SCALE) as usize,
        (HEIGHT * WINDOW_SCALE) as usize,
        WindowOptions::default(),
    )
    .unwrap_or_else(|e| panic!("Unable to create window: {e}"));
    window.set_target_fps(60);
    let window = Rc::new(RefCell::new(window));

    let display = MinifbSurface::new(Rc::clone(&window), WIDTH, HEIGHT, WINDOW_SCALE);
    let input = WindowedInput::new(Rc::clone(&window));
    let mut platform = HostPlatform::new(display, input, storage);

    println!("Emulator started!");

    let window_for_should_continue = Rc::clone(&window);
    run(&mut platform, app, sync_source, FRAME_BUDGET, || {
        window_for_should_continue.borrow().is_open() && !shutdown_signal.load(Ordering::Relaxed)
    });
}
