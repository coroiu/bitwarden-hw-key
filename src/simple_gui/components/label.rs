use crate::simple_gui::{
    primitives::{Color, Point, Rectangle},
    render::RenderCommand,
    style::font::Font,
    utils::LowerBound,
};

use super::Component;

pub struct Label {
    position: Point,
    font: &'static Font,
    text: String,

    calulated_bounds: Rectangle,
}

impl Label {
    pub fn new(position: Point, font: &'static Font, text: &str) -> Label {
        Label {
            position,
            font,
            text: text.to_owned(),

            calulated_bounds: Rectangle::new(position.x, position.y, 0, 0),
        }
    }
}

impl Component for Label {
    fn layout(&mut self) {
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
            .map(|c| self.font.get_character(c).image_buffer.height)
            .max()
            .unwrap_or(0);

        let character_bounds = Rectangle {
            x: self.position.x,
            y: self.position.y,
            width: width,
            height: height as u32,
        };

        self.calulated_bounds = character_bounds;
    }

    fn draw(&self, bounds: Rectangle, commands: &mut Vec<RenderCommand>) {
        commands.push(RenderCommand::Text(
            Color::white(),
            bounds,
            self.text.clone(),
            self.font,
        ));
    }

    fn get_bounds(&self) -> Rectangle {
        self.calulated_bounds
    }
}
