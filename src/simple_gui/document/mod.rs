use super::{components::Component, layout::view::StandaloneView};

pub struct Document {
    view: StandaloneView,
}

impl Document {
    pub fn new(width: u32, height: u32) -> Self {
        Document {
            view: StandaloneView::new(width, height),
        }
    }

    pub fn update(&mut self) {
        self.view.update();
    }

    pub fn layout(&mut self) {
        self.view.layout();
    }

    pub fn draw(&self) {
        self.view.draw();
    }

    #[allow(dead_code)]
    pub fn components(&self) -> &Vec<Box<dyn Component>> {
        &self.view.components
    }

    pub fn components_mut(&mut self) -> &mut Vec<Box<dyn Component>> {
        &mut self.view.components
    }
}
