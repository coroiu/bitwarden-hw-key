use std::collections::{HashMap, HashSet};

use crate::gui::{
    document::node::{ElementState, Node},
    primitives::Color,
};

#[derive(Default, Clone)]
pub struct Styles {
    pub display: Display,
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub border_color: Option<Color>,
    pub width: Option<Size>,
    pub height: Option<Size>,
    pub margin: Option<EdgeSizes>,
    pub padding: Option<EdgeSizes>,
    pub border: Option<EdgeSizes>,
    pub flex_direction: Option<FlexDirection>,
}

#[derive(Default, Clone)]
pub struct ElementStyles {
    pub base_styles: Styles,
    pub state_styles: HashMap<ElementState, Styles>,
}

#[derive(Default, Clone, Copy)]
pub enum Display {
    #[default]
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

// Convenience traits for working with styles
pub trait SizeFluentPixels {
    fn px(self) -> Size;
}

impl SizeFluentPixels for i32 {
    fn px(self) -> Size {
        Size::Pixels(self)
    }
}

pub trait SizeFluentPercentage {
    fn percent(self) -> Size;
}

impl SizeFluentPercentage for f32 {
    fn percent(self) -> Size {
        Size::Percentage(self)
    }
}

impl Styles {
    /// Combine two styles into one. If a rule exists in both then `other` will be prioritized
    pub(crate) fn merge_with(&self, other: &Styles) -> Styles {
        Styles {
            display: other.display,
            color: other.color.or(self.color),
            background_color: other.background_color.or(self.background_color),
            border_color: other.border_color.or(self.border_color),
            width: other.width.or(self.width),
            height: other.height.or(self.height),
            margin: other.margin.or(self.margin),
            padding: other.padding.or(self.padding),
            border: other.border.or(self.border),
            flex_direction: other.flex_direction.or(self.flex_direction),
        }
    }
}

impl ElementStyles {
    pub(crate) fn applicable_styles(&self, states: &HashSet<ElementState>) -> Styles {
        let mut applicable_styles = Styles::default();
        for state in states {
            if let Some(styles) = self.state_styles.get(state) {
                applicable_styles = applicable_styles.merge_with(styles);
            }
        }
        applicable_styles
    }
}
