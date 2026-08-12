//! Throwaway manual-verification helper (bead `ai-bitwarden-hw-key-0v8.6`):
//! renders `CredentialDetailView` in its three visually distinct states —
//! password masked, password revealed, and the "gone" (deleted upstream)
//! state — each to a zoomed PNG, for a human/agent to eyeball against the
//! approved mockup (screens 02 "masked" / 03 "revealed"). Not part of the
//! build; follows the same "example dumps a PNG" convention as
//! `render_scene.rs`/`m1_design_language.rs`.
//!
//! Drives the *real* `bhk_core::App` (the same wiring `main.rs`/the
//! emulator use — list `on_activate` pushing a detail screen, `App::step`
//! updating the shared `VaultStore` in place) rather than hand-assembling
//! a `Navigator`, so this exercises the actual production seam bead
//! `ai-bitwarden-hw-key-0v8.6` wires up, not a parallel construction of it.
//!
//! Run with: `cargo run -p bhk-core --example detail_view_probe --target <host-triple>`
//! Writes `detail_masked.png`, `detail_revealed.png`, `detail_gone.png` to
//! the current directory, each upscaled 3x (nearest-neighbor) from the
//! native 320x170 framebuffer for easier close-up inspection.

use std::convert::Infallible;

use bhk_core::render::FrameBuffer565;
use bhk_core::{App, NavIntent, SyncSource, VaultItem};
use uuid::Uuid;

const ZOOM: u32 = 3;
const WIDTH: u32 = 320;
const HEIGHT: u32 = 170;

fn full_item() -> VaultItem {
    VaultItem {
        id: Uuid::new_v4(),
        name: "Bitwarden.com".to_string(),
        username: "andreas@bitwarden.com".to_string(),
        password: "correct-horse-battery-staple".to_string(),
        uri: Some("https://vault.bitwarden.com".to_string()),
        notes: Some("2FA backup codes are in the safe.".to_string()),
    }
}

/// A `SyncSource` that always reports an empty vault — used to simulate
/// "this credential was deleted upstream" via a real `App::step` call,
/// exactly the path a live sync would take in production.
struct EmptyVault;
impl SyncSource for EmptyVault {
    type Error = Infallible;
    fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
        Ok(vec![])
    }
}

fn main() {
    render_masked_and_revealed();
    render_gone();
    render_notes_scrolled_into_view();
}

/// Builds a real `App`, activates the (only, so already-selected)
/// credential to push its detail screen, moves internal field focus to
/// PASSWORD, and dumps both the masked and revealed states.
fn render_masked_and_revealed() {
    let mut app = App::new(WIDTH, HEIGHT, vec![full_item()]);

    app.handle_input(vec![NavIntent::Activate]); // list -> detail
    app.handle_input(vec![NavIntent::Next]); // Username -> Password

    dump_zoomed_png(app.render(), "detail_masked.png");

    app.handle_input(vec![NavIntent::Activate]); // reveal
    dump_zoomed_png(app.render(), "detail_revealed.png");

    println!("wrote detail_masked.png and detail_revealed.png (320x170 native, {ZOOM}x zoomed)");
}

/// Builds a real `App`, pushes the detail screen for the only credential,
/// then drives a `step()` with a `SyncSource` that reports an empty vault
/// — simulating a real sync deleting the credential while its detail
/// screen is still open — and dumps the resulting gone state.
fn render_gone() {
    let mut app = App::new(WIDTH, HEIGHT, vec![full_item()]);
    app.handle_input(vec![NavIntent::Activate]);

    let mut empty_vault = EmptyVault;
    app.step(&mut empty_vault);

    dump_zoomed_png(app.render(), "detail_gone.png");
    println!("wrote detail_gone.png (320x170 native, {ZOOM}x zoomed)");
}

/// Navigates focus all the way to NOTES (the last field) and dumps the
/// result — proving the field-list auto-scroll fix keeps the focused field
/// visible even though all four fields don't fit the content area at once.
fn render_notes_scrolled_into_view() {
    let mut app = App::new(WIDTH, HEIGHT, vec![full_item()]);
    app.handle_input(vec![NavIntent::Activate]); // list -> detail (Username focused)
    app.handle_input(vec![NavIntent::Next]); // Password
    app.handle_input(vec![NavIntent::Next]); // Website
    app.handle_input(vec![NavIntent::Next]); // Notes

    dump_zoomed_png(app.render(), "detail_notes_scrolled.png");
    println!("wrote detail_notes_scrolled.png (320x170 native, {ZOOM}x zoomed)");
}

fn dump_zoomed_png(framebuffer: &FrameBuffer565, path: &str) {
    use embedded_graphics::prelude::RgbColor;

    let width = framebuffer.width();
    let height = framebuffer.height();
    let mut image = image::RgbImage::new(width, height);

    for pixel in framebuffer.pixels() {
        let point = pixel.0;
        let color = pixel.1;
        #[allow(clippy::cast_sign_loss)]
        image.put_pixel(
            point.x as u32,
            point.y as u32,
            image::Rgb([color.r() << 3, color.g() << 2, color.b() << 3]),
        );
    }

    let zoomed = image::imageops::resize(&image, width * ZOOM, height * ZOOM, image::imageops::FilterType::Nearest);
    zoomed.save(path).expect("failed to write PNG");
}
