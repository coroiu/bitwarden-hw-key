use crate::simple_gui::{
    components::{Component, Label},
    font::Font,
    primitives::{Point, Rectangle, Size},
    render::RenderCommand,
};

pub struct VerticalMenuItem {
    label: Label,
}

impl VerticalMenuItem {
    pub fn new(font: &'static Font, text: &str) -> Self {
        Self {
            label: Label::new(Point::zero(), font, text),
        }
    }

    pub fn size(&self) -> Size {
        self.label.size()
    }

    pub fn set_position(&mut self, position: Point) {
        self.label.set_position(position);
    }
}

impl Component for VerticalMenuItem {
    fn layout(&mut self) {
        self.label.layout();
    }

    fn draw(&self, bounds: Rectangle, commands: &mut Vec<RenderCommand>) {
        self.label.draw(bounds, commands);
    }
}
