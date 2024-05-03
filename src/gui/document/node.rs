use crate::gui::style::{font::Font, styles::ElementStyles};

pub struct Node {
    pub children: Vec<Node>,
    pub node_data: GenericNodeData,
    pub node_type: NodeType,
    pub states: Vec<ElementState>,
    // Sort of like an angular component, needs some method of refering to the node
    // pub custom_component: Option<Box<dyn CustomComponent>>,
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub enum ElementState {
    Focus,
}

// pub trait CustomComponent {
//     // reference needs to be able to stored, maybe use ARC or something
//     on_init(&self, node: &mut Node);
// }

// Attributes might not be needed, might also be mergeable with GenericNodeData
// type Attributes = HashMap<String, String>;
#[derive(Default)]
pub struct Attributes {
    pub id: Option<String>,
    pub style: Option<ElementStyles>,
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
    pub font: &'static Font,
}

impl Node {
    pub fn new(node_type: NodeType, attributes: Attributes) -> Self {
        Node {
            children: Vec::new(),
            states: Vec::new(),
            node_data: GenericNodeData { attributes },
            node_type,
        }
    }
}
