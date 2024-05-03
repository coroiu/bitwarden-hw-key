use crate::gui::document::{node::Node, Document};

use super::styled_node::StyledNode;

pub fn build_style_tree<'a>(
    document: &Document,
    root: &'a Node, /*, stylesheet: &Stylesheet */
) -> StyledNode<'a> {
    let element_styles = root
        .node_data
        .attributes
        .style
        .as_ref()
        .cloned()
        .unwrap_or_default();

    let style = element_styles.applicable_styles(&root.states);

    StyledNode {
        node: root,
        style,
        children: root
            .children
            .iter()
            .map(|child| build_style_tree(document, child))
            .collect(),
    }
}
