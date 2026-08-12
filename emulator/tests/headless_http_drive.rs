//! Proves the actual M0 testability deliverable this bead (W5) exists for:
//! an agent can drive the headless shell over HTTP, with no window and no
//! hardware, and *observe* the effect via a screenshot also served over
//! HTTP. See `.planning/decisions/2026-08-11-three-mode-testability.md`.
//!
//! This runs a real `emulator::desktop::SyncServer` (the same `tiny_http`
//! server `main.rs` starts in both run modes) end to end over a real TCP
//! socket -- nothing here is mocked at the HTTP layer -- interleaved with
//! real frames of `bhk_core::run::run` driving a real `bhk_core::App`
//! through a real `emulator::platform::HttpInput` /
//! `emulator::platform::SharedHeadlessSurface` pair, exactly as
//! `main.rs::run_headless` wires them.
//!
//! No HTTP client crate exists in this workspace (there was never a reason
//! to add one before this test), so [`http_request`] below speaks raw
//! HTTP/1.1 over a `TcpStream` rather than pulling one in for a single
//! test.

use std::convert::Infallible;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bhk_core::render::chrome::TITLE_BAR_HEIGHT;
use bhk_core::render::ROW_HEIGHT;
use bhk_core::{run, App, SyncSource, VaultItem};
use embedded_graphics::pixelcolor::{Rgb565, WebColors};
use embedded_graphics::prelude::RgbColor;
use emulator::desktop::{DesktopStorage, SyncServer};
use emulator::platform::{FileStorage, HostPlatform, HttpInput, SharedHeadlessSurface};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 170;

