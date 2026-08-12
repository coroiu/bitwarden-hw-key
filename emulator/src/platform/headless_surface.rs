//! `HeadlessSurface`: a `DisplaySurface` with no window. It keeps the most
//! recently flushed framebuffer in RAM (as a flat `Vec<Rgb565>`, not a
//! `FrameBuffer565` clone — see the note below) and exposes PNG encoding
//! on demand, so an agent driving the emulator headlessly can request a
//! screenshot without a display server.
//!
//! `flush` itself can never fail (there is no real device I/O to fail
//! against — it's a RAM-to-RAM copy), so `Error = Infallible`.
//!
//! The pixel -> PNG conversion (`Rgb565::r() << 3`, etc.) is deliberately
//! identical to the one already proven in
//! `core/tests/render_png_dump.rs` and `core/examples/render_scene.rs`:
//! this is the same "expand each Rgb565 channel to 8 bits by left-shifting"
//! convention used everywhere else in this codebase that turns a
//! `FrameBuffer565` into an `image::RgbImage`. Reusing it here (rather than
//! inventing a second conversion) is exactly what makes the headless-vs-
//! windowed parity test in `emulator/tests/surface_parity.rs` meaningful.

// Same justification as `bhk_core::render`'s identical `#![allow]`: pixel
// counts here never approach `u32::MAX` on any display this project
// targets, so `usize -> u32` here can't realistically truncate.
#![allow(clippy::cast_possible_truncation)]

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use bhk_core::platform::{DisplaySurface, FrameBuffer565};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::RgbColor;
use embedded_graphics::Pixel;

struct CapturedFrame {
    width: u32,
    height: u32,
    /// Row-major, per `FrameBuffer565::pixels()`'s documented iteration
    /// order.
    pixels: Vec<Rgb565>,
}

#[derive(Default)]
pub struct HeadlessSurface {
    last_frame: Option<CapturedFrame>,
}

impl HeadlessSurface {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Encodes the most recently flushed framebuffer as a PNG, returning
    /// `None` if `flush` has never been called.
    ///
    /// # Panics
    ///
    /// Panics if in-memory PNG encoding fails, which should not be
    /// possible for a buffer built directly from a `FrameBuffer565` (no
    /// filesystem or format-mismatch failure modes apply here).
    #[must_use]
    pub fn encode_png(&self) -> Option<Vec<u8>> {
        let frame = self.last_frame.as_ref()?;
        let mut image = image::RgbImage::new(frame.width, frame.height);

        for (index, color) in frame.pixels.iter().enumerate() {
            let index = index as u32;
            let x = index % frame.width;
            let y = index / frame.width;
            image.put_pixel(x, y, image::Rgb([color.r() << 3, color.g() << 2, color.b() << 3]));
        }

        let mut buffer = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Png)
            .expect("in-memory PNG encode should never fail");
        Some(buffer)
    }

    /// Convenience wrapper: encode and write straight to a file, for the
    /// verification example and for manual inspection.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the file couldn't be written. Returns
    /// `Ok(())` as a no-op if `flush` has never been called (nothing to
    /// save yet) — callers that need to distinguish "no frame" from
    /// "saved" should use [`HeadlessSurface::encode_png`] directly.
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        match self.encode_png() {
            Some(bytes) => std::fs::write(path, bytes),
            None => Ok(()),
        }
    }
}

impl DisplaySurface for HeadlessSurface {
    type Error = Infallible;

    fn flush(&mut self, framebuffer: &FrameBuffer565) -> Result<(), Self::Error> {
        let width = framebuffer.width();
        let height = framebuffer.height();
        let pixels: Vec<Rgb565> = framebuffer.pixels().map(|Pixel(_, color)| color).collect();
        self.last_frame = Some(CapturedFrame { width, height, pixels });
        Ok(())
    }
}

/// A [`HeadlessSurface`] shared between the render-loop thread and the HTTP
/// server thread (W5): the render loop's `DisplaySurface::flush` and
/// `GET /api/screenshot` (served from a different thread — see
/// `emulator::desktop::http_server::SyncServer`) both need to see the
/// *same* captured frame, so this wraps the surface in an `Arc<Mutex<_>>`
/// rather than handing the loop an owned `HeadlessSurface` the HTTP server
/// has no way to read. Mirrors the `Arc<Mutex<Vec<Credential>>>` sharing
/// pattern `PushSyncSource`/`SyncServer` already use for credentials (see
/// `emulator::desktop::push_sync_source`).
#[derive(Clone, Default)]
pub struct SharedHeadlessSurface(Arc<Mutex<HeadlessSurface>>);

impl SharedHeadlessSurface {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Hands out a second owner of the same underlying `HeadlessSurface` —
    /// e.g. for `SyncServer::set_screenshot_surface` to read whatever
    /// frame this surface's `flush` most recently wrote, from a different
    /// thread.
    #[must_use]
    pub fn handle(&self) -> Arc<Mutex<HeadlessSurface>> {
        Arc::clone(&self.0)
    }
}

impl DisplaySurface for SharedHeadlessSurface {
    type Error = Infallible;

