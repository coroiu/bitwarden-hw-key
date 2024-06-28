use crate::simple_gui::{
    primitives::{Color, Point, Rectangle, Size},
    render::RenderCommand,
    style::font::Font,
    utils::LowerBound,
};

use super::Component;

pub struct Label {
    position: Point,
    font: &'static Font,
    text: String,

    preferred_bounds: Rectangle,
}

impl Label {
    pub fn new(position: Point, font: &'static Font, text: &str) -> Label {
        Label {
            position,
            font,
            text: text.to_owned(),

            preferred_bounds: Rectangle::new(position.x, position.y, 0, 0),
        }
    }

    pub fn size(&self) -> Size {
        let width = self
            .text
            .chars()
            .map(|c| self.font.get_character(c).image_buffer.width as u32)
            .sum::<u32>()
            + (self.text.chars().count() - 1).lower_bound(0) as u32
                * self.font.letter_spacing as u32;
        let height = self
            .text
            .chars()
            .map(|c| self.font.get_character(c).image_buffer.height as u32)
            .max()
            .unwrap_or(0);

        Size::new(width, height)
    }

    pub fn set_position(&mut self, position: Point) {
        self.position = position;
    }
}

impl Component for Label {
    fn layout(&mut self) {
        let Size { width, height } = self.size();
        let character_bounds = Rectangle::new(self.position.x, self.position.y, width, height);

        self.preferred_bounds = character_bounds;
    }

    fn draw(&self, bounds: Rectangle, commands: &mut Vec<RenderCommand>) {
        commands.push(RenderCommand::Text(
            Color::white(),
            bounds.intersect(self.preferred_bounds).bounds,
            self.text.clone(),
            self.font,
        ));
    }
}
