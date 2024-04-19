use crate::gui::{
    primitives::{Edges, Rectangle},
    style::{styled_node::StyledNode, styles::Size},
};

pub(crate) struct LayoutBox<'a> {
    pub(crate) dimensions: Dimensions,
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
    pub(crate) content: Rectangle,
    pub(crate) padding: Edges,
    pub(crate) border: Edges,
    pub(crate) margin: Edges,
}

impl Dimensions {
    // The area covered by the content area plus its padding.
    fn padding_box(&self) -> Rectangle {
        self.content.expand(&self.padding)
    }

    // The area covered by the content area plus padding and borders.
    fn border_box(&self) -> Rectangle {
        self.padding_box().expand(&self.border)
    }

    // The area covered by the content area plus padding, borders, and margin.
    fn margin_box(&self) -> Rectangle {
        self.border_box().expand(&self.margin)
    }
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

    pub(crate) fn layout(&mut self, bounds: Rectangle) {
        let containing_block = Dimensions {
            content: bounds,
            padding: Default::default(),
            border: Default::default(),
            margin: Default::default(),
        };

        self.layout_as_child(&containing_block);
    }

    fn layout_as_child(&mut self, containing_block: &Dimensions) {
        match self.box_type {
            BoxType::BlockNode(_) => self.layout_block(containing_block),
            BoxType::InlineNode(_) => {}
            BoxType::AnonymousBlock => {}
        }
    }

    fn layout_block(&mut self, containing_block: &Dimensions) {
        self.calculate_block_width(containing_block);
        self.calculate_vertical_edges(containing_block);
        self.calculate_block_position(containing_block);
        self.layout_block_children();
        self.calculate_block_height(containing_block);
    }

    fn calculate_block_width(&mut self, containing_block: &Dimensions) {
        let style = match self.box_type {
            BoxType::BlockNode(style_node) => &style_node.style,
            _ => panic!("cannot extract styles from called on non-block"),
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

        let d = &mut self.dimensions;
        d.content.width = width
            .to_pixels(containing_width)
            .try_into()
            .unwrap_or_default();

        d.padding.left = padding_left.to_pixels(containing_width);
        d.padding.right = padding_right.to_pixels(containing_width);

        d.border.left = border_left.to_pixels(containing_width);
        d.border.right = border_right.to_pixels(containing_width);

        d.margin.left = margin_left.to_pixels(containing_width);
        d.margin.right = margin_right.to_pixels(containing_width);
    }

    fn calculate_vertical_edges(&mut self, containing_block: &Dimensions) {
        let style = match self.box_type {
            BoxType::BlockNode(style_node) => &style_node.style,
            _ => panic!("cannot extract styles from called on non-block"),
        };

        let containing_width = containing_block.content.width.try_into().unwrap();
        let d = &mut self.dimensions;

        d.margin.top = style
            .margin
            .and_then(|m| m.top)
            .map(|v| v.to_pixels(containing_width))
            .unwrap_or(0);
        d.margin.bottom = style
            .margin
            .and_then(|m| m.bottom)
            .map(|v| v.to_pixels(containing_width))
            .unwrap_or(0);

        d.border.top = style
            .border
            .and_then(|m| m.top)
            .map(|v| v.to_pixels(containing_width))
            .unwrap_or(0);
        d.border.bottom = style
            .border
            .and_then(|m| m.bottom)
            .map(|v| v.to_pixels(containing_width))
            .unwrap_or(0);

        d.padding.top = style
            .padding
            .and_then(|m| m.top)
            .map(|v| v.to_pixels(containing_width))
            .unwrap_or(0);
        d.padding.bottom = style
            .padding
            .and_then(|m| m.bottom)
            .map(|v| v.to_pixels(containing_width))
            .unwrap_or(0);
    }

    fn calculate_block_position(&mut self, containing_block: &Dimensions) {
        // let style = match self.box_type {
        //     BoxType::BlockNode(style_node) => &style_node.style,
        //     _ => panic!("cannot extract styles from called on non-block"),
        // };

        let d = &mut self.dimensions;
        d.content.x = containing_block.content.x + d.margin.left + d.border.left + d.padding.left;

        // Position the box below all the previous boxes in the container.
        d.content.y = containing_block.content.height as i32
            + containing_block.content.y
            + d.margin.top
            + d.border.top
            + d.padding.top;
    }

    fn layout_block_children(&mut self) {
        let d = &mut self.dimensions;
        for child in &mut self.children {
            child.layout_as_child(d);
            // Track the height so each child is laid out below the previous content.
            d.content.height = d.content.height + child.dimensions.margin_box().height;
        }
    }

    fn calculate_block_height(&mut self, containing_block: &Dimensions) {
        // If the height is set to an explicit length, use that exact length.
        // Otherwise, just keep the value set by `layout_block_children`.
        let style = match self.box_type {
            BoxType::BlockNode(style_node) => &style_node.style,
            _ => panic!("cannot extract styles from called on non-block"),
        };
        let containing_width = containing_block.content.width.try_into().unwrap();

        self.dimensions.content.height = style
            .height
            .map(|h| h.to_pixels(containing_width))
            .unwrap_or(0) as u32;
    }
}
