use crate::{
    gui::input::{InputEvent, KeyCode, KeyEvent},
    simple_gui::{
        components::{Component, ComponentAction, FocusEvent},
        font::Font,
        primitives::{Point, Rectangle},
    },
};

use super::VerticalMenuItem;

pub struct VerticalMenu {
    bounds: Rectangle,
    font: &'static Font,
    items: Vec<VerticalMenuItem>,
    scroll: u32,
    padding: u32,
    selected_index: usize,
    is_focused: bool,
}

impl VerticalMenu {
    pub fn new(bounds: Rectangle, font: &'static Font) -> Self {
        Self {
            bounds,
            font,
            items: Vec::new(),
            scroll: 0,
            padding: 2,
            selected_index: 0,
            is_focused: false,
        }
    }

    pub fn items(&self) -> &Vec<VerticalMenuItem> {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut Vec<VerticalMenuItem> {
        &mut self.items
    }

    /// Calculate the Y position of an item given its index
    fn item_y_position(&self, index: usize) -> i32 {
        let mut y = 0;
        for i in 0..index {
            if i < self.items.len() {
                y += self.items[i].size().height as i32 + self.padding as i32;
            }
        }
        y
    }

    /// Auto-scroll to keep the selected item visible
    fn auto_scroll(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let selected_y = self.item_y_position(self.selected_index);
        let selected_height = self.items[self.selected_index].size().height as i32;
        let viewport_height = self.bounds.height as i32;

        // If item is above viewport, scroll up
        if selected_y < self.scroll as i32 {
            self.scroll = selected_y as u32;
        }
        // If item is below viewport, scroll down
        else if selected_y + selected_height > self.scroll as i32 + viewport_height {
            self.scroll = (selected_y + selected_height - viewport_height).max(0) as u32;
        }
    }
}

impl Component for VerticalMenu {
    fn update(&mut self) {
        // Update all items (for marquee scrolling)
        for item in self.items.iter_mut() {
            item.update();
        }
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn on_focus_event(&mut self, event: FocusEvent) -> ComponentAction {
        match event {
            FocusEvent::Gained => {
                self.is_focused = true;
            }
            FocusEvent::Lost => {
                self.is_focused = false;
            }
            FocusEvent::Activated => {
                // Call the selected item's on_activate callback if it exists
                if !self.items.is_empty() && self.selected_index < self.items.len() {
                    if let Some(ref callback) = self.items[self.selected_index].get_on_activate() {
                        return callback();
                    }
                }
            }
        }
        ComponentAction::None
    }

    fn on_input(&mut self, events: &[InputEvent]) -> ComponentAction {
        if !self.is_focused || self.items.is_empty() {
            return ComponentAction::None;
        }

        for event in events {
            match (event.key_code, event.key_event) {
                (KeyCode::Down, KeyEvent::Clicked) => {
                    // Find next focusable item
                    let mut next = self.selected_index + 1;
                    while next < self.items.len() && !self.items[next].is_focusable() {
                        next += 1;
                    }
                    if next < self.items.len() {
                        self.selected_index = next;
                        self.auto_scroll();
                    }
                }
                (KeyCode::Up, KeyEvent::Clicked) => {
                    // Find previous focusable item
                    if self.selected_index > 0 {
                        let mut prev = self.selected_index - 1;
                        while prev > 0 && !self.items[prev].is_focusable() {
                            prev -= 1;
                        }
                        if self.items[prev].is_focusable() {
                            self.selected_index = prev;
                            self.auto_scroll();
                        }
                    }
                }
                _ => {}
            }
        }
        ComponentAction::None
    }

    fn layout(&mut self) {
        let x = self.bounds.x;
        let mut y = self.bounds.y - self.scroll as i32;

        for (index, item) in self.items.iter_mut().enumerate() {
            // Constrain item width to menu bounds
            item.set_max_width(self.bounds.width);
            item.set_position(Point::new(x, y));
            item.set_selected(self.is_focused && index == self.selected_index);
            item.layout();

            y += item.size().height as i32 + self.padding as i32;
        }
    }

    fn draw(
        &self,
        bounds: Rectangle,
        commands: &mut Vec<crate::simple_gui::render::RenderCommand>,
    ) {
        for item in self.items.iter() {
            item.draw(bounds, commands);
        }
    }
}
