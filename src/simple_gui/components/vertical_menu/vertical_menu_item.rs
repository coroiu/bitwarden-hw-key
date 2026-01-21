use crate::simple_gui::{
    components::{Component, Label},
    font::Font,
    primitives::{Color, Point, Rectangle, Size},
    render::RenderCommand,
};

pub struct VerticalMenuItem {
    label: Label,
    is_selected: bool,
    position: Point,
}

impl VerticalMenuItem {
    pub fn new(font: &'static Font, text: &str) -> Self {
        Self {
            label: Label::new(Point::zero(), font, text),
            is_selected: false,
            position: Point::zero(),
        }
    }

    pub fn size(&self) -> Size {
        let label_size = self.label.size();
        // Add padding for the border
        Size::new(label_size.width + 2, label_size.height + 2)
    }

    pub fn set_position(&mut self, position: Point) {
        self.position = position;
        // Offset label by 1 pixel for border
        self.label.set_position(Point::new(position.x + 1, position.y + 1));
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.is_selected = selected;
    }
}

impl Component for VerticalMenuItem {
    fn layout(&mut self) {
        self.label.layout();
    }

    fn draw(&self, bounds: Rectangle, commands: &mut Vec<RenderCommand>) {
        // Draw border if selected
        if self.is_selected {
            let size = self.size();
            let border_color = Color::white();
            let border_thickness = 1;

            // Top border
            commands.push(RenderCommand::SolidColor(
                border_color,
                Rectangle::new(
                    self.position.x,
                    self.position.y,
                    size.width,
                    border_thickness,
                ),
            ));

            // Bottom border
            commands.push(RenderCommand::SolidColor(
                border_color,
                Rectangle::new(
                    self.position.x,
                    self.position.y + size.height as i32 - border_thickness as i32,
                    size.width,
                    border_thickness,
                ),
            ));

            // Left border
            commands.push(RenderCommand::SolidColor(
                border_color,
                Rectangle::new(
                    self.position.x,
                    self.position.y,
                    border_thickness,
                    size.height,
                ),
            ));

            // Right border
            commands.push(RenderCommand::SolidColor(
                border_color,
                Rectangle::new(
                    self.position.x + size.width as i32 - border_thickness as i32,
                    self.position.y,
                    border_thickness,
                    size.height,
                ),
            ));
        }

        // Draw label
        self.label.draw(bounds, commands);
    }
}
