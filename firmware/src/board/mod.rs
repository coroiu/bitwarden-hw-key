//! Lilygo T-Embed (ESP32-S3) board adapter: concrete implementations of
//! `bhk_core::platform`'s four traits (`DisplaySurface`, `InputSource`,
//! `Clock`, `Storage`) for the real hardware target, assembled by
//! [`platform::BoardPlatform`] and driven by the unified
//! `bhk_core::run` loop from `main.rs` (W7).
//!
//! # What is and isn't verified
//!
//! This module (and `main.rs`'s wiring of it) **builds and links** for
//! `xtensa-esp32s3-espidf`. Nothing here has run against real hardware —
//! there is no T-Embed attached to the machine this was written on (no
//! `/dev/tty.*` device, confirmed during the W5 SDK spike). See each
//! submodule's doc comment for exactly what is and isn't verified about
//! its own piece (pin electrical behavior, panel init sequence, encoder
//! timing, ...).

pub mod board_config;
pub mod clock;
pub mod nvs_storage;
pub mod platform;
pub mod rotary_input;
pub mod st7789_surface;

// Only re-exported at this level if `main.rs` names it directly. This is a
// `bin` crate with no external consumer of `board`'s `pub` items, so an
// unused re-export here is flagged by `unused_imports` same as any other
// unused `use` — the error types (`NvsStorageError`,
// `St7789Surface{Error,InitError}`) and `EspClock` are still reachable via
// their own submodule paths if a future caller needs to name them (e.g.
// to match on a specific error variant); `main.rs` today only propagates
// them opaquely via `?`/`.expect()`.
pub use board_config::{BoardPeripherals, DISPLAY_HEIGHT, DISPLAY_WIDTH};
pub use nvs_storage::NvsStorage;
pub use platform::BoardPlatform;
pub use rotary_input::RotaryEncoderInput;
pub use st7789_surface::St7789Surface;
