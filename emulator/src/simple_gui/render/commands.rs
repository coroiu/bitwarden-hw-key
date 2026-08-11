use crate::simple_gui::{
    primitives::{Color, Rectangle},
    style::font::Font,
};

pub enum RenderCommand {
    SolidColor(Color, Rectangle),
    Text(Color, Rectangle, String, &'static Font),
}
