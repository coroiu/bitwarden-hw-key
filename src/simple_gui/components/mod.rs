mod label;

pub use label::*;

use super::{primitives::Rectangle, render::RenderCommand};

// pub trait UninitializedComponent {
//     fn initialize(self, ) -> Box<dyn Component>;
// }

pub trait Component {
    fn update(&mut self) {}

    fn layout(&mut self) {}

    fn draw(&self, bounds: Rectangle, commands: &mut Vec<RenderCommand>) {}

    fn get_bounds(&self) -> Rectangle;
}
