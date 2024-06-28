mod label;

pub use label::*;

pub trait Component {
    fn update(&mut self) {}

    fn layout(&mut self) {}

    fn draw(&self) {}
}
