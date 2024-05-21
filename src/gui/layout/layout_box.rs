use crate::gui::{
    primitives::{Edges, Point, Rectangle},
    style::{
        styled_node::{self, StyledNode},
        styles::{FlexDirection, Size, Styles},
    },
};

pub(crate) struct LayoutBox<'a> {
    pub(crate) dimensions: Dimensions,
    pub(crate) box_type: BoxType<'a>,
    children: Vec<LayoutBox<'a>>,
}

#[derive(Clone, Copy)]
pub(crate) enum BoxType<'a> {
    FlexNode(&'a StyledNode<'a>),
    InlineNode(&'a StyledNode<'a>),
    #[allow(dead_code)]
    AnonymousBlock,
}

#[derive(Debug, Default)]
pub(crate) struct Dimensions {
    pub(crate) content: Rectangle,
    pub(crate) padding: Edges,
    pub(crate) border: Edges,
    pub(crate) margin: Edges,
}

impl Dimensions {
    // The area covered by the content area plus its padding.
    pub(crate) fn padding_box(&self) -> Rectangle {
        self.content.expand(&self.padding)
    }

    // The area covered by the content area plus padding and borders.
    pub(crate) fn border_box(&self) -> Rectangle {
        self.padding_box().expand(&self.border)
    }

    // The area covered by the content area plus padding, borders, and margin.
    pub(crate) fn margin_box(&self) -> Rectangle {
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
                BoxType::InlineNode(_) | BoxType::FlexNode(_) | BoxType::AnonymousBlock => {
                    self.children.push(child)
                } // From when Block nodes were supported. Keeping this here for reference
                  // BoxType::BlockNode(_) => {
                  //     // Where are a block node, so we need to create an anonymous block box to hold this inline box.
                  //     // Make sure the last child is an anonymous block box where we can put this inline box.
                  //     match self.children.last() {
                  //         Some(&LayoutBox {
                  //             box_type: BoxType::AnonymousBlock,
                  //             ..
                  //         }) => {}
                  //         _ => self.children.push(LayoutBox::new(BoxType::AnonymousBlock)),
                  //     };

                  //     self.children.last_mut().unwrap().add_child(child);
                  // }
            },
            BoxType::FlexNode(_) => {
                self.children.push(child);
            }
            _ => {}
        }
    }

    pub(crate) fn layout(&mut self, bounds: Rectangle) {
        let containing_dimensions = Dimensions {
            content: bounds,
            ..Default::default()
        };

        self.layout_as_child(
            &containing_dimensions,
            &self.box_type.clone(),
            bounds.top_left(),
        );
    }

    fn layout_as_child(
        &mut self,
        containing_dimensions: &Dimensions,
        containing_type: &BoxType,
        offset: Point,
    ) {
        match containing_type {
            BoxType::FlexNode(containing_styles) => match &self.box_type {
                BoxType::FlexNode(styled_node) => self.layout_flex(
                    styled_node,
                    containing_dimensions,
                    &containing_styles.style,
                    offset,
                ),
                BoxType::InlineNode(_) => {}
                BoxType::AnonymousBlock => {}
            },
            BoxType::InlineNode(_) => {}
            BoxType::AnonymousBlock => {}
        }
    }

    fn layout_flex(
        &mut self,
        styled_node: &StyledNode,
        containing_dimensions: &Dimensions,
        containing_styles: &Styles,
        offset: Point,
    ) {
        self.calculate_flex_width(
            styled_node,
            containing_dimensions,
            containing_styles,
            offset,
        );
        self.calculate_flex_height(
            styled_node,
            containing_dimensions,
            containing_styles,
            offset,
        );
        self.layout_flex_children(
            styled_node,
            containing_dimensions,
            containing_styles,
            offset,
        );

        println!("{:?}", self.dimensions);
    }

    fn calculate_flex_width(
        &mut self,
        styled_node: &StyledNode,
        containing_dimensions: &Dimensions,
        containing_styles: &Styles,
        offset: Point,
    ) {
        match containing_styles.flex_direction.unwrap_or_default() {
            FlexDirection::Row => {
                let style = &styled_node.style;

                // // We don't expect the content width to be so big that we overflow an i32
                let containing_width = containing_dimensions.content.width.try_into().unwrap();
                let d = &mut self.dimensions;

                // margins, borders, and paddings default to 0
                let margin_left = style.margin.and_then(|m| m.left).unwrap_or(Size::zero());
                let margin_right = style.margin.and_then(|m| m.right).unwrap_or(Size::zero());

                let border_left = style.border.and_then(|b| b.left).unwrap_or(Size::zero());
                let border_right = style.border.and_then(|b| b.right).unwrap_or(Size::zero());

                let padding_left = style.padding.and_then(|p| p.left).unwrap_or(Size::zero());
                let padding_right = style.padding.and_then(|p| p.right).unwrap_or(Size::zero());

                d.padding.left = padding_left.to_pixels(containing_width);
                d.padding.right = padding_right.to_pixels(containing_width);

                d.border.left = border_left.to_pixels(containing_width);
                d.border.right = border_right.to_pixels(containing_width);

                d.margin.left = margin_left.to_pixels(containing_width);
                d.margin.right = margin_right.to_pixels(containing_width);

                d.content.x = offset.x + d.margin.left + d.border.left + d.padding.left;

                let width = style.width.unwrap_or_default();
                match width {
                    Size::Pixels(_) | Size::Percentage(_) => {
                        // width is set to a fixed value
                        let margin_box_width = width.to_pixels(containing_width) as u32;
                        d.content.width = margin_box_width
                            - d.margin.left as u32
                            - d.border.left as u32
                            - d.padding.left as u32
                            - d.margin.right as u32
                            - d.border.right as u32
                            - d.padding.right as u32;
                    }
                    _ => {} // width is auto, it will be calculated below (not currently supported)
                }
            }
            FlexDirection::Column => {
                let style = &styled_node.style;

                // We don't expect the content width to be so big that we overflow an i32
                let containing_width = containing_dimensions.content.width.try_into().unwrap();

                // margins, borders, and paddings default to 0
                let margin_left = style.margin.and_then(|m| m.left).unwrap_or(Size::zero());
                let margin_right = style.margin.and_then(|m| m.right).unwrap_or(Size::zero());

                let border_left = style.border.and_then(|b| b.left).unwrap_or(Size::zero());
                let border_right = style.border.and_then(|b| b.right).unwrap_or(Size::zero());

                let padding_left = style.padding.and_then(|p| p.left).unwrap_or(Size::zero());
                let padding_right = style.padding.and_then(|p| p.right).unwrap_or(Size::zero());

                let d = &mut self.dimensions;

                d.padding.left = padding_left.to_pixels(containing_width);
                d.padding.right = padding_right.to_pixels(containing_width);

                d.border.left = border_left.to_pixels(containing_width);
                d.border.right = border_right.to_pixels(containing_width);

                d.margin.left = margin_left.to_pixels(containing_width);
                d.margin.right = margin_right.to_pixels(containing_width);

                d.content.x = offset.x + d.margin.left + d.border.left + d.padding.left;

                let width = style.width.unwrap_or_default();
                match width {
                    Size::Pixels(_) | Size::Percentage(_) => {
                        // width is set to a fixed value
                        let margin_box_width = width.to_pixels(containing_width) as u32;
                        d.content.width = margin_box_width
                            - d.margin.left as u32
                            - d.border.left as u32
                            - d.padding.left as u32
                            - d.margin.right as u32
                            - d.border.right as u32
                            - d.padding.right as u32;
                    }
                    Size::Auto => {
                        // Fill up the width (could probably be handled as width = 100%)
                        d.content.width = containing_width as u32
                            - d.margin.left as u32
                            - d.border.left as u32
                            - d.padding.left as u32
                            - d.margin.right as u32
                            - d.border.right as u32
                            - d.padding.right as u32;
                    }
                }
            }
        }
    }

    fn calculate_flex_height(
        &mut self,
        styled_node: &StyledNode,
        containing_dimensions: &Dimensions,
        containing_styles: &Styles,
        offset: Point,
    ) {
        match containing_styles.flex_direction.unwrap_or_default() {
            FlexDirection::Row => {
                let style = &styled_node.style;

                // We don't expect the content width to be so big that we overflow an i32
                let containing_height = containing_dimensions.content.height.try_into().unwrap();

                // margins, borders, and paddings default to 0
                let margin_top = style.margin.and_then(|m| m.top).unwrap_or(Size::zero());
                let margin_bottom = style.margin.and_then(|m| m.bottom).unwrap_or(Size::zero());

                let border_top = style.border.and_then(|b| b.top).unwrap_or(Size::zero());
                let border_bottom = style.border.and_then(|b| b.bottom).unwrap_or(Size::zero());

                let padding_top = style.padding.and_then(|p| p.top).unwrap_or(Size::zero());
                let padding_bottom = style.padding.and_then(|p| p.bottom).unwrap_or(Size::zero());

                let d = &mut self.dimensions;

                d.padding.top = padding_top.to_pixels(containing_height);
                d.padding.bottom = padding_bottom.to_pixels(containing_height);

                d.border.top = border_top.to_pixels(containing_height);
                d.border.bottom = border_bottom.to_pixels(containing_height);

                d.margin.top = margin_top.to_pixels(containing_height);
                d.margin.bottom = margin_bottom.to_pixels(containing_height);

                d.content.y = offset.y + d.margin.top + d.border.top + d.padding.top;

                let height = style.height.unwrap_or_default();
                match height {
                    Size::Pixels(_) | Size::Percentage(_) => {
                        // height is set to a fixed value
                        let margin_box_height = height.to_pixels(containing_height) as u32;
                        d.content.height = margin_box_height
                            - d.margin.top as u32
                            - d.border.top as u32
                            - d.padding.top as u32
                            - d.margin.bottom as u32
                            - d.border.bottom as u32
                            - d.padding.bottom as u32;
                    }
                    Size::Auto => {
                        // Fill up the height (could probably be handled as height = 100%)
                        d.content.height = containing_height as u32
                            - d.margin.top as u32
                            - d.border.top as u32
                            - d.padding.top as u32
                            - d.margin.bottom as u32
                            - d.border.bottom as u32
                            - d.padding.bottom as u32;
                    }
                }
            }
            FlexDirection::Column => {
                let style = &styled_node.style;

                // // We don't expect the content height to be so big that we overflow an i32
                let containing_height = containing_dimensions.content.height.try_into().unwrap();
                let d = &mut self.dimensions;

                // margins, borders, and paddings default to 0
                let margin_top = style.margin.and_then(|m| m.top).unwrap_or(Size::zero());
                let margin_bottom = style.margin.and_then(|m| m.bottom).unwrap_or(Size::zero());

                let border_top = style.border.and_then(|b| b.top).unwrap_or(Size::zero());
                let border_bottom = style.border.and_then(|b| b.bottom).unwrap_or(Size::zero());

                let padding_top = style.padding.and_then(|p| p.top).unwrap_or(Size::zero());
                let padding_bottom = style.padding.and_then(|p| p.bottom).unwrap_or(Size::zero());

                d.padding.top = padding_top.to_pixels(containing_height);
                d.padding.bottom = padding_bottom.to_pixels(containing_height);

                d.border.top = border_top.to_pixels(containing_height);
                d.border.bottom = border_bottom.to_pixels(containing_height);

                d.margin.top = margin_top.to_pixels(containing_height);
                d.margin.bottom = margin_bottom.to_pixels(containing_height);

                d.content.y = offset.y + d.margin.top + d.border.top + d.padding.top;

                let height = style.height.unwrap_or_default();
                match height {
                    Size::Pixels(_) | Size::Percentage(_) => {
                        // height is set to a fixed value
                        let margin_box_height = height.to_pixels(containing_height) as u32;
                        d.content.height = margin_box_height
                            - d.margin.top as u32
                            - d.border.top as u32
                            - d.padding.top as u32
                            - d.margin.bottom as u32
                            - d.border.bottom as u32
                            - d.padding.bottom as u32;
                    }
                    _ => {} // height is auto, it will be calculated below (not currently supported)
                }
            }
        }
    }

    fn layout_flex_children(
        &mut self,
        styled_node: &StyledNode,
        _containing_dimensions: &Dimensions,
        _containing_styles: &Styles,
        offset: Point,
    ) {
        let d = &mut self.dimensions;
        let style = &styled_node.style;

        match style.flex_direction.unwrap_or_default() {
            FlexDirection::Row => {
                let y_position = d.content.y;
                let mut x_position = d.content.x;
                for child in &mut self.children {
                    child.layout_as_child(
                        d,
                        &self.box_type,
                        offset.translate(x_position, y_position),
                    );
                    // Track the x offset so each child is laid out next to the previous node.z
                    x_position = x_position + child.dimensions.margin_box().width as i32;
                }
            }
            FlexDirection::Column => {
                let mut y_position = d.content.y;
                let x_position = d.content.x;

                for child in &mut self.children {
                    child.layout_as_child(
                        d,
                        &self.box_type,
                        offset.translate(x_position, y_position),
                    );
                    // Track the y offset so each child is laid out next to the previous node.
                    y_position = y_position + child.dimensions.margin_box().height as i32;
                }
            }
        }
    }
}
