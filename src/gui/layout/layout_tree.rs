use crate::gui::style::{styled_node::StyledNode, styles::Display};

use super::layout_box::{BoxType, LayoutBox};

pub fn build_layout_tree<'a>(style_node: &'a StyledNode<'a>) -> LayoutBox<'a> {
    let mut root = LayoutBox::new(match style_node.style.display {
        Display::Block => BoxType::BlockNode(style_node),
        Display::Inline => BoxType::InlineNode(style_node),
        Display::None => panic!("Root node has display: none."),
    });

    for child in &style_node.children {
        let child_layout_box = build_layout_tree(child);
        root.add_child(child_layout_box);
    }

    root
}
