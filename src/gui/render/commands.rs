use crate::gui::{
    document::node::NodeType,
    layout::layout_box::{BoxType, LayoutBox},
    primitives::{Color, Rectangle},
    style::font::Font,
};

pub enum RenderCommand {
    SolidColor(Color, Rectangle),
    Text(Color, Rectangle, String, &'static Font),
}

pub fn build_render_commands(layout_root: &LayoutBox) -> Vec<RenderCommand> {
    let mut list = Vec::new();
    render_layout_box(&mut list, layout_root);
    return list;
}

fn render_layout_box(list: &mut Vec<RenderCommand>, layout_box: &LayoutBox) {
    render_generic_traits(list, layout_box);
    // TODO: render text

    for child in layout_box.children() {
        render_layout_box(list, child);
    }
}

fn render_generic_traits(list: &mut Vec<RenderCommand>, layout_box: &LayoutBox) {
    match layout_box.box_type {
        BoxType::AnonymousBlock => {} // Anonymous boxes are not rendered
        BoxType::InlineNode(styled_node) | BoxType::FlexNode(styled_node) => {
            // # Content box
            list.push(RenderCommand::SolidColor(
                styled_node.style.background_color.unwrap_or_default(),
                Rectangle {
                    x: layout_box.dimensions.content.x,
                    y: layout_box.dimensions.content.y,
                    width: layout_box.dimensions.content.width,
                    height: layout_box.dimensions.content.height,
                },
            ));

            // # Border
            let border_box = layout_box.dimensions.border_box();

            // ## Top border
            list.push(RenderCommand::SolidColor(
                styled_node.style.border_color.unwrap_or_default(),
                Rectangle {
                    x: border_box.x,
                    y: border_box.y,
                    width: border_box.width,
                    height: layout_box.dimensions.border.top as u32,
                },
            ));

            // ## Right border
            list.push(RenderCommand::SolidColor(
                styled_node.style.border_color.unwrap_or_default(),
                Rectangle {
                    x: border_box.x + border_box.width as i32
                        - layout_box.dimensions.border.right as i32,
                    y: border_box.y,
                    width: layout_box.dimensions.border.right as u32,
                    height: border_box.height,
                },
            ));

            // ## Bottom border
            list.push(RenderCommand::SolidColor(
                styled_node.style.border_color.unwrap_or_default(),
                Rectangle {
                    x: border_box.x,
                    y: border_box.y + border_box.height as i32
                        - layout_box.dimensions.border.bottom as i32,
                    width: border_box.width,
                    height: layout_box.dimensions.border.bottom as u32,
                },
            ));

            // ## Left border
            list.push(RenderCommand::SolidColor(
                styled_node.style.border_color.unwrap_or_default(),
                Rectangle {
                    x: border_box.x,
                    y: border_box.y,
                    width: layout_box.dimensions.border.left as u32,
                    height: border_box.height,
                },
            ));
        }
    }
    render_specific_traits(list, layout_box);
}

fn render_specific_traits(list: &mut Vec<RenderCommand>, layout_box: &LayoutBox) {
    match layout_box.box_type {
        BoxType::AnonymousBlock => {} // Anonymous boxes are not rendered
        BoxType::InlineNode(styled_node) | BoxType::FlexNode(styled_node) => {
            match &styled_node.node.node_type {
                NodeType::Box() => {} // Box doesn't have any specific rendering traits
                NodeType::Text(text_node_data) => {
                    list.push(RenderCommand::Text(
                        styled_node.style.color.unwrap_or_default(),
                        Rectangle {
                            x: layout_box.dimensions.content.x,
                            y: layout_box.dimensions.content.y,
                            width: layout_box.dimensions.content.width,
                            height: layout_box.dimensions.content.height,
                        },
                        text_node_data.text.clone(),
                        text_node_data.font,
                    ));
                }
            }
        }
    }
}
