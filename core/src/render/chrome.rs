//! Fixed chrome regions: every screen is laid out as a title bar, a content
//! area, and a hint bar, stacked vertically. This is deliberately *not* a
//! general layout engine (see
//! `.planning/decisions/2026-08-11-ui-framework-reuse-vs-rewrite.md`, which
//! rejects both the old flexbox attempt and a new general-purpose one in
//! favor of "fixed chrome regions + linear stacks").
//!
//! The bar heights are fixed pixel constants, not resolution-derived
//! fractions — but the *regions* are computed from whatever screen size is
//! passed in, and nothing here (or in any widget) hardcodes a screen
//! dimension. The same [`compute_chrome`] call works for a 320x170 T-Embed
//! panel, a 128x32 HUZZAH32 OLED, or an arbitrary test framebuffer.

use embedded_graphics::prelude::{Point, Size};
use embedded_graphics::primitives::Rectangle;

/// Height of the title bar, in pixels.
pub const TITLE_BAR_HEIGHT: u32 = 16;
/// Height of the hint/status bar, in pixels.
pub const HINT_BAR_HEIGHT: u32 = 12;

/// The three fixed regions a screen renders into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChromeLayout {
    pub title: Rectangle,
    pub content: Rectangle,
    pub hint: Rectangle,
}

/// Computes the three chrome regions for a screen of the given size.
/// Saturates rather than panicking on very small screens: a screen too
/// short for a full title/hint bar just gets a squeezed (possibly
/// zero-height) region instead of an arithmetic overflow.
#[must_use]
pub fn compute_chrome(screen_size: Size) -> ChromeLayout {
    let width = screen_size.width;
    let height = screen_size.height;

    let title_height = TITLE_BAR_HEIGHT.min(height);
    let remaining = height.saturating_sub(title_height);
    let hint_height = HINT_BAR_HEIGHT.min(remaining);
    let content_height = remaining.saturating_sub(hint_height);

    let title = Rectangle::new(Point::new(0, 0), Size::new(width, title_height));
    let content = Rectangle::new(
        Point::new(0, title_height as i32),
        Size::new(width, content_height),
    );
    let hint = Rectangle::new(
        Point::new(0, (title_height + content_height) as i32),
        Size::new(width, hint_height),
    );

    ChromeLayout {
        title,
        content,
        hint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_stack_vertically_and_fill_the_screen_exactly() {
        let chrome = compute_chrome(Size::new(320, 170));
        assert_eq!(chrome.title.top_left, Point::new(0, 0));
        assert_eq!(chrome.content.top_left.y, chrome.title.size.height as i32);
        assert_eq!(
            chrome.hint.top_left.y,
            (chrome.title.size.height + chrome.content.size.height) as i32
        );
        let total_height =
            chrome.title.size.height + chrome.content.size.height + chrome.hint.size.height;
        assert_eq!(total_height, 170);
    }

    #[test]
    fn no_literal_resolution_is_baked_in_128x32_also_lays_out_cleanly() {
        let chrome = compute_chrome(Size::new(128, 32));
        let total_height =
            chrome.title.size.height + chrome.content.size.height + chrome.hint.size.height;
        assert_eq!(total_height, 32);
        // Content is squeezed but never underflows.
        assert!(chrome.content.size.height <= 32);
    }

    #[test]
    fn tiny_screen_does_not_panic_or_underflow() {
        let chrome = compute_chrome(Size::new(10, 5));
        let total_height =
            chrome.title.size.height + chrome.content.size.height + chrome.hint.size.height;
        assert_eq!(total_height, 5);
    }

    #[test]
    fn full_width_is_preserved_in_every_region() {
        let chrome = compute_chrome(Size::new(201, 90));
        assert_eq!(chrome.title.size.width, 201);
        assert_eq!(chrome.content.size.width, 201);
        assert_eq!(chrome.hint.size.width, 201);
    }
}
