# Presentation Surface and Run-Mode Seam

**Date**: 2026-08-11
**Status**: Accepted

## Context

The project now targets three run modes (headless, windowed, real-target) over a shared application core. The core must render consistently across all three modes so that a headless screenshot faithfully reproduces what the windowed emulator shows and what the hardware will display. This requires a hardware-independent presentation abstraction.

At the same time, the migration to the Lilygo T-Embed (320x170 color ST7789) introduces a choice: should the core render in RGB888 (full color) and quantize at the device boundary, or commit to Rgb565 (the panel's native format) throughout? The latter eliminates any color fidelity mismatch between headless and hardware.

## Decision

The platform is abstracted as a **capability bundle** of four injected traits:

```rust
pub trait DisplaySurface {
    fn flush(&mut self, framebuffer: &FrameBuffer565) -> Result<(), Self::Error>;
    type Error;
}

pub trait InputSource {
    fn poll(&mut self) -> Vec<InputEvent>;
}

pub trait Clock {
    fn now(&self) -> Instant;
}

pub trait Storage {
    fn get(&self, key: &str) -> Option<Vec<u8>>;
    fn set(&mut self, key: &str, value: Vec<u8>) -> Result<(), Self::Error>;
    type Error;
}
```

The app core is **platform-free**: it does not reference any of these trait implementations, only the traits themselves. The core owns a single **shared in-RAM framebuffer in Rgb565 format** (`embedded-graphics-framebuf` or equivalent) as its sole render output. All three run modes differ only in their `DisplaySurface::flush()` implementation:

- **Headless**: buffers the framebuffer to a PNG byte array (captured on demand via HTTP endpoint)
- **Windowed**: copies the framebuffer to a minifb window
- **Real-target**: transfers the framebuffer to the ST7789 via SPI

The core's `DrawTarget` is bound to `Error = Infallible` (only `draw()` ops can fail, and they are re-fallible in the surface adapters). Device-specific errors (e.g., ST7789 SPI write failures) are absorbed in the surface adapter's `DisplaySurface::Error` type, not exposed to the app.

**Canonical pixel format**: Rgb565 throughout the core. This is the panel's native format and the most memory-efficient encoding for embedded graphics. Desktop surfaces (headless, windowed) render at Rgb565 directly; if a future target uses a different format, the adapter layer quantizes, not the core.

**Run-mode selection**: the desktop binary (`src/bin/desktop.rs` or equivalent) accepts a `--headless` flag at runtime. Both headless and windowed surface implementations remain compiled and linked in the same binary, preventing drift. The real-target binary (`firmware`) includes only the ST7789 surface.

## Rationale

- **Shared render pipeline** ensures headless screenshots are pixel-for-pixel identical to what the window or hardware would show. No "looks better on hardware" surprises.
- **Rgb565 canonical** avoids quantization artifacts and simplifies memory budgets. The ST7789 natively expects Rgb565, and emulators can display it without color loss.
- **Trait-based injection** decouples the app from any specific HAL, driver, or emulator library, keeping the core portable and testable in isolation.
- **Single source of truth** (one framebuffer) reduces buffer synchronization bugs and makes the three modes honest by construction.
- **Runtime mode selection** (not compile-time cfg) ensures both host surfaces are always built, catching divergence early.

## Alternatives Considered

- **RGB888 core with quantize-at-device.** Core renders in full color; each surface quantizes to its native format.
  - **Pros**: core has more color information available.
  - **Cons**: headless screenshots are more colorful than hardware output, violating three-mode fidelity. Adds quantization code and complexity in device adapters.
  - **Verdict**: Rejected in favor of Rgb565-canonical.

- **Platform abstraction via embedded-io-async HAL.** Reimplement the four traits on top of an existing I/O abstraction.
  - **Pros**: leverage existing ecosystem.
  - **Cons**: embedded-io-async is designed for async HAL (good for real hardware), but headless render-and-capture is inherently sync and synchronous. Adds indirection without payoff.
  - **Verdict**: Rejected. Use simple sync traits; they're sufficient.

- **Compile-time feature flags to select surface.** `cfg!(headless)` determines which surface is compiled in.
  - **Pros**: binary size on embedded.
  - **Cons**: different code paths on host and hardware introduce drift risk; can't test surface switching during development.
  - **Verdict**: Rejected in favor of runtime selection on host, compile-time on real-target (where only one is shipped).

## Consequences

### Positive
- Headless test results are trustworthy: they represent what users will see.
- No color fidelity surprises when moving features from emulator to hardware.
- Core is hardware-agnostic and testable in unit tests (inject mock surfaces).
- Surface layer is simple and focused: just data I/O, not business logic.

### Negative
- Rgb565 is less color-rich than full 24-bit color. (Acceptable: ST7789 is Rgb565 anyway.)
- On-device quantization errors if a future target uses a different format. (Mitigation: the adapter quantizes; core stays Rgb565.)
- Host binary includes both headless and windowed code. (Negligible impact; the real-target binary ships only its surface.)

## References

- Owners: Fern (fe-architect), Ruby (rust-embedded-supervisor), Tess (tester)
- Related decisions: [2026-08-11-three-mode-testability.md](2026-08-11-three-mode-testability.md)
- Related: [2026-01-21-desktop-emulation.md](2026-01-21-desktop-emulation.md)
