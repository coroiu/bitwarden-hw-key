use std::iter::repeat;

use super::commands::{build_render_commands, RenderCommand};
use crate::gui::{
    layout::layout_box::LayoutBox,
    primitives::{Color, Rectangle},
};

pub struct Canvas {
    pub pixels: Vec<Color>,
    pub width: usize,
    pub height: usize,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Canvas {
        let white = Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        return Canvas {
            pixels: repeat(white).take(width * height).collect(),
            width: width,
            height: height,
        };
    }

    fn paint_item(&mut self, item: &RenderCommand) {
        match item {
            &RenderCommand::SolidColor(color, rect) => {
                // Clip the rectangle to the canvas boundaries.
                let x0 = rect.x.clamp(0, self.width as i32) as usize;
                let y0 = rect.y.clamp(0, self.height as i32) as usize;
                let x1 = (rect.x + rect.width as i32).clamp(0, self.width as i32) as usize;
                let y1 = (rect.y + rect.height as i32).clamp(0, self.height as i32) as usize;

                for y in y0..y1 {
                    for x in x0..x1 {
                        // TODO: alpha compositing with existing pixel
                        self.pixels[x + y * self.width] = color;
                    }
                }
            }
        }
    }
}

pub fn paint(layout_root: &LayoutBox, bounds: Rectangle) -> Canvas {
    let display_list = build_render_commands(layout_root);
    let mut canvas = Canvas::new(bounds.width as usize, bounds.height as usize);
    for item in display_list {
        canvas.paint_item(&item);
    }
    return canvas;
}
