use crate::simple_gui::primitives::Size;

pub trait View {}

pub struct StandaloneView {
    size: Size,
}

impl StandaloneView {
    pub fn new(width: u32, height: u32) -> Self {
        StandaloneView {
            size: Size::new(width, height),
        }
    }
}

impl View for StandaloneView {}

pub struct SubView {}

impl View for SubView {}
