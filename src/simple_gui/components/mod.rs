mod label;
mod vertical_menu;

pub use label::*;
pub use vertical_menu::*;

use super::{primitives::Rectangle, render::RenderCommand};

// pub trait UninitializedComponent {
//     fn initialize(self, ) -> Box<dyn Component>;
// }

pub trait Component {
    fn update(&mut self) {}

    fn layout(&mut self) {}

    fn draw(&self, _: Rectangle, _: &mut Vec<RenderCommand>) {}
}
