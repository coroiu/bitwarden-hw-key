#[derive(Default, Clone, Copy)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[derive(Default)]
pub struct Rectangle {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}
