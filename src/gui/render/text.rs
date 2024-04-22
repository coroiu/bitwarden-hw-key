use crate::gui::{
    primitives::{Intersection, Rectangle},
    style::font::Font,
};

use super::{drawable::Drawable, image_buffer::ImageBuffer};

pub(crate) struct Text {
    string: String,
    font: &'static Font,
    // color: Color,
}

impl Text {
    pub fn new(string: String, font: &'static Font) -> Self {
        Text { string, font }
    }
}

impl Drawable for Text {
    fn draw(&self, target: &mut ImageBuffer, bounds: Rectangle) {
        for (i, c) in self.string.chars().enumerate() {
            let font_character = self.font.get_character(c);
            let character_bounds = Rectangle {
                x: bounds.x + i as i32 * font_character.image_buffer.width as i32,
                y: bounds.y,
                width: font_character.image_buffer.width as u32,
                height: font_character.image_buffer.height as u32,
            };
            let Intersection { bounds, .. } = bounds.intersect(character_bounds);

            if (bounds.width == 0) || (bounds.height == 0) {
                return;
            }

            font_character.image_buffer.draw(target, bounds);
        }
    }
}
