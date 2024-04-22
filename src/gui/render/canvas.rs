use super::{
    commands::{build_render_commands, RenderCommand},
    drawable::Drawable,
    image_buffer::ImageBuffer,
    solid_color::SolidColor,
};
use crate::gui::{layout::layout_box::LayoutBox, primitives::Rectangle};

pub struct Canvas {
    pub image_buffer: ImageBuffer,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Canvas {
        return Canvas {
            image_buffer: ImageBuffer::new(width, height),
        };
    }

    fn execute(&mut self, command: &RenderCommand) {
        match &command {
            RenderCommand::SolidColor(color, rect) => {
                SolidColor::new(*color).draw(&mut self.image_buffer, *rect);
            }
            // TODO: Implement re-coloring fonts
            RenderCommand::Text(_color, rect, text, font) => {
                let first_character = text.chars().next().unwrap();
                let font_character = font.get_character(first_character);

                font_character
                    .image_buffer
                    .draw(&mut self.image_buffer, *rect);
            }
        }
    }
}

pub fn draw(layout_root: &LayoutBox, bounds: Rectangle) -> Canvas {
    let display_list = build_render_commands(layout_root);
    let mut canvas = Canvas::new(bounds.width as usize, bounds.height as usize);
    for item in display_list {
        canvas.execute(&item);
    }
    return canvas;
}
