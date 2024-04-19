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
    pub flex_direction: Option<FlexDirection>,
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

impl From<i32> for Size {
    fn from(px: i32) -> Self {
        Size::Pixels(px)
    }
}

#[derive(Default, Clone, Copy)]
pub struct EdgeSizes {
    pub top: Option<Size>,
    pub right: Option<Size>,
    pub bottom: Option<Size>,
    pub left: Option<Size>,
}

impl EdgeSizes {
    pub fn zero() -> EdgeSizes {
        EdgeSizes {
            top: Some(Size::zero()),
            right: Some(Size::zero()),
            bottom: Some(Size::zero()),
            left: Some(Size::zero()),
        }
    }

    pub fn all(size: Size) -> EdgeSizes {
        EdgeSizes {
            top: Some(size),
            right: Some(size),
            bottom: Some(size),
            left: Some(size),
        }
    }

    pub fn axis(vertical: Size, horizontal: Size) -> EdgeSizes {
        EdgeSizes {
            top: Some(vertical),
            right: Some(horizontal),
            bottom: Some(vertical),
            left: Some(horizontal),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
}
