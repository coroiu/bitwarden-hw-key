//! Verifiable deliverable for W3: renders a titled screen with a short
//! vertical list of `VaultItem`-like rows into the in-RAM Rgb565
//! framebuffer, then dumps that framebuffer to a PNG — proving the render
//! core works end-to-end without needing any of the (not-yet-built)
//! windowed/headless/board `DisplaySurface`s.
//!
//! Run with: `cargo run -p bhk-core --example render_scene`
//! Writes `render_scene.png` to the current directory.

use bhk_core::render::{FrameBuffer565, ListItem, Navigator, Screen, VerticalList};

fn main() {
    let items = vec![
        ListItem::new("Bitwarden.com").with_sublabel("alice@example.com"),
        ListItem::new("GitHub").with_sublabel("alice-dev"),
        ListItem::new("AWS Console").with_sublabel("alice@corp.io"),
        ListItem::new("Postgres (prod)").with_sublabel("svc-account"),
    ];
    let list = VerticalList::new(items);
    let root = Screen::new("Vault", vec![Box::new(list)]).with_hint("Next/Prev  Select  Back");
    let mut navigator = Navigator::new(root);

    // Move selection once, so the PNG visibly shows the selection highlight
    // on a row other than the first — cheap extra proof that on_intent
    // dispatch actually affects what gets rendered.
    navigator.dispatch(bhk_core::NavIntent::Next);

    let mut framebuffer = FrameBuffer565::new(320, 170);
    navigator.render(&mut framebuffer).expect("core DrawTarget is Infallible");

    let path = "render_scene.png";
    dump_png(&framebuffer, path);
    println!("wrote {path} ({}x{})", framebuffer.width(), framebuffer.height());
}

fn dump_png(framebuffer: &FrameBuffer565, path: &str) {
    use embedded_graphics::prelude::RgbColor;

    let width = framebuffer.width();
    let height = framebuffer.height();
    let mut image = image::RgbImage::new(width, height);

    for pixel in framebuffer.pixels() {
        let point = pixel.0;
        let color = pixel.1;
        // `Point` coordinates from a `FrameBuffer565::pixels()` iterator
        // are always in `[0, width)` / `[0, height)`, i.e. non-negative,
        // by construction of the iterator (see `PixelIterator` in
        // `embedded-graphics-framebuf`).
        #[allow(clippy::cast_sign_loss)]
        image.put_pixel(
            point.x as u32,
            point.y as u32,
            image::Rgb([color.r() << 3, color.g() << 2, color.b() << 3]),
        );
    }

    image.save(path).expect("failed to write PNG");
}
