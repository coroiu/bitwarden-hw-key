use minifb::{Key, Window};
use std::collections::HashMap;

use crate::gui::input::{InputEvent, InputInterface, KeyCode, KeyEvent};

pub struct DesktopInput {
    key_states: HashMap<KeyCode, bool>,
    pending_events: Vec<InputEvent>,
}

impl DesktopInput {
    pub fn new() -> Self {
        DesktopInput {
            key_states: HashMap::new(),
            pending_events: Vec::new(),
        }
    }

    /// Process window keyboard state and store events internally
    pub fn process_window(&mut self, window: &Window) {
        self.pending_events.clear();

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
                self.pending_events.push(InputEvent {
                    key_code: *key_code,
                    key_event: KeyEvent::Clicked,
                });
            }

            // Update state
            self.key_states.insert(*key_code, is_pressed);
        }
    }
}

impl InputInterface for DesktopInput {
    fn get_events(&mut self) -> Vec<InputEvent> {
        // Return events collected by process_window
        self.pending_events.clone()
    }

    fn update(&mut self) {
        // No-op for desktop, updating happens in process_window
    }
}
