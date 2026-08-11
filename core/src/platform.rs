//! Platform capability bundle: the four traits the app core is injected
//! with (`DisplaySurface`, `InputSource`, `Clock`, `Storage`), per the
//! presentation-surface ADR. This module is a **placeholder seam only**:
//! trait shapes are frozen here so `firmware` and `emulator` have a shared
//! contract to build against, but no implementations exist yet (host
//! surfaces land in W4, the T-Embed board adapter in W6) and the render
//! core itself is not built here (W3).
//!
//! See: .planning/decisions/2026-08-11-presentation-surface-run-mode-seam.md

use crate::input::NavIntent;
use std::time::Instant;

/// Placeholder for the canonical Rgb565 framebuffer the app core will own
/// and render into. Real definition (backing storage, pixel access,
/// `DrawTarget` impl) lands with the render core in W3; this stub exists
/// only so `DisplaySurface` has a concrete signature today.
#[derive(Debug, Default)]
pub struct FrameBuffer565 {
    _placeholder: (),
}

/// Transfers the shared framebuffer to a physical or virtual display.
/// Implementations: headless (PNG capture), windowed (minifb), real-target
/// (ST7789 over SPI).
pub trait DisplaySurface {
    type Error;

    /// # Errors
    ///
    /// Returns `Self::Error` if the framebuffer could not be transferred to
    /// the underlying display (e.g. an SPI write failure on real hardware).
    fn flush(&mut self, framebuffer: &FrameBuffer565) -> Result<(), Self::Error>;
}

/// Polls for input, already resolved to the semantic `NavIntent` level
/// (see `crate::input`). Per the ADR, raw platform events (encoder ticks,
/// keycodes) stay driver-local and are mapped to `NavIntent` before
/// reaching this trait.
pub trait InputSource {
    fn poll(&mut self) -> Vec<NavIntent>;
}

/// Wall-clock access, injected so the app core never calls platform time
/// APIs directly. `std::time::Instant` is available on both targets today
/// (esp-idf-svc's `std` feature provides it), so no custom time type is
/// needed yet.
pub trait Clock {
    fn now(&self) -> Instant;
}

/// Persistent key/value storage. Implementations: native filesystem
/// (emulator), NVS (firmware).
pub trait Storage {
    type Error;

    fn get(&self, key: &str) -> Option<Vec<u8>>;

    /// # Errors
    ///
    /// Returns `Self::Error` if the value could not be persisted (e.g. an
    /// NVS write failure on real hardware, or a filesystem error on host).
    fn set(&mut self, key: &str, value: Vec<u8>) -> Result<(), Self::Error>;
}
