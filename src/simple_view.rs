use embedded_graphics::{pixelcolor::BinaryColor, Drawable};

use crate::simple_gui::{
    components::{Label, VerticalMenu, VerticalMenuItem},
    font,
    primitives::{Point, Rectangle},
    Canvas, Document,
};

pub fn create_view(width: u32, height: u32) -> Document {
    let mut document = Document::new(width, height);

    // document.components_mut().push(Box::new(Label::new(
    //     Point::zero(),
    //     &font::FONT_5X8,
    //     "Hello world!",
    // )));

    let mut menu = VerticalMenu::new(Rectangle::new(0, 0, 128, 64), &font::FONT_5X8);

    menu.items_mut()
        .push(VerticalMenuItem::new(&font::FONT_5X8, "Item 1"));
    menu.items_mut()
        .push(VerticalMenuItem::new(&font::FONT_5X8, "Item 2"));
    menu.items_mut()
        .push(VerticalMenuItem::new(&font::FONT_5X8, "Item 3"));
    menu.items_mut()
        .push(VerticalMenuItem::new(&font::FONT_5X8, "Item 4"));
    menu.items_mut()
        .push(VerticalMenuItem::new(&font::FONT_5X8, "Item 5"));

    document.components_mut().push(Box::new(menu));

    document
}

impl Drawable for Canvas {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        let pixels = self
            .image_buffer
            .pixels
            .iter()
            .enumerate()
            .map(|(i, color)| {
                let x = i % self.image_buffer.width;
                let y = i / self.image_buffer.width;

                let combined_colors = color.r as i32 + color.g as i32 + color.b as i32;
                let mapped_color = match combined_colors {
                    c if c > 300 => BinaryColor::On,
                    _ => BinaryColor::Off,
                };

                embedded_graphics::Pixel(
                    embedded_graphics::geometry::Point::new(x as i32, y as i32),
                    mapped_color,
                )
            });

        target.draw_iter(pixels)
    }
}
