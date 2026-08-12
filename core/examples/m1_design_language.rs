//! Verification aid for bead `ai-bitwarden-hw-key-0v8.5` (the M1 visual
//! design language applied to the list + chrome): renders the two
//! mockup-parity scenes — a focused, multi-row credential list, and the
//! empty-vault content state — through the *real* production path
//! (`bhk_core::App`, i.e. `CredentialListView` + `Screen`'s chrome
//! rendering), not the generic `VerticalList` demo `render_scene.rs` uses,
//! and dumps each to a PNG for zoomed visual review against
//! `.worktrees/design-review-m1-mockups/docs/mockups/01_list_focused_amazon*`
//! and `04_empty_state*`.
//!
//! Run with:
//! `cargo run -p bhk-core --example m1_design_language --target <host-triple>`
//! Writes `m1_list_focused.png` and `m1_empty_state.png` to the current
//! directory.

use bhk_core::render::FrameBuffer565;
use bhk_core::{App, NavIntent, VaultItem};

fn item(name: &str, username: &str) -> VaultItem {
    VaultItem {
        id: uuid::Uuid::new_v4(),
        name: name.to_string(),
        username: username.to_string(),
        password: "hunter2".to_string(),
        uri: None,
        notes: None,
    }
}

fn dump_png(framebuffer: &FrameBuffer565, path: &str) {
    use embedded_graphics::prelude::RgbColor;

    let width = framebuffer.width();
    let height = framebuffer.height();
    let mut image = image::RgbImage::new(width, height);

    for pixel in framebuffer.pixels() {
        let point = pixel.0;
        let color = pixel.1;
        // See the identical comment in `examples/render_scene.rs`:
        // coordinates from this iterator are always non-negative.
        #[allow(clippy::cast_sign_loss)]
        image.put_pixel(
            point.x as u32,
            point.y as u32,
            image::Rgb([color.r() << 3, color.g() << 2, color.b() << 3]),
        );
    }

    image.save(path).expect("failed to write PNG");
    println!("wrote {path} ({width}x{height})");
}

fn main() {
    let items = vec![
        item("GitHub", "octocat@example.com"),
        item("Amazon Web Services", "andreas@bitwarden.com"),
        item("Postgres (prod)", "admin"),
        item("Cloudflare", "acoroiu"),
        item("Figma", "acoroiu@bitwarden.com"),
    ];
    let mut app = App::new(320, 170, items);
    // Select row 1 ("Amazon Web Services"), matching the approved mockup's
    // focused row so this is a like-for-like visual comparison.
    app.handle_input(vec![NavIntent::Next]);
    dump_png(app.render(), "m1_list_focused.png");

    let mut empty_app = App::new(320, 170, vec![]);
    dump_png(empty_app.render(), "m1_empty_state.png");
}
