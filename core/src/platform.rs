//! Platform capability bundle: the four traits the app core is injected
//! with (`DisplaySurface`, `InputSource`, `Clock`, `Storage`), per the
//! presentation-surface ADR, plus the [`Platform`] trait that groups them.
//! This module is a **trait-seam placeholder**: shapes are frozen here so
//! `firmware` and `emulator` have a shared contract to build against, but
//! no implementations exist yet (host surfaces land in W4, the T-Embed
//! board adapter in W6).
//!
//! The render core itself — including the real [`FrameBuffer565`]
//! definition `DisplaySurface::flush` refers to — lives in
//! `crate::render` (built in W3, this bead); it's re-exported here only so
//! this module's signatures stay meaningful without a second definition.
//!
//! See: .planning/decisions/2026-08-11-presentation-surface-run-mode-seam.md

use crate::input::NavIntent;
use std::time::Instant;

pub use crate::render::FrameBuffer565;

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

/// Capability bundle: groups the four injected platform traits behind a
/// single generic parameter, so app-wiring code (the unified main loop,
/// W7) can be generic over "a platform" instead of threading four separate
/// type parameters through every function signature.
///
/// **Definition only.** No concrete `Platform` implementation exists yet —
/// those are assembled once the headless/windowed surfaces (W4) and the
/// T-Embed board adapter (W6) exist.
pub trait Platform {
    type Display: DisplaySurface;
    type Input: InputSource;
    type Clock: Clock;
    type Storage: Storage;

    fn display(&mut self) -> &mut Self::Display;
    fn input(&mut self) -> &mut Self::Input;
    fn clock(&self) -> &Self::Clock;
    fn storage(&mut self) -> &mut Self::Storage;
}
