use crate::gui::primitives::Color;

#[derive(Default, Clone, Copy)]
pub struct Styles {
    pub display: Display,
    pub color: Option<Color>,
    pub background_color: Option<Color>,
}

#[derive(Default, Clone, Copy)]
pub enum Display {
    #[default]
    Block,
    Inline,
    None,
}
