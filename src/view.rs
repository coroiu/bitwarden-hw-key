use std::collections::HashMap;

use embedded_graphics::{pixelcolor::BinaryColor, Drawable};

use crate::gui::{
    document::{
        node::{Attributes, ElementState, Node, NodeType, TextNodeData},
        Document,
    },
    input::InputInterface,
    primitives::Color,
    render::Canvas,
    style::{
        font::FONT_5X8,
        styles::{
            Display, EdgeSizes, ElementStyles, FlexDirection, Size, SizeFluentPercentage,
            SizeFluentPixels, Styles,
        },
    },
};

pub fn create_view(width: u32, height: u32, input: Box<dyn InputInterface>) -> Document {
    let mut document = Document::new(width, height, input);

    let mut container = Node::new(
        NodeType::Box(),
        Attributes {
            style: ElementStyles {
                base_styles: Styles {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row.into(),
                    border_color: Color::white().into(),
                    width: 100.percent().into(),
                    height: 100.percent().into(),
                    padding: EdgeSizes::all(1.px()).into(),
                    margin: EdgeSizes::all(1.px()).into(),
                    border: EdgeSizes::all(1.px()).into(),
                    ..Default::default()
                },
                state_styles: Default::default(),
            }
            .into(),
            ..Default::default()
        },
    );

    let mut container_a = Node::new(
        NodeType::Box(),
        Attributes {
            style: ElementStyles {
                base_styles: Styles {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column.into(),
                    border_color: Color::white().into(),
                    width: 50.percent().into(),
                    height: 100.percent().into(),
                    margin: EdgeSizes::all(1.px()).into(),
                    border: EdgeSizes::all(1.px()).into(),
                    ..Default::default()
                },
                state_styles: Default::default(),
            }
            .into(),
            ..Default::default()
        },
    );

    container_a.children_mut().push(Node::new(
        NodeType::Box(),
        Attributes {
            style: ElementStyles {
                base_styles: Styles {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row.into(),
                    border_color: Color::white().into(),
                    width: Size::Auto.into(),
                    height: 33.percent().into(),
                    padding: EdgeSizes::all(1.px()).into(),
                    margin: EdgeSizes::all(1.px()).into(),
                    border: EdgeSizes::all(1.px()).into(),
                    ..Default::default()
                },
                state_styles: Default::default(),
            }
            .into(),
            ..Default::default()
        },
    ));

    container_a.children_mut().push(Node::new(
        NodeType::Box(),
        Attributes {
            style: ElementStyles {
                base_styles: Styles {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row.into(),
                    border_color: Color::white().into(),
                    width: 100.percent().into(),
                    height: 33.percent().into(),
                    padding: EdgeSizes::all(1.px()).into(),
                    margin: EdgeSizes::all(1.px()).into(),
                    border: EdgeSizes::all(1.px()).into(),
                    ..Default::default()
                },
                state_styles: Default::default(),
            }
            .into(),
            ..Default::default()
        },
    ));

    container_a.children_mut().push(Node::new(
        NodeType::Box(),
        Attributes {
            style: ElementStyles {
                base_styles: Styles {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row.into(),
                    border_color: Color::white().into(),
                    width: 50.percent().into(),
                    height: 33.percent().into(),
                    padding: EdgeSizes::all(1.px()).into(),
                    margin: EdgeSizes::all(1.px()).into(),
                    border: EdgeSizes::all(1.px()).into(),
                    ..Default::default()
                },
                state_styles: Default::default(),
            }
            .into(),
            ..Default::default()
        },
    ));

    container.children_mut().push(container_a);

    container.children_mut().push(Node::new(
        NodeType::Box(),
        Attributes {
            style: ElementStyles {
                base_styles: Styles {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row.into(),
                    border_color: Color::white().into(),
                    width: 33.percent().into(),
                    height: 100.percent().into(),
                    padding: EdgeSizes::all(1.px()).into(),
                    margin: EdgeSizes::all(1.px()).into(),
                    border: EdgeSizes::all(1.px()).into(),
                    ..Default::default()
                },
                state_styles: Default::default(),
            }
            .into(),
            ..Default::default()
        },
    ));

    container.children_mut().push(Node::new(
        NodeType::Text(TextNodeData {
            text: "Hello, world".to_string(),
            font: &FONT_5X8,
        }),
        Attributes {
            style: ElementStyles {
                base_styles: Styles {
                    display: Display::Flex,
                    width: 33.percent().into(),
                    ..Default::default()
                },
                state_styles: Default::default(),
            }
            .into(),
            ..Default::default()
        },
    ));

    document.children_mut().push(container);

    document
}

impl Drawable for Canvas {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        let pixels = self
            .image_buffer
            .pixels
            .iter()
            .enumerate()
            .map(|(i, color)| {
                let x = i % self.image_buffer.width;
                let y = i / self.image_buffer.width;

                let combined_colors = color.r as i32 + color.g as i32 + color.b as i32;
                let mapped_color = match combined_colors {
                    c if c > 300 => BinaryColor::On,
                    _ => BinaryColor::Off,
                };

                embedded_graphics::Pixel(
                    embedded_graphics::geometry::Point::new(x as i32, y as i32),
                    mapped_color,
                )
            });

        target.draw_iter(pixels)
    }
}