/// A `SyncSource` that always returns the exact same snapshot it was
/// constructed with. `App::step` only rebuilds (and resets selection)
/// when a sync actually changes the item list, so holding it fixed means
/// every re-render this test observes is driven purely by the injected
/// `NavIntent`, not incidental sync churn.
struct FixedSyncSource(Vec<VaultItem>);
impl SyncSource for FixedSyncSource {
    type Error = Infallible;
    fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
        Ok(self.0.clone())
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

/// Sends a minimal HTTP/1.1 request over a raw socket and returns
/// `(status_code, body)`. Sends `Connection: close`, which `tiny_http`
/// (the server this test talks to) honors by closing the socket once it
/// has written the response -- that's what lets this read the body with
/// a simple `read_to_end` instead of implementing chunked/keep-alive
/// framing.
fn http_request(addr: SocketAddr, method: &str, path: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(addr).expect("connect to the test SyncServer");
    stream.set_read_timeout(Some(Duration::from_secs(5))).expect("set a read timeout");

    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    request.extend_from_slice(body);
    stream.write_all(&request).expect("write the request");

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read the full response before the server closes the connection");

    let status_line_end = raw.iter().position(|&b| b == b'\r').expect("a status line ending in CRLF");
    let status_line = std::str::from_utf8(&raw[..status_line_end]).expect("status line is ASCII");
    let status: u16 = status_line.split_whitespace().nth(1).expect("status line has a code").parse().expect("status code is numeric");

    let header_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a well-formed HTTP response has a blank line after headers")
        + 4;

    (status, raw[header_end..].to_vec())
}

fn get(addr: SocketAddr, path: &str) -> (u16, Vec<u8>) {
    http_request(addr, "GET", path, b"")
}

fn post(addr: SocketAddr, path: &str, body: &[u8]) -> (u16, Vec<u8>) {
    http_request(addr, "POST", path, body)
}

/// Decodes a `GET /api/screenshot` response as an RGB8 image, failing with
/// a readable message (rather than a PNG-decode panic) if the server
/// didn't actually return image bytes -- e.g. a 404 body because
/// `set_screenshot_surface` wasn't wired up.
fn decode_screenshot(response: (u16, Vec<u8>)) -> image::RgbImage {
    let (status, body) = response;
    assert_eq!(status, 200, "GET /api/screenshot did not succeed: {}", String::from_utf8_lossy(&body));
    image::load_from_memory(&body).expect("screenshot body must be a valid PNG").to_rgb8()
}

#[test]
fn injecting_a_navintent_over_http_moves_the_selection_and_is_observable_in_the_next_screenshot() {
    // --- Assemble exactly what `main.rs::run_headless` assembles for
    // headless mode, just on an ephemeral port instead of the fixed 8080
    // so this test can run alongside a real emulator instance. ---
    let storage_backend = Arc::new(Mutex::new(DesktopStorage::new().expect("open credential storage")));
    let mut server = SyncServer::new("127.0.0.1:0", storage_backend).expect("start SyncServer on an ephemeral port");
    let addr = server.local_addr();
    let input_queue = server.get_input_queue_ref();
    let surface = SharedHeadlessSurface::new();
    server.set_screenshot_surface(surface.handle());

    std::thread::spawn(move || loop {
        // The test process exiting (dropping the listener) is the only
        // expected way this ever stops; nothing to assert on in this
        // thread itself.
        if server.handle_request().is_err() {
            break;
        }
    });

    let kv_storage_path = std::env::temp_dir().join(format!("bhk-headless-http-drive-test-{}.json", uuid::Uuid::new_v4()));
    let kv_storage = FileStorage::new(kv_storage_path).expect("open a temp kv store");
    let mut platform = HostPlatform::new(surface, HttpInput::new(input_queue), kv_storage);

    // Three items so a moved selection lands on a second, distinguishable
    // row rather than clamping back to the same one.
    let items = vec![vault_item("Bitwarden.com"), vault_item("GitHub"), vault_item("AWS Console")];
    let mut app = App::new(WIDTH, HEIGHT, items.clone());
    let mut sync = FixedSyncSource(items);

    // Pixel coordinates for row 0's and row 1's selection-highlight fill,
    // per `core/src/render/list.rs` (`ROW_HEIGHT`, row top = chrome
    // content top + index * ROW_HEIGHT) and `core/src/render/chrome.rs`
    // (`TITLE_BAR_HEIGHT` is the content area's top). `+2` samples safely
    // inside the row, away from its edges -- the same offset
    // `navigator.rs`'s own
    // `rendering_into_the_same_framebuffer_twice_does_not_leave_the_previous_frames_selection_highlight_behind`
    // test uses for the identical row-0 case.
    //
    // x=250 (not x=2): `CredentialListView` (bead `ai-bitwarden-hw-key-0v8.4`)
    // paints a 3px focus-accent bar at the left edge of a selected row
    // (x in [0,3)) in a different color than the row's plain fill, and
    // draws row text starting at x=4 -- either of which x=2/x=6 could land
    // on depending on the label. x=250 is comfortably past both of these
    // short labels' text and well clear of the accent bar, landing on the
    // plain fill/background every time.
    let row0_y = TITLE_BAR_HEIGHT + 2;
    let row1_y = TITLE_BAR_HEIGHT + ROW_HEIGHT + 2;
    let sample_x = 250;
    let highlight = Rgb565::CSS_DARK_SLATE_BLUE;
    let highlight_rgb8 = image::Rgb([highlight.r() << 3, highlight.g() << 2, highlight.b() << 3]);

    // --- Frame 1: render the initial state (no input yet) and observe it
    // via a real HTTP screenshot request. ---
    let mut iterations = 0;
    run(&mut platform, &mut app, &mut sync, Duration::from_millis(0), || {
        iterations += 1;
        iterations <= 1
    });

    let before = decode_screenshot(get(addr, "/api/screenshot"));
    assert_eq!(*before.get_pixel(sample_x, row0_y), highlight_rgb8, "row 0 starts selected");
    assert_ne!(*before.get_pixel(sample_x, row1_y), highlight_rgb8, "row 1 is not selected before any input");

    // --- Inject NavIntent::Next over HTTP -- the exact wire shape
    // `bhk_core::input::NavIntent`'s derived `Deserialize` expects for a
    // unit variant. ---
    let (status, body) = post(addr, "/api/input", b"\"Next\"");
    assert_eq!(status, 200, "POST /api/input did not succeed: {}", String::from_utf8_lossy(&body));

    // --- Frame 2: the render loop's `HttpInput::poll()` drains the queued
    // intent, `App::handle_input` forwards it to the navigator, the list
    // moves its selection, and the app re-renders -- no changes to
    // `bhk_core::run::run` itself were needed for any of this. ---
    let mut iterations = 0;
    run(&mut platform, &mut app, &mut sync, Duration::from_millis(0), || {
        iterations += 1;
        iterations <= 1
    });

    let after = decode_screenshot(get(addr, "/api/screenshot"));
    assert_ne!(
        *after.get_pixel(sample_x, row0_y),
        highlight_rgb8,
        "row 0's highlight must not still be showing after the injected Next moved the selection away"
    );
    assert_eq!(
        *after.get_pixel(sample_x, row1_y),
        highlight_rgb8,
        "row 1 must become selected once the HTTP-injected Next reaches the running App"
    );
}

#[test]
fn screenshot_is_a_404_when_no_headless_surface_is_registered() {
    // Windowed mode never calls `set_screenshot_surface`; this proves the
    // endpoint degrades to a clear 404 rather than panicking or hanging
    // when there's nothing to screenshot.
    let storage_backend = Arc::new(Mutex::new(DesktopStorage::new().expect("open credential storage")));
    let server = SyncServer::new("127.0.0.1:0", storage_backend).expect("start SyncServer on an ephemeral port");
    let addr = server.local_addr();

    std::thread::spawn(move || loop {
        if server.handle_request().is_err() {
            break;
        }
    });

    let (status, _body) = get(addr, "/api/screenshot");
    assert_eq!(status, 404);
}
