//! The render core: platform-free UI rendering and navigation built on
//! `embedded-graphics`, replacing the retired `gui`/`simple_gui` engines
//! (custom RGBA rasterizer, baked ASCII fonts, character-skip marquee
//! clipping — see
//! `.planning/decisions/2026-08-11-ui-framework-reuse-vs-rewrite.md`).
//!
//! Module map:
//! - [`framebuffer`]: the canonical Rgb565 in-RAM framebuffer
//!   ([`FrameBuffer565`]), the app core's single render output.
//! - [`widget`]: the retained-mode [`Widget`] trait, [`Action`], and
//!   [`FocusEvent`].
//! - [`chrome`]: fixed title/content/hint region layout
//!   ([`compute_chrome`]).
//! - [`list`]: [`VerticalList`], the one content widget this bead needs.
//! - [`screen`]: [`Screen`], one entry in the navigation stack.
//! - [`navigator`]: [`Navigator`], owning the screen stack.
//!
//! - [`theme`]: the M1 visual design language — the semantic color
//!   palette, per-role `u8g2-fonts` accessors, `open_iconic` icon
//!   codepoints, and the shared chip/selection drawing primitives (bead
//!   `ai-bitwarden-hw-key-0v8.8`). `screen`/`list`/`credential_list_view`
//!   render through this instead of `embedded-graphics`' built-in
//!   `MonoFont`/`WebColors`.
//!
//! What's deliberately NOT here yet (later beads, see the bead's explicit
//! out-of-scope list):
//! - Any `DisplaySurface`/`InputSource`/`Clock`/`Storage` *implementation*
//!   (those traits themselves are frozen in `crate::platform` from W1).

// `embedded-graphics` represents position as `Point` (`i32`) and extent as
// `Size` (`u32`) — a mismatch baked into the upstream library, not
// introduced here. Converting between the two throughout a layout/render
// module is therefore idiomatic embedded-graphics usage, not sloppiness;
// every display this project targets is a few hundred pixels per side, so
// none of these conversions can realistically wrap, truncate, or lose a
// sign. Allowed at the module level rather than peppering every call site.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

pub mod chrome;
pub mod framebuffer;
pub mod list;
pub mod navigator;
pub mod screen;
pub mod theme;
pub mod widget;

pub use chrome::{compute_chrome, ChromeLayout};
pub use framebuffer::FrameBuffer565;
pub use list::{ListItem, VerticalList, ROW_HEIGHT};
pub use navigator::Navigator;
pub use screen::Screen;
pub use widget::{Action, ChromeContribution, ChromeStatus, FocusEvent, Widget};
