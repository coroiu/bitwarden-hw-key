use crate::gui::primitives::{Color, Intersection, Rectangle};

use super::drawable::Drawable;

#[derive(Clone)]
pub(crate) struct ImageBuffer {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<Color>,
}

impl ImageBuffer {
    pub(super) fn new(width: usize, height: usize) -> Self {
        ImageBuffer {
            width,
            height,
            pixels: vec![Color::black(); (width * height) as usize],
        }
    }

    pub(super) fn bounds(&self) -> Rectangle {
        Rectangle::new(0, 0, self.width as u32, self.height as u32)
    }
}

impl Drawable for ImageBuffer {
    fn draw(&self, target: &mut ImageBuffer, bounds: Rectangle) {
        // Clamp the bounds to the target image buffer so we don't try to draw outside of it.
        let Intersection {
            bounds,
            // This is the offset in the source image buffer.
            offset_a: offset_source,
            // This is the offset from screen origin to where we should start drawing.
            // Because the target bounds always start at 0,0, this is always the same as bounds.x,y.
            offset_b: offset_target,
        } = bounds.intersect(target.bounds());

        // Clamp the bounds to the source image buffer so we don't try to read outside of it.
        let Intersection { bounds, .. } = bounds.intersect(Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: self.width as u32,
            height: self.height as u32,
        });

        for y in 0..bounds.height as i32 {
            for x in 0..bounds.width as i32 {
                let target_x = x + offset_target.x;
                let target_y = y + offset_target.y;

                let source_x = x + offset_source.x;
                let source_y = y + offset_source.y;

                let target_index = (target_x + target_y * target.width as i32) as usize;
                let source_index = (source_x + source_y * self.width as i32) as usize;

                target.pixels[target_index] = self.pixels[source_index];
            }
        }
    }
}
