use crate::gui::{primitives::Rectangle, style::styled_node::StyledNode};

pub(crate) struct LayoutBox<'a> {
    dimensions: Dimensions,
    pub(crate) box_type: BoxType<'a>,
    children: Vec<LayoutBox<'a>>,
}

#[derive(Clone, Copy)]
pub(crate) enum BoxType<'a> {
    BlockNode(&'a StyledNode<'a>),
    InlineNode(&'a StyledNode<'a>),
    AnonymousBlock,
}

#[derive(Default)]
pub(crate) struct Dimensions {
    content: Rectangle,
    padding: EdgeSizes,
    border: EdgeSizes,
    margin: EdgeSizes,
}

#[derive(Default)]
pub(crate) struct EdgeSizes {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

impl<'a> LayoutBox<'a> {
    // Constructor function
    pub(super) fn new(box_type: BoxType) -> LayoutBox {
        LayoutBox {
            box_type: box_type,
            dimensions: Default::default(),
            children: Vec::new(),
        }
    }

    pub fn children(&self) -> &Vec<LayoutBox> {
        &self.children
    }

    pub(crate) fn add_child(&mut self, child: LayoutBox<'a>) {
        match child.box_type {
            BoxType::InlineNode(_) => match self.box_type {
                BoxType::InlineNode(_) | BoxType::AnonymousBlock => self.children.push(child),
                BoxType::BlockNode(_) => {
                    // Where are a block node, so we need to create an anonymous block box to hold this inline box.
                    // Make sure the last child is an anonymous block box where we can put this inline box.
                    match self.children.last() {
                        Some(&LayoutBox {
                            box_type: BoxType::AnonymousBlock,
                            ..
                        }) => {}
                        _ => self.children.push(LayoutBox::new(BoxType::AnonymousBlock)),
                    };

                    self.children.last_mut().unwrap().add_child(child);
                }
            },
            BoxType::BlockNode(_) => {
                self.children.push(child);
            }
            _ => {}
        }
    }
}
