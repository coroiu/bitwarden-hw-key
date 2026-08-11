use crate::gui::primitives::{Color, Rectangle};

use super::{drawable::Drawable, image_buffer::ImageBuffer};

pub(crate) struct SolidColor {
    color: Color,
}

impl SolidColor {
    pub fn new(color: Color) -> Self {
        SolidColor { color }
    }
}

impl Drawable for SolidColor {
    fn draw(&self, target: &mut ImageBuffer, bounds: Rectangle) {
        // Clip the rectangle to the target boundaries.
        let x0 = bounds.x.clamp(0, target.width as i32) as usize;
        let y0 = bounds.y.clamp(0, target.height as i32) as usize;
        let x1 = (bounds.x + bounds.width as i32).clamp(0, target.width as i32) as usize;
        let y1 = (bounds.y + bounds.height as i32).clamp(0, target.height as i32) as usize;

        for y in y0..y1 {
            for x in x0..x1 {
                // TODO: alpha compositing with existing pixels
                target.pixels[x + y * target.width as usize] = self.color;
            }
        }
    }
}
