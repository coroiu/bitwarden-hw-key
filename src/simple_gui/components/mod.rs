mod label;
mod vertical_menu;

pub use label::*;
pub use vertical_menu::*;

use super::{primitives::Rectangle, render::RenderCommand};
use crate::gui::input::InputEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusEvent {
    /// Component gained focus (e.g., user navigated to it)
    Gained,
    /// Component lost focus (e.g., user navigated away)
    Lost,
    /// Component was activated (e.g., user pressed select/enter while focused)
    Activated,
}

/// Actions that components can return to control navigation and document behavior
pub enum ComponentAction {
    /// No action needed
    None,
    /// Push a new view onto the navigation stack
    PushView(Box<dyn Fn() -> super::layout::view::StandaloneView>),
    /// Pop the current view from the navigation stack
    PopView,
}

// pub trait UninitializedComponent {
//     fn initialize(self, ) -> Box<dyn Component>;
// }

pub trait Component {
    fn update(&mut self) {}

    fn layout(&mut self) {}

    fn draw(&self, _: Rectangle, _: &mut Vec<RenderCommand>) {}

    /// Returns whether this component can receive focus
    fn is_focusable(&self) -> bool {
        false
    }

    /// Called when focus-related events occur
    /// Returns an optional action for the document to process
    fn on_focus_event(&mut self, _event: FocusEvent) -> ComponentAction {
        ComponentAction::None
    }

    /// Called when this component has focus and receives input events
    /// Only called if is_focusable() returns true
    /// Returns an optional action for the document to process
    fn on_input(&mut self, _events: &[InputEvent]) -> ComponentAction {
        ComponentAction::None
    }
}
