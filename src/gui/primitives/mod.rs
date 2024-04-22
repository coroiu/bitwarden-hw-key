#[derive(Debug, Default, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255 }
    }

    pub fn black() -> Color {
        Color::rgb(0, 0, 0)
    }

    pub fn white() -> Color {
        Color::rgb(255, 255, 255)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Edges {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
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

#[derive(Debug, Default, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    pub fn zero() -> Point {
        Point { x: 0, y: 0 }
    }
}
