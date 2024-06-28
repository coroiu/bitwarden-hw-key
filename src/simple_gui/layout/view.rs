use crate::simple_gui::{components::Component, primitives::Size};

pub trait View {}

pub struct StandaloneView {
    size: Size,
    pub(crate) components: Vec<Box<dyn Component>>,
}

impl StandaloneView {
    pub fn new(width: u32, height: u32) -> Self {
        StandaloneView {
            size: Size::new(width, height),
            components: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn components(&self) -> &Vec<Box<dyn Component>> {
        &self.components
    }

    pub(crate) fn components_mut(&mut self) -> &mut Vec<Box<dyn Component>> {
        &mut self.components
    }

    pub(crate) fn update(&mut self) {
        self.components_mut().iter_mut().for_each(|c| c.update());
    }

    pub(crate) fn layout(&mut self) {
        self.components_mut().iter_mut().for_each(|c| c.layout());
    }

    pub(crate) fn draw(&self) {
        self.components().iter().for_each(|c| c.draw());
    }
}

impl View for StandaloneView {}

pub struct SubView<C>
where
    C: Component,
{
    size: Size,
    components: Vec<Box<C>>,
}

impl<C> View for SubView<C> where C: Component {}
