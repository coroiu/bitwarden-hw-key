use crate::gui::primitives::Rectangle;

use super::image_buffer::ImageBuffer;

pub(crate) trait Drawable {
    /// Draw the drawable object to the target image buffer.
    /// The bounds parameter specifies the region of the target image buffer to draw to.
    fn draw(&self, target: &mut ImageBuffer, bounds: Rectangle);
}
