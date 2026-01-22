use crate::simple_gui::{
    components::{Component, Label},
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
}

const HORIZONTAL_MARGIN: u32 = 2;
const SCROLL_SPEED: u32 = 8; // Update every N frames

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
        }
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
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
        let available_width = self.available_width();

        // Calculate text position with scroll offset
        let text_x = self.position.x + 1 + HORIZONTAL_MARGIN as i32 - self.scroll_offset.max(0);
        let text_y = self.position.y + 1;

        // Calculate text bounds
        let text_width = self.text_width();
        let text_height = self.label.size().height;
        let text_rect = Rectangle::new(text_x, text_y, text_width, text_height);

        // Create clipping rectangle (where text should be visible)
        let clip_rect = Rectangle::new(
            self.position.x + 1 + HORIZONTAL_MARGIN as i32,
            text_y,
            available_width,
            text_height,
        );

        // Intersect with both bounds and clip rect
        let visible_rect = bounds.intersect(clip_rect);
        if visible_rect.bounds.width > 0 && visible_rect.bounds.height > 0 {
            let text_visible = visible_rect.bounds.intersect(text_rect);
            if text_visible.bounds.width > 0 && text_visible.bounds.height > 0 {
                commands.push(RenderCommand::Text(
                    Color::white(),
                    text_visible.bounds,
                    self.text.clone(),
                    self.font,
                ));
            }
        }
    }
}
