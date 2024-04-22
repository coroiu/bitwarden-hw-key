use crate::gui::primitives::Rectangle;

use super::image_buffer::ImageBuffer;

pub(crate) trait Drawable {
    fn draw(&self, target: &mut ImageBuffer, bounds: Rectangle);
}
