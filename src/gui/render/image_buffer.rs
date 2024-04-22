use crate::gui::primitives::{Color, Rectangle};

use super::drawable::Drawable;

pub(crate) struct ImageBuffer {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<Color>,
}

struct ImageBufferSlice<'a> {
    rectangle: Rectangle,
    buffer: &'a ImageBuffer,
}

impl ImageBuffer {
    pub(super) fn new(width: usize, height: usize) -> Self {
        ImageBuffer {
            width,
            height,
            pixels: vec![Color::black(); (width * height) as usize],
        }
    }

    fn sub_image(&self, rectangle: Rectangle) -> ImageBufferSlice {
        ImageBufferSlice {
            rectangle,
            buffer: self,
        }
    }
}

impl Drawable for ImageBuffer {
    fn draw(&self, target: &mut ImageBuffer, bounds: Rectangle) {}
}
