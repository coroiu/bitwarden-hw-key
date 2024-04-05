use super::node::{Attributes, Node, NodeType};

pub struct Document {
    pub(super) root: Node,
}

impl Document {
    pub fn new() -> Self {
        Document {
            root: Node::new(NodeType::Box(), Attributes::default()),
        }
    }

    pub fn children(&self) -> &Vec<Node> {
        &self.root.children
    }

    pub fn children_mut(&mut self) -> &mut Vec<Node> {
        &mut self.root.children
    }
}
