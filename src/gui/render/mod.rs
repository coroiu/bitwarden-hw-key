use super::{
    document::node::NodeType,
    layout::layout_box::{BoxType, LayoutBox},
    primitives::{Color, Rectangle},
};

enum RenderCommand {
    SolidColor(Color, Rectangle),
}

fn build_render_commands(layout_root: &LayoutBox) -> Vec<RenderCommand> {
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

fn render_generic_traits(list: &mut Vec<RenderCommand>, layout_box: &LayoutBox) {}

fn render_specific_traits(list: &mut Vec<RenderCommand>, layout_box: &LayoutBox) {
    match layout_box.box_type {
        BoxType::AnonymousBlock => {} // Anonymous boxes are not rendered
        BoxType::BlockNode(styled_node) | BoxType::InlineNode(styled_node) => {
            match &styled_node.node.node_type {
                NodeType::Box() => {} // Box doesn't have any specific rendering traits
                NodeType::Text(text_node_data) => {
                    // TODO: render text
                }
            }
        }
    }
}
