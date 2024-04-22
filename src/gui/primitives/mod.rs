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

    pub fn intersect(&self, b: Rectangle) -> Intersection {
        let x0 = self.x.max(b.x);
        let y0 = self.y.max(b.y);
        let x1 = (self.x + self.width as i32).min(b.x + b.width as i32);
        let y1 = (self.y + self.height as i32).min(b.y + b.height as i32);

        let bounds = Rectangle {
            x: x0,
            y: y0,
            width: (x1 - x0).max(0) as u32,
            height: (y1 - y0).max(0) as u32,
        };

        Intersection {
            bounds,
            offset_a: Point {
                x: bounds.x - self.x,
                y: bounds.y - self.y,
            },
            offset_b: Point {
                x: bounds.x - b.x,
                y: bounds.y - b.y,
            },
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Intersection {
    /// The area of the intersection between two rectangles.
    pub bounds: Rectangle,

    /// The offset of the intersection relative to the first rectangle.
    pub offset_a: Point,

    /// The offset of the intersection relative to the second rectangle.
    pub offset_b: Point,
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
