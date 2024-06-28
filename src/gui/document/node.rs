use std::{cell::Cell, collections::HashSet};

use uuid::Uuid;

use crate::gui::{
    primitives::{Point, Rectangle},
    style::{font::Font, styles::ElementStyles},
};

pub struct Node {
    pub id: Uuid,
    pub children: Vec<Node>,
    pub node_data: GenericNodeData,
    pub node_type: NodeType,
    pub states: HashSet<ElementState>,
    // Sort of like an angular component, needs some method of refering to the node
    // pub custom_component: Option<Box<dyn CustomComponent>>,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
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
    /// Always negative
    pub scroll: Cell<Point>,
    pub dimensions: Cell<Rectangle>,
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
            id: Uuid::new_v4(),
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

    pub fn children_mut(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }

    pub fn scroll_into_view(&self, bounds: Rectangle) {
        let self_bounds = self.node_data.properties.dimensions.get();
        let intersection = self_bounds.intersect(bounds);
        if intersection.bounds == bounds {
            // Already in view
            return;
        }

        let scroll = self.node_data.properties.scroll.get();
        // let scroll_to = Point {
        //     x: if intersection.bounds.x == bounds.x {
        //         scroll.x
        //     } else if intersection.bounds.x + intersection.bounds.width as i32
        //         == bounds.x + bounds.width as i32
        //     {
        //         scroll.x + bounds.width as i32 - intersection.bounds.width as i32
        //     } else {
        //         scroll.x
        //     },
        //     y: if intersection.bounds.y == bounds.y {
        //         scroll.y
        //     } else if intersection.bounds.y + intersection.bounds.height as i32
        //         == bounds.y + bounds.height as i32
        //     {
        //         scroll.y + bounds.height as i32 - intersection.bounds.height as i32
        //     } else {
        //         scroll.y
        //     },
        // };
        // Works but need to fix properly
        let scroll_to = scroll.translate(0, 5);
        println!("Scrolling to {:?}", scroll_to);
        self.node_data.properties.scroll.set(scroll_to);
    }

    // pub fn is_within_scroll(&self, check: Rectangle) -> bool {
    //     let bounds = self.node_data.properties.dimensions.get();
    //     let scroll = self.node_data.properties.scroll.get();
    //     let check_scrolled = check.offset(scroll);
    //     let intersect = bounds.intersect(check);

    //     intersect.is_zero()
    // }
}
