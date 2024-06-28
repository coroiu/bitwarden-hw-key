use super::{
    commands::RenderCommand, drawable::Drawable, image_buffer::ImageBuffer,
    solid_color::SolidColor, text::Text,
};

pub struct Canvas {
    pub image_buffer: ImageBuffer,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Canvas {
        return Canvas {
            image_buffer: ImageBuffer::new(width, height),
        };
    }

    pub fn clear(&mut self) {
        self.image_buffer.clear();
    }

    pub(crate) fn execute(&mut self, command: &RenderCommand) {
        match &command {
            RenderCommand::SolidColor(color, rect) => {
                SolidColor::new(*color).draw(&mut self.image_buffer, *rect);
            }
            // TODO: Implement re-coloring fonts
            RenderCommand::Text(_color, rect, text, font) => {
                Text::new(text.clone(), font).draw(&mut self.image_buffer, *rect);
            }
        }
    }
}
