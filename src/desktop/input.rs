use minifb::{Key, Window};
use std::collections::HashMap;

use crate::gui::input::{InputEvent, InputInterface, KeyCode, KeyEvent};

pub struct DesktopInput {
    key_states: HashMap<KeyCode, bool>,
}

impl DesktopInput {
    pub fn new() -> Self {
        DesktopInput {
            key_states: HashMap::new(),
        }
    }

    pub fn process_window(&mut self, window: &Window) -> Vec<InputEvent> {
        let mut events = Vec::new();

        // Map keyboard keys to our KeyCode enum
        let key_mappings = [
            (Key::Up, KeyCode::Up),
            (Key::Down, KeyCode::Down),
            (Key::Space, KeyCode::Middle),
        ];

        for (key, key_code) in &key_mappings {
            let is_pressed = window.is_key_down(*key);
            let was_pressed = self.key_states.get(key_code).copied().unwrap_or(false);

            // Detect key press (rising edge)
            if is_pressed && !was_pressed {
                events.push(InputEvent {
                    key_code: *key_code,
                    key_event: KeyEvent::Clicked,
                });
            }

            // Update state
            self.key_states.insert(*key_code, is_pressed);
        }

        events
    }
}

impl InputInterface for DesktopInput {
    fn get_events(&mut self) -> Vec<InputEvent> {
        // Events are collected via process_window
        // This method is called by the main loop but we handle events differently
        Vec::new()
    }

    fn update(&mut self) {
        // No-op for desktop, updating happens in process_window
    }
}
