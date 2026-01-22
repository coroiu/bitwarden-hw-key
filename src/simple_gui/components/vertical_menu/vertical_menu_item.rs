use crate::simple_gui::{
    components::{Component, ComponentAction, Label},
    font::Font,
    primitives::{Color, Point, Rectangle, Size},
    render::RenderCommand,
};

pub struct VerticalMenuItem {
    label: Label,
    text: String,
    font: &'static Font,
    is_selected: bool,
    position: Point,
    focusable: bool,
    scroll_offset: i32,
    scroll_counter: u32,
    max_width: Option<u32>,
    on_activate: Option<Box<dyn Fn() -> ComponentAction>>,
}

const HORIZONTAL_MARGIN: u32 = 2;
const SCROLL_SPEED: u32 = 3; // Update every N frames (lower = faster)

impl VerticalMenuItem {
    pub fn new(font: &'static Font, text: &str) -> Self {
        Self {
            label: Label::new(Point::zero(), font, text),
            text: text.to_owned(),
            font,
            is_selected: false,
            position: Point::zero(),
            focusable: true,
            scroll_offset: 0,
            scroll_counter: 0,
            max_width: None,
            on_activate: None,
        }
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    pub fn on_activate<F>(mut self, f: F) -> Self
    where
        F: Fn() -> ComponentAction + 'static,
    {
        self.on_activate = Some(Box::new(f));
        self
    }

    pub fn get_on_activate(&self) -> &Option<Box<dyn Fn() -> ComponentAction>> {
        &self.on_activate
    }

    pub fn set_max_width(&mut self, max_width: u32) {
        self.max_width = Some(max_width);
    }

    pub fn size(&self) -> Size {
        let label_size = self.label.size();
        // Add padding for border (1px) and horizontal margins (2px each side)
        let calculated_width = label_size.width + 2 + HORIZONTAL_MARGIN * 2;

        // Constrain to max_width if set
        let width = if let Some(max_width) = self.max_width {
            calculated_width.min(max_width)
        } else {
            calculated_width
        };

        Size::new(width, label_size.height + 2)
    }

    pub fn set_position(&mut self, position: Point) {
        self.position = position;
        // Offset label by 1 pixel for border + horizontal margin
        self.label.set_position(Point::new(
            position.x + 1 + HORIZONTAL_MARGIN as i32,
            position.y + 1,
        ));
    }

    pub fn set_selected(&mut self, selected: bool) {
        let was_selected = self.is_selected;
        self.is_selected = selected;

        // Reset scroll when losing focus
        if was_selected && !selected {
            self.scroll_offset = 0;
            self.scroll_counter = 0;
        }
    }

    fn available_width(&self) -> u32 {
        let size = self.size();
        // Subtract borders (2px) and horizontal margins (4px)
        size.width.saturating_sub(2 + HORIZONTAL_MARGIN * 2)
    }

    fn text_width(&self) -> u32 {
        self.text
            .chars()
            .map(|c| self.font.get_character(c).image_buffer.width as u32)
            .sum::<u32>()
            + (self.text.chars().count().saturating_sub(1)) as u32
                * self.font.letter_spacing as u32
    }
}

impl Component for VerticalMenuItem {
    fn is_focusable(&self) -> bool {
        self.focusable
    }

    fn update(&mut self) {
        if !self.is_selected {
            return;
        }

        let text_width = self.text_width();
        let available_width = self.available_width();

        // Only scroll if text is longer than available width
        if text_width <= available_width {
            return;
        }

        // Update scroll counter
        self.scroll_counter += 1;
        if self.scroll_counter < SCROLL_SPEED {
            return;
        }
        self.scroll_counter = 0;

        // Calculate max scroll (text can scroll until the end is visible)
        let max_scroll = (text_width - available_width) as i32;

        // Scroll right until we reach the end, then reset to start
        self.scroll_offset += 1;

        if self.scroll_offset > max_scroll + 10 {
            // Add a pause at the end before wrapping
            self.scroll_offset = -10; // Pause at start
        }
    }

    fn layout(&mut self) {
        self.label.layout();
    }

    fn draw(&self, bounds: Rectangle, commands: &mut Vec<RenderCommand>) {
        // Draw border if selected
        if self.is_selected {
            let size = self.size();
            let border_color = Color::white();
            let border_thickness = 1;

            // Top border
            commands.push(RenderCommand::SolidColor(
                border_color,
                Rectangle::new(
                    self.position.x,
                    self.position.y,
                    size.width,
                    border_thickness,
                ),
            ));

            // Bottom border
            commands.push(RenderCommand::SolidColor(
                border_color,
                Rectangle::new(
                    self.position.x,
                    self.position.y + size.height as i32 - border_thickness as i32,
                    size.width,
                    border_thickness,
                ),
            ));

            // Left border
            commands.push(RenderCommand::SolidColor(
                border_color,
                Rectangle::new(
                    self.position.x,
                    self.position.y,
                    border_thickness,
                    size.height,
                ),
            ));

            // Right border
            commands.push(RenderCommand::SolidColor(
                border_color,
                Rectangle::new(
                    self.position.x + size.width as i32 - border_thickness as i32,
                    self.position.y,
                    border_thickness,
                    size.height,
                ),
            ));
        }

        // Draw text with scrolling and clipping
        let original_pos = Point::new(
            self.position.x + 1 + HORIZONTAL_MARGIN as i32,
            self.position.y + 1,
        );

        // Calculate how many characters to skip from the beginning
        let skip_chars = if self.scroll_offset > 0 {
            let mut total_width = 0u32;
            let mut chars_to_skip = 0usize;

            for (i, c) in self.text.chars().enumerate() {
                let char_width = self.font.get_character(c).image_buffer.width as u32;
                let spacing = if i > 0 { self.font.letter_spacing as u32 } else { 0 };

                if total_width + char_width <= self.scroll_offset.max(0) as u32 {
                    total_width += char_width + spacing;
                    chars_to_skip = i + 1;
                } else {
                    break;
                }
            }
            chars_to_skip
        } else {
            0
        };

        // Get the substring to display
        let display_text: String = self.text.chars().skip(skip_chars).collect();

        if !display_text.is_empty() {
            // Create label with the visible portion of text at original position
            let mut temp_label = Label::new(original_pos, self.font, &display_text);
            temp_label.layout();

            // Create clipping rectangle
            let clip_rect = Rectangle::new(
                original_pos.x,
                original_pos.y,
                self.available_width(),
                self.label.size().height,
            );

            temp_label.draw(clip_rect, commands);
        }
    }
}
