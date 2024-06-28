use crate::simple_gui::{components::Label, Document};

pub fn create_view(width: u32, height: u32) -> Document {
    let mut document = Document::new(width, height);

    document
        .components_mut()
        .push(Box::new(Label::new("Hello world!")));

    document
}
