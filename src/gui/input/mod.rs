use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum KeyCode {
    Up,
    Middle,
    Down,
}

#[derive(Debug, Clone, Copy)]
pub enum KeyEvent {
    Clicked,
    DoubleClicked,
    TripleClicked,
    LongPress(Duration),
}

#[derive(Debug, Clone, Copy)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub key_code: KeyCode,
    pub key_event: KeyEvent,
}

pub trait InputInterface {
    fn get_events(&mut self) -> Vec<InputEvent>;
    fn update(&mut self);
}
