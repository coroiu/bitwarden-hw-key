use crate::gui::primitives::Color;

#[derive(Default, Clone, Copy)]
pub struct Styles {
    pub display: Display,
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub width: Option<Size>,
    pub height: Option<Size>,
    pub margin: Option<EdgeSizes>,
    pub padding: Option<EdgeSizes>,
    pub border: Option<EdgeSizes>,
}

#[derive(Default, Clone, Copy)]
pub enum Display {
    #[default]
    Block,
    Flex,
    Inline,
    None,
}

#[derive(Debug, Default, Clone, Copy)]
pub enum Size {
    #[default]
    Auto,
    Pixels(i32),
    Percentage(f32),
}

impl Size {
    pub fn zero() -> Size {
        Size::Pixels(0)
    }

    pub fn to_pixels(&self, containing_block_size: i32) -> i32 {
        match self {
            Size::Auto => 0,
            Size::Pixels(px) => *px,
            Size::Percentage(percentage) => {
                (containing_block_size as f32 * percentage / 100.0) as i32
            }
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct EdgeSizes {
    pub top: Option<Size>,
    pub right: Option<Size>,
    pub left: Option<Size>,
    pub bottom: Option<Size>,
}
