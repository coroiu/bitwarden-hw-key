//! The canonical Rgb565 in-RAM framebuffer: the app core's single render
//! output, per the presentation-surface ADR. Every run mode (headless,
//! windowed, real-target) differs only in how it *flushes* this buffer to a
//! physical or virtual display (`DisplaySurface::flush`, implemented in
//! later beads); the buffer itself, and everything that draws into it, is
//! platform-free and lives here in `bhk-core`.
//!
//! Backed by `embedded-graphics-framebuf`, whose `FrameBuf<C, B>` is generic
//! over a storage backend `B`. The upstream crate only ships backends for
//! fixed-size arrays (`[C; N]` / `&mut [C; N]`), which would force a
//! compile-time resolution into this crate — exactly what the render core
//! must avoid (no `128`/`320`/`170` literals baked into widgets or the
//! buffer type). [`HeapBuffer`] is a small local newtype implementing
//! `FrameBufferBackend` over a `Vec<Rgb565>` instead, so [`FrameBuffer565`]
//! can be sized at runtime from whatever `DisplaySurface` reports.
//!
//! See: .planning/decisions/2026-08-11-presentation-surface-run-mode-seam.md

use std::convert::Infallible;

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::OriginDimensions,
    pixelcolor::Rgb565,
    prelude::{Point, Size},
    Pixel,
};
use embedded_graphics_framebuf::{backends::FrameBufferBackend, FrameBuf};

/// Heap-backed storage for [`FrameBuf`]. Local newtype required by Rust's
/// orphan rules: neither `FrameBufferBackend` (foreign trait, from
/// `embedded-graphics-framebuf`) nor `Vec<Rgb565>` (foreign type, from
/// `std`) is defined in this crate, so a direct `impl` is not allowed.
struct HeapBuffer(Vec<Rgb565>);

impl FrameBufferBackend for HeapBuffer {
    type Color = Rgb565;

    fn set(&mut self, index: usize, color: Self::Color) {
        self.0[index] = color;
    }

    fn get(&self, index: usize) -> Self::Color {
        self.0[index]
    }

    fn nr_elements(&self) -> usize {
        self.0.len()
    }
}

/// The shared Rgb565 framebuffer the render core draws into and a
/// `DisplaySurface` implementation flushes out. Resolution is a runtime
/// parameter (see [`FrameBuffer565::new`]) — nothing in this type, or in
/// anything that draws into it, hardcodes a specific panel's dimensions.
pub struct FrameBuffer565 {
    inner: FrameBuf<Rgb565, HeapBuffer>,
}

impl FrameBuffer565 {
    /// Allocates a new framebuffer of the given size, cleared to black.
    ///
    /// # Panics
    ///
    /// Panics if `width * height` overflows `usize` (not a realistic
    /// concern for any display this project targets).
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let pixel_count = width as usize * height as usize;
        let data = vec![Rgb565::default(); pixel_count];
        Self {
            inner: FrameBuf::new(HeapBuffer(data), width as usize, height as usize),
        }
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.inner.width() as u32
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.inner.height() as u32
    }

    /// Reads back a single pixel. Primarily for tests and the PNG-dump
    /// verification path (see `examples/render_scene.rs`); the render path
    /// itself only ever writes via `DrawTarget`.
    #[must_use]
    pub fn pixel(&self, p: Point) -> Rgb565 {
        self.inner.get_color_at(p)
    }

    /// Iterates every pixel in the buffer in row-major order, for surfaces
    /// (or tests) that need to walk the whole framebuffer, e.g. to encode a
    /// PNG or hand pixels to `minifb`.
    pub fn pixels(&self) -> impl Iterator<Item = Pixel<Rgb565>> + '_ {
        self.inner.into_iter()
    }
}

impl OriginDimensions for FrameBuffer565 {
    fn size(&self) -> Size {
        self.inner.size()
    }
}

/// Bound to `Error = Infallible`: only the surface adapters (later beads)
/// talk to fallible hardware; the core's draw path itself can never fail.
/// See: .planning/decisions/2026-08-11-presentation-surface-run-mode-seam.md
impl DrawTarget for FrameBuffer565 {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.inner.draw_iter(pixels)
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.inner.clear(color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::{
        prelude::{Primitive, RgbColor},
        primitives::{PrimitiveStyle, Rectangle},
        Drawable,
    };

    #[test]
    fn new_is_sized_correctly_and_cleared_to_black() {
        let fb = FrameBuffer565::new(37, 11);
        assert_eq!(fb.width(), 37);
        assert_eq!(fb.height(), 11);
        assert_eq!(fb.pixel(Point::new(0, 0)), Rgb565::BLACK);
        assert_eq!(fb.pixel(Point::new(36, 10)), Rgb565::BLACK);
    }

    #[test]
    fn drawing_a_rect_sets_pixels_inside_and_leaves_outside_untouched() {
        let mut fb = FrameBuffer565::new(10, 10);
        Rectangle::new(Point::new(2, 2), Size::new(3, 3))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
            .draw(&mut fb)
            .unwrap();

        assert_eq!(fb.pixel(Point::new(2, 2)), Rgb565::RED);
        assert_eq!(fb.pixel(Point::new(4, 4)), Rgb565::RED);
        assert_eq!(fb.pixel(Point::new(5, 5)), Rgb565::BLACK);
        assert_eq!(fb.pixel(Point::new(0, 0)), Rgb565::BLACK);
    }

    #[test]
    fn resolution_is_a_runtime_parameter_not_a_compile_time_constant() {
        // Two arbitrary, non-standard sizes prove the type isn't secretly
        // bound to any particular panel's dimensions.
        let a = FrameBuffer565::new(13, 5);
        let b = FrameBuffer565::new(401, 233);
        assert_eq!(a.width() * a.height(), 65);
        assert_eq!(b.width() * b.height(), 401 * 233);
    }

    #[test]
    fn pixels_iterates_in_row_major_order() {
        let mut fb = FrameBuffer565::new(2, 2);
        fb.inner.set_color_at(Point::new(0, 0), Rgb565::RED);
        fb.inner.set_color_at(Point::new(1, 0), Rgb565::GREEN);
        fb.inner.set_color_at(Point::new(0, 1), Rgb565::BLUE);
        fb.inner.set_color_at(Point::new(1, 1), Rgb565::WHITE);

        let colors: Vec<Rgb565> = fb.pixels().map(|Pixel(_, c)| c).collect();
        assert_eq!(colors, vec![Rgb565::RED, Rgb565::GREEN, Rgb565::BLUE, Rgb565::WHITE]);
    }
}
