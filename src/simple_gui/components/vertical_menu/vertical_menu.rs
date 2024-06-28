use crate::simple_gui::{
    components::Component,
    font::Font,
    primitives::{Point, Rectangle},
};

use super::VerticalMenuItem;

pub struct VerticalMenu {
    bounds: Rectangle,
    font: &'static Font,
    items: Vec<VerticalMenuItem>,
    scroll: u32,
    padding: u32,
}

impl VerticalMenu {
    pub fn new(bounds: Rectangle, font: &'static Font) -> Self {
        Self {
            bounds,
            font,
            items: Vec::new(),
            scroll: 0,
            padding: 2,
        }
    }

    pub fn items(&self) -> &Vec<VerticalMenuItem> {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut Vec<VerticalMenuItem> {
        &mut self.items
    }
}

impl Component for VerticalMenu {
    fn layout(&mut self) {
        let x = self.bounds.x;
        let mut y = self.bounds.y - self.scroll as i32;

        for item in self.items.iter_mut() {
            item.set_position(Point::new(x, y));
            println!("item position: {:?}", (x, y));
            item.layout();

            y += item.size().height as i32 + self.padding as i32;
        }
    }

    fn draw(
        &self,
        bounds: Rectangle,
        commands: &mut Vec<crate::simple_gui::render::RenderCommand>,
    ) {
        for item in self.items.iter() {
            item.draw(bounds, commands);
        }
    }
}
