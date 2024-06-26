use crate::gui::document::node::Node;

use super::styles::Styles;

// We don't techincally need this, but it's going to be useful
// if we decide to add support for standalone stylesheets
pub struct StyledNode<'a> {
    pub(crate) node: &'a Node,
    pub(crate) style: Styles,
    pub(crate) children: Vec<StyledNode<'a>>,
}
