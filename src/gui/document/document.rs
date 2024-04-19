use crate::gui::{
    layout::layout_tree::build_layout_tree,
    primitives::Rectangle,
    render::{paint, Canvas},
    style::{
        style_tree::build_style_tree,
        styles::{Display, Size, Styles},
    },
};

use super::node::{Attributes, Node, NodeType};

pub struct Document {
    pub(super) root: Node,
    width: u32,
    height: u32,
}

impl Document {
    pub fn new(width: u32, height: u32) -> Self {
        Document {
            root: Node::new(
                NodeType::Box(),
                Attributes {
                    style: Some(Styles {
                        display: Display::Flex,
                        width: Some(Size::Pixels(width as i32)),
                        height: Some(Size::Pixels(height as i32)),
                        background_color: Some(crate::gui::primitives::Color {
                            r: 10,
                            g: 10,
                            b: 10,
                            a: 10,
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ),
            width,
            height,
        }
    }

    pub fn children(&self) -> &Vec<Node> {
        &self.root.children
    }

    pub fn children_mut(&mut self) -> &mut Vec<Node> {
        &mut self.root.children
    }

    pub fn draw(&self) -> Canvas {
        let bounds = Rectangle::new(0, 0, self.width, self.height);

        let style_root = build_style_tree(&self.root);
        let mut layout_root = build_layout_tree(&style_root);
        layout_root.layout(bounds);
        paint(&layout_root, bounds)
    }
}
