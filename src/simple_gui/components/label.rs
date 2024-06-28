use super::Component;

pub struct Label {
    text: String,
}

impl Label {
    pub fn new(text: &str) -> Label {
        Label {
            text: text.to_owned(),
        }
    }
}

impl Component for Label {}
