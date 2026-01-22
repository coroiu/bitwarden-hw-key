use super::{
    components::{Component, ComponentAction, FocusEvent},
    layout::view::StandaloneView,
    render::Canvas,
};
use crate::gui::input::{InputEvent, InputInterface, KeyCode, KeyEvent};

struct ViewStackEntry {
    view: StandaloneView,
    focused_index: Option<usize>,
}

pub struct Document {
    view_stack: Vec<ViewStackEntry>,
}

impl Document {
    pub fn new(width: u32, height: u32) -> Self {
        Document {
            view_stack: vec![ViewStackEntry {
                view: StandaloneView::new(width, height),
                focused_index: None,
            }],
        }
    }

    /// Get a reference to the current view entry
    fn current_entry(&self) -> &ViewStackEntry {
        self.view_stack.last().expect("view stack should never be empty")
    }

    /// Get a mutable reference to the current view entry
    fn current_entry_mut(&mut self) -> &mut ViewStackEntry {
        self.view_stack.last_mut().expect("view stack should never be empty")
    }

    /// Push a new view onto the stack
    pub fn push_view(&mut self, builder: Box<dyn Fn() -> StandaloneView>) {
        // Save current focus
        let current_focus = self.current_entry().focused_index;
        self.current_entry_mut().focused_index = current_focus;

        // Build and push new view
        let new_view = builder();
        self.view_stack.push(ViewStackEntry {
            view: new_view,
            focused_index: None,
        });
    }

    /// Pop the current view from the stack and restore previous focus
    pub fn pop_view(&mut self) {
        if self.view_stack.len() > 1 {
            self.view_stack.pop();

            // Restore focus from the now-current entry
            let restored_focus = self.current_entry().focused_index;
            self.set_focus(restored_focus);
        }
    }

    pub fn update(&mut self) {
        self.current_entry_mut().view.update();
    }

    pub fn layout(&mut self) {
        self.current_entry_mut().view.layout();
    }

    pub fn draw(&self, canvas: &mut Canvas) {
        let commands = self.current_entry().view.draw();

        commands.iter().for_each(|c| canvas.execute(c));
    }

    #[allow(dead_code)]
    pub fn components(&self) -> &Vec<Box<dyn Component>> {
        &self.current_entry().view.components
    }

    pub fn components_mut(&mut self) -> &mut Vec<Box<dyn Component>> {
        &mut self.current_entry_mut().view.components
    }

    /// Process input events and manage focus navigation
    pub fn handle_input(&mut self, input: &mut dyn InputInterface) {
        let events = input.get_events();

        if events.is_empty() {
            return;
        }

        let mut action = ComponentAction::None;

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
                    let focused_index = self.current_entry().focused_index;
                    if let Some(index) = focused_index {
                        let components_len = self.current_entry().view.components.len();
                        if index < components_len {
                            action = self.current_entry_mut().view.components[index]
                                .on_focus_event(FocusEvent::Activated);
                        }
                    }
                }
                _ => {}
            }
        }

        // Pass remaining events to focused component
        let focused_index = self.current_entry().focused_index;
        if let Some(index) = focused_index {
            let components_len = self.current_entry().view.components.len();
            if index < components_len {
                let input_action = self.current_entry_mut().view.components[index].on_input(&events);
                if !matches!(input_action, ComponentAction::None) {
                    action = input_action;
                }
            }
        }

        // Process any actions returned by components
        match action {
            ComponentAction::PushView(builder) => {
                self.push_view(builder);
            }
            ComponentAction::PopView => {
                self.pop_view();
            }
            ComponentAction::None => {}
        }
    }

    /// Move focus to the next focusable component
    fn focus_next(&mut self) {
        let focused_index = self.current_entry().focused_index;
        let start_index = focused_index.map(|i| i + 1).unwrap_or(0);
        let components_len = self.current_entry().view.components.len();

        for i in 0..components_len {
            let index = (start_index + i) % components_len;
            if self.current_entry().view.components[index].is_focusable() {
                self.set_focus(Some(index));
                return;
            }
        }
    }

    /// Move focus to the previous focusable component
    fn focus_previous(&mut self) {
        let focused_index = self.current_entry().focused_index;
        let start_index = focused_index.unwrap_or(0);
        let components_len = self.current_entry().view.components.len();

        for i in 0..components_len {
            let index = (start_index + components_len - 1 - i) % components_len;
            if self.current_entry().view.components[index].is_focusable() {
                self.set_focus(Some(index));
                return;
            }
        }
    }

    /// Set focus to a specific component index
    fn set_focus(&mut self, new_index: Option<usize>) {
        let current_focus = self.current_entry().focused_index;

        if current_focus == new_index {
            return;
        }

        // Notify old component it lost focus
        if let Some(old_index) = current_focus {
            let components_len = self.current_entry().view.components.len();
            if old_index < components_len {
                self.current_entry_mut().view.components[old_index]
                    .on_focus_event(FocusEvent::Lost);
            }
        }

        // Notify new component it gained focus
        if let Some(new_index) = new_index {
            let components_len = self.current_entry().view.components.len();
            if new_index < components_len {
                self.current_entry_mut().view.components[new_index]
                    .on_focus_event(FocusEvent::Gained);
            }
        }

        self.current_entry_mut().focused_index = new_index;
    }

    /// Initialize focus on the first focusable component
    pub fn initialize_focus(&mut self) {
        let current_focus = self.current_entry().focused_index;
        if current_focus.is_none() {
            let components = &self.current_entry().view.components;
            for (index, component) in components.iter().enumerate() {
                if component.is_focusable() {
                    self.set_focus(Some(index));
                    return;
                }
            }
        }
    }
}
