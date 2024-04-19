use crate::gui::document::node::Node;

use super::styled_node::StyledNode;

pub fn build_style_tree<'a>(root: &'a Node /*, stylesheet: &Stylesheet */) -> StyledNode<'a> {
    let style = root.node_data.attributes.style.unwrap_or_default();

    StyledNode {
        node: root,
        style,
        children: root
            .children
            .iter()
            .map(|child| build_style_tree(child))
            .collect(),
    }
}
