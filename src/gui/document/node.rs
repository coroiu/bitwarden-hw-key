pub struct Node {
    children: Vec<Node>,
    node_data: GenericNodeData,
    node_type: NodeType,
}

// Attributes might not be needed, might also be mergeable with GenericNodeData
// type Attributes = HashMap<String, String>;
pub struct Attributes {
    id: Option<String>,
}

pub struct GenericNodeData {
    attributes: Attributes,
}

pub enum NodeType {
    Text(TextNodeData),
    Box(),
}

pub struct TextNodeData {
    text: String,
}
