use crate::gui::{
    input::{InputEvent, InputInterface, KeyCode, KeyEvent},
    layout::layout_tree::build_layout_tree,
    primitives::{Color, Rectangle},
    render::{draw, Canvas},
    style::{
        style_tree::build_style_tree,
        styles::{Display, ElementStyles, Size, Styles},
    },
};

use super::{
    node::{Attributes, ElementState, Node, NodeType},
    utils::SequenceGenerator,
};

pub struct Document {
    pub(super) root: Node,
    pub(super) tab_index: u32,
    width: u32,
    height: u32,
    // Boxed so we don't have to add generic variable to every place that references Documents
    input: Box<dyn InputInterface>,
}

impl Document {
    pub fn new(width: u32, height: u32, input: Box<dyn InputInterface>) -> Self {
        Document {
            root: Node::new(
                NodeType::Box(),
                Attributes {
                    style: Some(ElementStyles {
                        base_styles: Styles {
                            display: Display::Flex,
                            width: Some(Size::Pixels(width as i32)),
                            height: Some(Size::Pixels(height as i32)),
                            background_color: Color::black().into(),
                            ..Default::default()
                        },
                        state_styles: Default::default(),
                    }),
                    ..Default::default()
                },
            ),
            width,
            height,
            tab_index: 0,
            input,
        }
    }

    #[allow(dead_code)]
    pub fn children(&self) -> &Vec<Node> {
        &self.root.children
    }

    pub fn children_mut(&mut self) -> &mut Vec<Node> {
        &mut self.root.children
    }

    pub fn update_input(&mut self) {
        self.input.update();
    }

    pub fn update(&mut self) {
        self.handle_input();
        self.assign_tab_index();
        self.assign_states();
    }

    pub fn draw(&self) -> Canvas {
        let bounds = Rectangle::new(0, 0, self.width, self.height);

        let style_root = build_style_tree(self, &self.root);
        let mut layout_root = build_layout_tree(&style_root);
        layout_root.layout(bounds);
        draw(&layout_root, bounds)
    }

    fn handle_input(&mut self) {
        let events = self.input.get_events();
        for event in events {
            match event {
                InputEvent {
                    key_code: KeyCode::Up,
                    key_event: KeyEvent::Clicked,
                } => {
                    self.tab_index = (self.tab_index - 1).max(0);
                }

                InputEvent {
                    key_code: KeyCode::Down,
                    key_event: KeyEvent::Clicked,
                } => {
                    self.tab_index += 1;
                }

                _ => {}
            }

            log::info!("Event: {:?}, tab_index: {:?}", event, self.tab_index);
        }
    }

    fn assign_tab_index(&mut self) {
        let mut sequence = SequenceGenerator::new();
        self.root.traverse_mut(&mut |node| {
            if let Some(tab_index) = node
                .node_data
                .attributes
                .tab_index
                .and_then(|i| i.try_into().ok())
            {
                sequence.reserve(tab_index);
            }
        });

        self.root.traverse_mut(&mut |node| {
            match node.node_data.attributes.tab_index {
                Some(tab_index) if tab_index >= 0 => {
                    node.node_data.properties.tab_index = Some(tab_index as u32)
                }
                Some(_) => {
                    node.node_data.properties.tab_index = None;
                }
                None => {
                    node.node_data.properties.tab_index = Some(sequence.next());
                }
            };
        });
    }

    fn assign_states(&mut self) {
        self.root
            .traverse_mut(&mut |node| match node.node_data.properties.tab_index {
                Some(tab_index) if tab_index == self.tab_index => {
                    node.states.insert(ElementState::Focus);
                }
                _ => {
                    node.states.remove(&ElementState::Focus);
                }
            });
    }
}