    fn flush(&mut self, framebuffer: &FrameBuffer565) -> Result<(), Self::Error> {
        self.0.lock().unwrap().flush(framebuffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::pixelcolor::WebColors;
    use embedded_graphics::prelude::{Point, Size};
    use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
    use embedded_graphics::{draw_target::DrawTarget, prelude::Primitive, Drawable};

    #[test]
    fn encode_png_returns_none_before_the_first_flush() {
        let surface = HeadlessSurface::new();
        assert!(surface.encode_png().is_none());
    }

    #[test]
    fn flush_then_encode_round_trips_a_solid_color_frame() {
        let mut framebuffer = FrameBuffer565::new(4, 4);
        framebuffer.clear(Rgb565::RED).unwrap();

        let mut surface = HeadlessSurface::new();
        surface.flush(&framebuffer).unwrap();

        let png_bytes = surface.encode_png().expect("frame was flushed");
        let decoded = image::load_from_memory(&png_bytes).unwrap().to_rgb8();
        assert_eq!(decoded.width(), 4);
        assert_eq!(decoded.height(), 4);
        let expected = image::Rgb([Rgb565::RED.r() << 3, Rgb565::RED.g() << 2, Rgb565::RED.b() << 3]);
        for pixel in decoded.pixels() {
            assert_eq!(*pixel, expected);
        }
    }

    #[test]
    fn flush_replaces_the_previously_captured_frame() {
        let mut red_framebuffer = FrameBuffer565::new(2, 2);
        red_framebuffer.clear(Rgb565::RED).unwrap();
        let mut blue_framebuffer = FrameBuffer565::new(2, 2);
        blue_framebuffer.clear(Rgb565::BLUE).unwrap();

        let mut surface = HeadlessSurface::new();
        surface.flush(&red_framebuffer).unwrap();
        surface.flush(&blue_framebuffer).unwrap();

        let png_bytes = surface.encode_png().unwrap();
        let decoded = image::load_from_memory(&png_bytes).unwrap().to_rgb8();
        let expected = image::Rgb([Rgb565::BLUE.r() << 3, Rgb565::BLUE.g() << 2, Rgb565::BLUE.b() << 3]);
        assert_eq!(*decoded.get_pixel(0, 0), expected);
    }

    #[test]
    fn preserves_pixel_positions_not_just_colors_present() {
        // Regression guard for the row-major reconstruction math in
        // `encode_png`: draw a single 1x1 rectangle away from the origin
        // and confirm it lands at the same coordinates in the decoded PNG.
        let mut framebuffer = FrameBuffer565::new(5, 5);
        Rectangle::new(Point::new(3, 1), Size::new(1, 1))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::GREEN))
            .draw(&mut framebuffer)
            .unwrap();

        let mut surface = HeadlessSurface::new();
        surface.flush(&framebuffer).unwrap();
        let decoded = image::load_from_memory(&surface.encode_png().unwrap()).unwrap().to_rgb8();

        let green = image::Rgb([Rgb565::GREEN.r() << 3, Rgb565::GREEN.g() << 2, Rgb565::GREEN.b() << 3]);
        assert_eq!(*decoded.get_pixel(3, 1), green);
        assert_eq!(*decoded.get_pixel(0, 0), image::Rgb([0, 0, 0]));
    }

    #[test]
    fn a_handle_observes_frames_flushed_through_the_shared_surface() {
        // This is the sharing guarantee W5 depends on: the render loop
        // owns a `SharedHeadlessSurface` and calls `flush` on it, while
        // the HTTP server thread only ever holds a `handle()` — it must
        // see exactly what the loop wrote, with no separate copy to fall
        // out of sync.
        let mut framebuffer = FrameBuffer565::new(2, 2);
        framebuffer.clear(Rgb565::CSS_HOT_PINK).unwrap();

        let mut surface = SharedHeadlessSurface::new();
        let handle = surface.handle();

        assert!(handle.lock().unwrap().encode_png().is_none(), "nothing flushed yet");

        surface.flush(&framebuffer).unwrap();

        let png_bytes = handle.lock().unwrap().encode_png().expect("the handle sees the flush");
        let decoded = image::load_from_memory(&png_bytes).unwrap().to_rgb8();
        let expected = image::Rgb([
            Rgb565::CSS_HOT_PINK.r() << 3,
            Rgb565::CSS_HOT_PINK.g() << 2,
            Rgb565::CSS_HOT_PINK.b() << 3,
        ]);
        assert_eq!(*decoded.get_pixel(0, 0), expected);
    }

    #[test]
    fn cloning_a_shared_surface_shares_the_same_underlying_frame() {
        let mut framebuffer = FrameBuffer565::new(2, 2);
        framebuffer.clear(Rgb565::BLUE).unwrap();

        let mut surface = SharedHeadlessSurface::new();
        let clone = surface.clone();

        surface.flush(&framebuffer).unwrap();

        // `clone` is a second owner of the same `Arc<Mutex<HeadlessSurface>>`,
        // not an independent copy — it must observe the flush `surface`
        // performed after the clone was made.
        assert!(clone.handle().lock().unwrap().encode_png().is_some());
    }
}
