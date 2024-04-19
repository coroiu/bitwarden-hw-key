use embedded_graphics::{pixelcolor::BinaryColor, Drawable};

use crate::gui::{
    document::{
        node::{Attributes, Node, NodeType, TextNodeData},
        Document,
    },
    primitives::Color,
    render::Canvas,
    style::styles::{Display, EdgeSizes, Size, Styles},
};

pub fn create_view(width: u32, height: u32) -> Document {
    let mut document = Document::new(width, height);

    document.children_mut().push(Node::new(
        NodeType::Box(),
        Attributes {
            style: Some(Styles {
                display: Display::Flex,
                background_color: Color::white().into(),
                width: Some(Size::Pixels(30)),
                height: Some(Size::Pixels(20)),
                ..Default::default()
            }),
            ..Default::default()
        },
    ));

    // document.children_mut().push(Node::new(
    //     NodeType::Box(),
    //     Attributes {
    //         style: Some(Styles {
    //             display: Display::Flex,
    //             background_color: Color::black().into(),
    //             width: Size::Pixels(30).into(),
    //             height: Size::Pixels(20).into(),
    //             ..Default::default()
    //         }),
    //         ..Default::default()
    //     },
    // ));

    document.children_mut().push(Node::new(
        NodeType::Box(),
        Attributes {
            style: Some(Styles {
                display: Display::Flex,
                background_color: Color::white().into(),
                border_color: Color::white().into(),
                width: Size::Pixels(30).into(),
                height: Size::Pixels(20).into(),
                border: EdgeSizes::all(Size::Pixels(1)).into(),
                padding: EdgeSizes::all(Size::Pixels(1)).into(),
                margin: EdgeSizes::all(Size::Pixels(1)).into(),
                ..Default::default()
            }),
            ..Default::default()
        },
    ));

    // document.children_mut().push(Node::new(
    //     NodeType::Text(TextNodeData {
    //         text: "Hello, ".to_string(),
    //     }),
    //     Attributes {
    //         style: Some(Styles {
    //             display: Display::Inline,
    //             ..Default::default()
    //         }),
    //         ..Default::default()
    //     },
    // ));

    // document.children_mut().push(Node::new(
    //     NodeType::Text(TextNodeData {
    //         text: "world!".to_string(),
    //     }),
    //     Attributes {
    //         style: Some(Styles {
    //             display: Display::Inline,
    //             ..Default::default()
    //         }),
    //         ..Default::default()
    //     },
    // ));

    document
}

impl Drawable for Canvas {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        let pixels = self.pixels.iter().enumerate().map(|(i, color)| {
            let x = i % self.width;
            let y = i / self.width;

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
