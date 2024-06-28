use super::layout::view::StandaloneView;

pub struct Document {
    view: StandaloneView,
}

impl Document {
    pub fn new(width: u32, height: u32) -> Self {
        Document {
            view: StandaloneView::new(width, height),
        }
    }
}
