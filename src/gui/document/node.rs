use crate::gui::layout::styles::Styles;

pub struct Node {
    pub children: Vec<Node>,
    pub node_data: GenericNodeData,
    pub node_type: NodeType,
}

// Attributes might not be needed, might also be mergeable with GenericNodeData
// type Attributes = HashMap<String, String>;
#[derive(Default)]
pub struct Attributes {
    pub id: Option<String>,
    pub style: Option<Styles>,
}

pub struct GenericNodeData {
    pub attributes: Attributes,
}

pub enum NodeType {
    Text(TextNodeData),
    Box(),
}

pub struct TextNodeData {
    pub text: String,
}

impl Node {
    pub fn new(node_type: NodeType, attributes: Attributes) -> Self {
        Node {
            children: Vec::new(),
            node_data: GenericNodeData { attributes },
            node_type,
        }
    }
}
