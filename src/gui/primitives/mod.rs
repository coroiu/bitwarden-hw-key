#[derive(Default, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Default, Clone, Copy)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Default, Clone, Copy)]
pub struct Edges {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Rectangle {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Rectangle {
        Rectangle {
            x,
            y,
            width,
            height,
        }
    }

    pub fn expand(&self, edges: &Edges) -> Rectangle {
        Rectangle {
            x: self.x - edges.left,
            y: self.y - edges.top,
            width: self.width + (edges.left + edges.right) as u32,
            height: self.height + (edges.top + edges.bottom) as u32,
        }
    }
}
