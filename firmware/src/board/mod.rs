//! Lilygo T-Embed (ESP32-S3) board adapter: concrete implementations of
//! `bhk_core::platform`'s four traits (`DisplaySurface`, `InputSource`,
//! `Clock`, `Storage`) for the real hardware target.
//!
//! # Scope of this module (W6)
//!
//! This module provides the trait implementations and the pin map. It
//! deliberately does **not**:
//! - assemble them into a concrete `bhk_core::platform::Platform` impl,
//! - wire that into a running main loop, or
//! - touch `main.rs`'s existing (old HUZZAH32) engine wiring beyond the
//!   minimal pin fix needed to keep it compiling under the ESP32-S3
//!   target switch (see `main.rs`'s comment at the OLED-pin setup).
//!
//! All of that is the unified Platform-generic main loop, W7, per the M0
//! epic. Until then, this module exists to prove the T-Embed-specific
//! code **compiles** for `xtensa-esp32s3-espidf`; nothing here has run
//! against real hardware (none is attached to the machine this was
//! written on — no `/dev/tty.*` device, confirmed during the W5 SDK
//! spike). See each submodule's doc comment for exactly what is and
//! isn't verified.
// Definitions only; W7 wires these into a running main loop. Until then,
// nothing in this binary crate consumes the pub re-exports below (there's
// no external consumer of a `bin` crate's `pub` items), so both lints
// fire on every item here.
#![allow(dead_code, unused_imports)]

pub mod board_config;
pub mod clock;
pub mod nvs_storage;
pub mod rotary_input;
pub mod st7789_surface;

pub use board_config::{BoardPeripherals, DISPLAY_HEIGHT, DISPLAY_WIDTH};
pub use clock::EspClock;
pub use nvs_storage::{NvsStorage, NvsStorageError};
pub use rotary_input::RotaryEncoderInput;
pub use st7789_surface::{St7789Surface, St7789SurfaceError, St7789SurfaceInitError};
