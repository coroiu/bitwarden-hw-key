use std::collections::HashSet;

use crate::gui::style::{font::Font, styles::ElementStyles};

pub struct Node {
    pub children: Vec<Node>,
    pub node_data: GenericNodeData,
    pub node_type: NodeType,
    pub states: HashSet<ElementState>,
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
    /// None = Automatically assigned by the DOM
    /// Some(-1) = Not focusable
    /// Some(>=0) = Manually assigned
    pub tab_index: Option<i32>,
}

/// These are assigned by the DOM and are not user defined
#[derive(Default)]
pub struct Properties {
    pub tab_index: Option<u32>,
}

pub struct GenericNodeData {
    pub attributes: Attributes,
    pub properties: Properties,
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
            states: HashSet::new(),
            node_data: GenericNodeData {
                attributes,
                properties: Default::default(),
            },
            node_type,
        }
    }

    pub fn traverse_mut(&mut self, f: &mut dyn FnMut(&mut Node)) {
        f(self);
        for child in &mut self.children {
            child.traverse_mut(f);
        }
    }
}
