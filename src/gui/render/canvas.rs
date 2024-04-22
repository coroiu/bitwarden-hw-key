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

    fn paint_command(&mut self, command: &RenderCommand) {
        match &command {
            RenderCommand::SolidColor(color, rect) => {
                SolidColor::new(*color).draw(&mut self.image_buffer, *rect);
            }
            RenderCommand::Text(color, rect, text, font) => {
                // let first_character = text.chars().next().unwrap();
                // let font_character = font.get_character(first_character);

                // let x0 = rect.x.clamp(0, self.width as i32) as usize;
                // let y0 = rect.y.clamp(0, self.height as i32) as usize;
                // let x1 = (rect.x + rect.width as i32).clamp(0, self.width as i32) as usize;
                // let y1 = (rect.y + rect.height as i32).clamp(0, self.height as i32) as usize;

                // for y in y0..y1 {
                //     for x in x0..x1 {
                //         // TODO: alpha compositing with existing pixe
                //         self.pixels[x + y * self.width] = *color;
                //     }
                // }
            }
        }
    }
}

pub fn paint(layout_root: &LayoutBox, bounds: Rectangle) -> Canvas {
    let display_list = build_render_commands(layout_root);
    let mut canvas = Canvas::new(bounds.width as usize, bounds.height as usize);
    for item in display_list {
        canvas.paint_command(&item);
    }
    return canvas;
}
