use crate::gui::style::{styled_node::StyledNode, styles::Display};

use super::layout_box::{BoxType, LayoutBox};

pub fn build_layout_tree<'a>(style_root: &'a StyledNode<'a>) -> LayoutBox<'a> {
    let mut root = LayoutBox::new(match style_root.style.display {
        Display::Block => BoxType::BlockNode(style_root),
        Display::Inline => BoxType::InlineNode(style_root),
        Display::None => panic!("Root node has display: none."),
    });

    // TODO: Maybe we should just send this straight into the constructor above, instead of
    // manually pushing evey item into the vector. See `fn style_tree()`.
    for child in &style_root.children {
        let child_layout_box = build_layout_tree(child);
        root.add_child(child_layout_box);
    }

    root
}
