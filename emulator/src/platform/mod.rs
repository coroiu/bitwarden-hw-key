//! Host implementations of `bhk_core::platform`'s capability-bundle traits
//! (`DisplaySurface`, `InputSource`, `Clock`, `Storage`), plus a small
//! `Platform` bundle wiring them together.
//!
//! This is **W4** of the M0 platform migration: it gives the emulator real
//! adapters for the trait seams `bhk_core::platform` froze in W1, built on
//! top of the render core (`FrameBuffer565`, `Navigator`) from W3.
//!
//! Two `DisplaySurface`s exist side by side, per the presentation-surface
//! ADR: [`headless_surface::HeadlessSurface`] (PNG-on-demand, no window)
//! and [`minifb_surface::MinifbSurface`] (a real `minifb` window). Both
//! consume the exact same `FrameBuffer565`, so they are provably
//! pixel-identical — see `emulator/tests/surface_parity.rs`.
//!
//! What's deliberately NOT here (later beads):
//! - The unified `Platform`-generic main loop wiring input -> app ->
//!   render -> present across all run modes (W7). [`host_platform`] gives
//!   just enough wiring to construct a `Platform` and render one scene for
//!   verification; it is not that loop.
//! - The headless HTTP `NavIntent` injection + screenshot protocol (W5).
//!   [`HeadlessSurface`] only exposes an in-process PNG-encode method; wiring
//!   it to an HTTP endpoint is W5's job.
//! - The T-Embed/ST7789 board `DisplaySurface` (W6).

pub mod clock;
pub mod headless_surface;
pub mod host_platform;
pub mod input;
pub mod minifb_surface;
pub mod storage;

pub use clock::HostClock;
pub use headless_surface::HeadlessSurface;
pub use host_platform::HostPlatform;
pub use input::{NoopInput, WindowedInput};
pub use minifb_surface::MinifbSurface;
pub use storage::{FileStorage, FileStorageError};
