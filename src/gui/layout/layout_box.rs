use crate::gui::{
    primitives::Rectangle,
    style::{styled_node::StyledNode, styles::Size},
};

pub(crate) struct LayoutBox<'a> {
    dimensions: Dimensions,
    pub(crate) box_type: BoxType<'a>,
    children: Vec<LayoutBox<'a>>,
}

#[derive(Clone, Copy)]
pub(crate) enum BoxType<'a> {
    BlockNode(&'a StyledNode<'a>),
    InlineNode(&'a StyledNode<'a>),
    AnonymousBlock,
}

#[derive(Default)]
pub(crate) struct Dimensions {
    content: Rectangle,
    padding: EdgeSizes,
    border: EdgeSizes,
    margin: EdgeSizes,
}

#[derive(Default)]
pub(crate) struct EdgeSizes {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

impl<'a> LayoutBox<'a> {
    // Constructor function
    pub(super) fn new(box_type: BoxType) -> LayoutBox {
        LayoutBox {
            box_type: box_type,
            dimensions: Default::default(),
            children: Vec::new(),
        }
    }

    pub(crate) fn children(&self) -> &Vec<LayoutBox> {
        &self.children
    }

    pub(crate) fn add_child(&mut self, child: LayoutBox<'a>) {
        match child.box_type {
            BoxType::InlineNode(_) => match self.box_type {
                BoxType::InlineNode(_) | BoxType::AnonymousBlock => self.children.push(child),
                BoxType::BlockNode(_) => {
                    // Where are a block node, so we need to create an anonymous block box to hold this inline box.
                    // Make sure the last child is an anonymous block box where we can put this inline box.
                    match self.children.last() {
                        Some(&LayoutBox {
                            box_type: BoxType::AnonymousBlock,
                            ..
                        }) => {}
                        _ => self.children.push(LayoutBox::new(BoxType::AnonymousBlock)),
                    };

                    self.children.last_mut().unwrap().add_child(child);
                }
            },
            BoxType::BlockNode(_) => {
                self.children.push(child);
            }
            _ => {}
        }
    }

    pub(crate) fn layout(&mut self, containing_block: Dimensions) {
        match self.box_type {
            BoxType::BlockNode(_) => self.layout_block(containing_block),
            BoxType::InlineNode(_) => {}
            BoxType::AnonymousBlock => {}
        }
    }

    fn layout_block(&mut self, containing_block: Dimensions) {
        self.calculate_block_width(containing_block);
        // self.calculate_block_position(containing_block);
        // self.layout_block_children();
        // self.calculate_block_height();
    }

    fn calculate_block_width(&mut self, containing_block: Dimensions) {
        let style = match self.box_type {
            BoxType::BlockNode(style_node) => &style_node.style,
            _ => panic!("calculate_block_width called on non-block"),
        };

        // We don't expect the content width to be so big that we overflow an i32
        let containing_width = containing_block.content.width.try_into().unwrap();

        // width defaults to auto
        let mut width = style.width.unwrap_or(Size::Auto);

        // margins, borders, and paddings default to 0
        let mut margin_left = style.margin.and_then(|m| m.left).unwrap_or(Size::zero());
        let mut margin_right = style.margin.and_then(|m| m.right).unwrap_or(Size::zero());

        let border_left = style.border.and_then(|b| b.left).unwrap_or(Size::zero());
        let border_right = style.border.and_then(|b| b.right).unwrap_or(Size::zero());

        let padding_left = style.padding.and_then(|p| p.left).unwrap_or(Size::zero());
        let padding_right = style.padding.and_then(|p| p.right).unwrap_or(Size::zero());

        let total: i32 = [
            &margin_left,
            &margin_right,
            &border_left,
            &border_right,
            &padding_left,
            &padding_right,
            &width,
        ]
        .iter()
        .map(|v| v.to_pixels(containing_width))
        .sum();

        // if width is not auto and the total is wider than the container, treat auto margins as 0
        if !matches!(width, Size::Auto) && total > containing_width as i32 {
            if matches!(width, Size::Auto) {
                margin_left = Size::zero();
            }
            if matches!(margin_right, Size::Auto) {
                margin_right = Size::zero();
            }
        }

        // Handle underflow/overflow
        let underflow = containing_width - total;
        match (width, margin_left, margin_right) {
            // If exactly only one margin is set to auto, then the box will overflow/underflow into that direction.
            (_, _, Size::Auto) => margin_right = Size::Pixels(underflow),
            (_, Size::Auto, _) => margin_left = Size::Pixels(underflow),

            // If the width is set to auto then it will expand to fill the underflow,
            // leaving auto margins with zero width.
            (Size::Auto, _, _) => {
                if matches!(margin_left, Size::Auto) {
                    margin_left = Size::zero();
                }
                if matches!(margin_right, Size::Auto) {
                    margin_right = Size::zero();
                }

                if underflow >= 0 {
                    // Fill underflow with width
                    width = Size::Pixels(underflow);
                } else {
                    // Negative width is not allowed, so set to 0, and make the right margin negative
                    width = Size::zero();
                    margin_right =
                        Size::Pixels(margin_right.to_pixels(containing_width) + underflow);
                }
            }

            // If margin-left and margin-right are both auto, they will share the underflow equally.
            // This will either center the box or make the box overflow equally on both sides.
            (_, Size::Auto, Size::Auto) => {
                margin_left = Size::Pixels(underflow / 2);
                margin_right = Size::Pixels(underflow / 2);
            }

            // If the values are overconstrained, calculate margin_right, because a block must always fill the width of its container.
            (_, _, _) => {
                margin_right = Size::Pixels(margin_right.to_pixels(containing_width) + underflow);
            }
        }
    }
}
