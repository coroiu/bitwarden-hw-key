use super::{
    components::{Component, FocusEvent},
    layout::view::StandaloneView,
    render::Canvas,
};
use crate::gui::input::{InputEvent, InputInterface, KeyCode, KeyEvent};

pub struct Document {
    view: StandaloneView,
    focused_component_index: Option<usize>,
}

impl Document {
    pub fn new(width: u32, height: u32) -> Self {
        Document {
            view: StandaloneView::new(width, height),
            focused_component_index: None,
        }
    }

    pub fn update(&mut self) {
        self.view.update();
    }

    pub fn layout(&mut self) {
        self.view.layout();
    }

    pub fn draw(&self, canvas: &mut Canvas) {
        let commands = self.view.draw();

        commands.iter().for_each(|c| canvas.execute(c));
    }

    #[allow(dead_code)]
    pub fn components(&self) -> &Vec<Box<dyn Component>> {
        &self.view.components
    }

    pub fn components_mut(&mut self) -> &mut Vec<Box<dyn Component>> {
        &mut self.view.components
    }

    /// Process input events and manage focus navigation
    pub fn handle_input(&mut self, input: &mut dyn InputInterface) {
        let events = input.get_events();

        if events.is_empty() {
            return;
        }

        // Handle focus navigation (Up/Down keys)
        for event in &events {
            match (event.key_code, event.key_event) {
                (KeyCode::Down, KeyEvent::Clicked) => {
                    self.focus_next();
                }
                (KeyCode::Up, KeyEvent::Clicked) => {
                    self.focus_previous();
                }
                (KeyCode::Middle, KeyEvent::Clicked) => {
                    // Send activation event to focused component
                    if let Some(index) = self.focused_component_index {
                        if index < self.view.components.len() {
                            self.view.components[index].on_focus_event(FocusEvent::Activated);
                        }
                    }
                }
                _ => {}
            }
        }

        // Pass remaining events to focused component
        if let Some(index) = self.focused_component_index {
            if index < self.view.components.len() {
                self.view.components[index].on_input(&events);
            }
        }
    }

    /// Move focus to the next focusable component
    fn focus_next(&mut self) {
        let start_index = self.focused_component_index.map(|i| i + 1).unwrap_or(0);

        for i in 0..self.view.components.len() {
            let index = (start_index + i) % self.view.components.len();
            if self.view.components[index].is_focusable() {
                self.set_focus(Some(index));
                return;
            }
        }
    }

    /// Move focus to the previous focusable component
    fn focus_previous(&mut self) {
        let start_index = self.focused_component_index.unwrap_or(0);

        for i in 0..self.view.components.len() {
            let index = (start_index + self.view.components.len() - 1 - i) % self.view.components.len();
            if self.view.components[index].is_focusable() {
                self.set_focus(Some(index));
                return;
            }
        }
    }

    /// Set focus to a specific component index
    fn set_focus(&mut self, new_index: Option<usize>) {
        if self.focused_component_index == new_index {
            return;
        }

        // Notify old component it lost focus
        if let Some(old_index) = self.focused_component_index {
            if old_index < self.view.components.len() {
                self.view.components[old_index].on_focus_event(FocusEvent::Lost);
            }
        }

        // Notify new component it gained focus
        if let Some(new_index) = new_index {
            if new_index < self.view.components.len() {
                self.view.components[new_index].on_focus_event(FocusEvent::Gained);
            }
        }

        self.focused_component_index = new_index;
    }

    /// Initialize focus on the first focusable component
    pub fn initialize_focus(&mut self) {
        if self.focused_component_index.is_none() {
            for (index, component) in self.view.components.iter().enumerate() {
                if component.is_focusable() {
                    self.set_focus(Some(index));
                    return;
                }
            }
        }
    }
}
