use crate::gui::{
    document::{
        node::{Attributes, Node, NodeType, TextNodeData},
        Document,
    },
    style::styles::{Display, Styles},
};

pub fn create_view() -> Document {
    let mut document = Document::new();

    document.children_mut().push(Node::new(
        NodeType::Text(TextNodeData {
            text: "Hello, ".to_string(),
        }),
        Attributes {
            style: Some(Styles {
                display: Display::Inline,
                ..Default::default()
            }),
            ..Default::default()
        },
    ));

    document.children_mut().push(Node::new(
        NodeType::Text(TextNodeData {
            text: "world!".to_string(),
        }),
        Attributes {
            style: Some(Styles {
                display: Display::Inline,
                ..Default::default()
            }),
            ..Default::default()
        },
    ));

    document
}
