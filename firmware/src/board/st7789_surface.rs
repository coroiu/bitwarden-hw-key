//! [`bhk_core::platform::DisplaySurface`] for the T-Embed's 320x170 ST7789
//! panel, via `mipidsi` over SPI.
//!
//! Per the presentation-surface ADR: the core hands this a
//! `&FrameBuffer565` (Rgb565, the panel's native format) and this
//! adapter's only job is to blit it out over SPI and absorb whatever
//! hardware-specific error `mipidsi`/`esp-idf-hal` produce into this
//! module's own `Error` type — the core itself stays `Error = Infallible`
//! and never sees an SPI or DC-pin failure.
//!
//! # Dependency note (deviates from the original bead wording)
//!
//! The bead described this as "`mipidsi` (add `mipidsi` +
//! `display-interface-spi`)". That combination was accurate for older
//! `mipidsi` releases, but the current `mipidsi` (0.10) dropped the
//! `display-interface`/`display-interface-spi` crates entirely in favor
//! of its own `mipidsi::interface::SpiInterface` (confirmed by reading
//! `mipidsi-0.10.0`'s `Cargo.toml` and `src/interface/spi.rs` — it has no
//! dependency on `display-interface-spi` at all, and takes an
//! `embedded_hal::spi::SpiDevice` + a `&'static mut [u8]` scratch buffer
//! directly). This adapter uses `mipidsi::interface::SpiInterface`; only
//! `mipidsi` itself was added to `firmware/Cargo.toml`.
//!
//! # What is unverified
//!
//! See `board_config`'s module doc for the full list (pin electrical
//! behavior, `mipidsi`'s generic ST7789 init vs. `LilyGo`'s vendor-tuned
//! gamma table, orientation/mirroring). Nothing in this file has been
//! run against a real panel — only compiled for `xtensa-esp32s3-espidf`.

use bhk_core::platform::{DisplaySurface, FrameBuffer565};
use embedded_graphics::Pixel;
use esp_idf_hal::delay::Delay;
use esp_idf_hal::gpio::{AnyOutputPin, GpioError, Output, PinDriver};
use esp_idf_hal::prelude::*;
use esp_idf_hal::spi::{SpiDeviceDriver, SpiDriver};
use esp_idf_hal::sys::EspError;
use mipidsi::interface::{SpiError as MipidsiSpiError, SpiInterface};
use mipidsi::models::ST7789;
use mipidsi::options::{ColorOrder, Orientation, Rotation};
use mipidsi::{Builder, Display};

use crate::board::board_config::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

type Spi = SpiDeviceDriver<'static, SpiDriver<'static>>;
type CtrlPin = PinDriver<'static, AnyOutputPin, Output>;
type Interface = SpiInterface<'static, Spi, CtrlPin>;
type InterfaceError = MipidsiSpiError<esp_idf_hal::spi::SpiError, GpioError>;
type MipidsiDisplay = Display<Interface, ST7789, CtrlPin>;

/// Scratch buffer `SpiInterface` batches pixel data through before each
/// SPI write. Larger is generally faster (fewer, bigger SPI
/// transactions) at the cost of RAM; 2KiB is an untuned starting point,
/// not a measured optimum (there's no hardware here to profile against).
const SPI_TRANSFER_BUFFER_LEN: usize = 2048;

/// SPI clock for the display bus. ST7789 panels commonly tolerate well
/// above this, but the T-Embed's specific panel/wiring has not been
/// tested, so this starts conservative rather than guessing at the
/// panel's real ceiling.
///
/// TODO: verify on hardware and raise if stable.
const SPI_BAUDRATE_MHZ: u32 = 20;

/// The ST7789V panel's NATIVE, unrotated dimensions (170 wide x 320
/// tall, portrait), as required by `mipidsi::Builder::display_size`.
///
/// This is **not** the same as `DISPLAY_WIDTH`/`DISPLAY_HEIGHT` in
/// `board_config`, which are the logical/landscape size (320x170) that
/// the core renders into and that `flush` blits in rotated space.
/// `mipidsi` validates `display_size` against the model's
/// `FRAMEBUFFER_SIZE` (240x320 for `ST7789`) *before* rotation is
/// applied, so passing the landscape 320x170 here overflows the model's
/// 240 max width and mipidsi rejects the config with
/// `InvalidConfiguration(InvalidDisplaySize)`. Confirmed by reading
/// `mipidsi-0.10.0`'s `src/builder.rs` `init()` check (`width >
/// max_width` where `max_width` comes from `MODEL::FRAMEBUFFER_SIZE`)
/// and `src/models/st7789.rs`'s `FRAMEBUFFER_SIZE = (240, 320)`; the
/// real-hardware boot-loop panic this constant fixes is on first T-Embed
/// flash, per bead ai-bitwarden-hw-key-c6e (fix itself not yet
/// hardware-verified as of writing, see that bead for status).
/// `Orientation::Deg90` below then rotates this native 170x320 into the
/// 320x170 landscape the core expects.
const NATIVE_WIDTH: u16 = 170;
const NATIVE_HEIGHT: u16 = 320;

/// Errors from [`St7789Surface::new`].
///
/// `#[allow(dead_code)]`: both variants' inner values are read only
/// through the derived `Debug` impl (`main.rs` calls `.expect(...)` on
/// construction, which `Debug`-formats the error into the panic message);
/// `rustc`'s dead-code analysis doesn't count that as a "real" read of
/// the field, so it would otherwise warn on every variant here.
#[derive(Debug)]
#[allow(dead_code)]
pub enum St7789SurfaceInitError {
    /// Failed to acquire or configure a GPIO/SPI peripheral.
    Peripheral(EspError),
    /// `mipidsi`'s panel init sequence failed.
    Display(mipidsi::InitError<InterfaceError, GpioError>),
}

impl From<EspError> for St7789SurfaceInitError {
    fn from(e: EspError) -> Self {
        Self::Peripheral(e)
    }
}

/// Errors from [`St7789Surface::flush`]. This is the
/// [`DisplaySurface::Error`] the core's `Error = Infallible` draw path
/// never has to see — it only reaches whatever calls `flush` on this
/// concrete type.
///
/// `#[allow(dead_code)]`: see the identical note on
/// `St7789SurfaceInitError` — `run`'s main loop drops a `flush` error
/// (per its own doc comment on why), so nothing pattern-matches these
/// fields directly; they exist for whatever future caller wants to
/// inspect or log them, and for the `Debug` panic-message path.
#[derive(Debug)]
#[allow(dead_code)]
pub enum St7789SurfaceError {
    /// The SPI write to the panel failed.
    Spi(InterfaceError),
    /// The framebuffer handed to `flush` isn't sized for this panel.
    /// `FrameBuffer565`'s resolution is a runtime parameter (see
    /// `core/src/render/framebuffer.rs`), so this is a real possibility,
    /// not just defensive paranoia, if a caller wires up the wrong size.
    FramebufferSizeMismatch { expected: (u32, u32), actual: (u32, u32) },
}

/// `DisplaySurface` for the T-Embed's ST7789, driven over SPI2.
pub struct St7789Surface {
    display: MipidsiDisplay,
    backlight: CtrlPin,
    /// Kept alive for as long as the display is: dropping this would
    /// (per `LilyGo`'s own `tft.ino`) cut power to the panel. Never read
    /// after `new`, hence `#[allow(dead_code)]`-shaped usage — its job is
    /// existing, not being called.
    _peripheral_power: CtrlPin,
}

impl St7789Surface {
    /// Powers on the T-Embed's shared peripheral rail, brings up the SPI
    /// bus, initializes the ST7789 via `mipidsi`, and turns on the
    /// backlight.
    ///
    /// # Errors
    ///
    /// Returns [`St7789SurfaceInitError`] if any GPIO/SPI peripheral
    /// fails to configure, or if the panel doesn't respond to `mipidsi`'s
    /// init sequence the way the generic ST7789 model expects.
    ///
    /// # Panics
    ///
    /// Never at runtime for a normal call; the `Box::leak` below is a
    /// one-time, intentional heap allocation (see its comment), not a
    /// fallible operation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        spi: esp_idf_hal::spi::SPI2,
        sclk: AnyOutputPin,
        mosi: AnyOutputPin,
        cs: AnyOutputPin,
        dc: AnyOutputPin,
        reset: AnyOutputPin,
        backlight: AnyOutputPin,
        peripheral_power_on: AnyOutputPin,
    ) -> Result<Self, St7789SurfaceInitError> {
        let mut peripheral_power = PinDriver::output(peripheral_power_on)?;
        // Per LilyGo's `tft.ino`: PIN_POWER_ON must be driven high before
        // `tft.begin()` or the panel's power rail is off entirely. The
        // settle delay before touching SPI is a guess (untested) — long
        // enough to be safe on a busy-wait, not derived from a rail
        // spec sheet we don't have.
        peripheral_power.set_high()?;
        Delay::new_default().delay_ms(10);

        let driver_config = esp_idf_hal::spi::config::DriverConfig::new();
        let spi_config = esp_idf_hal::spi::config::Config::new()
            .baudrate(SPI_BAUDRATE_MHZ.MHz().into())
            .write_only(true);
        let spi_device = SpiDeviceDriver::new_single(
            spi,
            sclk,
            mosi,
            Option::<esp_idf_hal::gpio::AnyIOPin>::None,
            Some(cs),
            &driver_config,
            &spi_config,
        )?;

        let dc_pin = PinDriver::output(dc)?;
        let reset_pin = PinDriver::output(reset)?;
        let mut backlight_pin = PinDriver::output(backlight)?;

        // `SpiInterface` needs a `&'static mut [u8]` scratch buffer, and
        // borrowing it from a field of `Self` would make this struct
        // self-referential (not expressible in safe Rust without extra
        // indirection). `Box::leak` is the standard, deliberate way
        // around that for a peripheral meant to live for the entire
        // program: it trades a one-time, bounded (2KiB) heap allocation
        // that's never freed for avoiding a self-referential struct or
        // an `unsafe` pinned-borrow workaround. No `unsafe` involved.
        let spi_buffer: &'static mut [u8] = Box::leak(Box::new([0u8; SPI_TRANSFER_BUFFER_LEN]));
        let interface = SpiInterface::new(spi_device, dc_pin, spi_buffer);

        let mut delay = Delay::new_default();
        let display = Builder::new(ST7789, interface)
            .reset_pin(reset_pin)
            .display_size(NATIVE_WIDTH, NATIVE_HEIGHT)
            .color_order(ColorOrder::Rgb)
            // TODO: verify on hardware. `tft.ino` uses
            // `tft.setRotation(3)` (TFT_eSPI's rotation index 3) to get
            // 320x170 landscape; the equivalent mipidsi `Orientation` has
            // not been confirmed against a real panel; landscape at all
            // (vs. a rotated/mirrored image) is the only thing that's a
            // reasonably confident guess here.
            .orientation(Orientation::new().rotate(Rotation::Deg90))
            .init(&mut delay)
            .map_err(St7789SurfaceInitError::Display)?;

        backlight_pin.set_high()?;

        Ok(Self {
            display,
            backlight: backlight_pin,
            _peripheral_power: peripheral_power,
        })
    }
}

impl DisplaySurface for St7789Surface {
    type Error = St7789SurfaceError;

    fn flush(&mut self, framebuffer: &FrameBuffer565) -> Result<(), Self::Error> {
        let (expected_w, expected_h) = (u32::from(DISPLAY_WIDTH), u32::from(DISPLAY_HEIGHT));
        if framebuffer.width() != expected_w || framebuffer.height() != expected_h {
            return Err(St7789SurfaceError::FramebufferSizeMismatch {
                expected: (expected_w, expected_h),
                actual: (framebuffer.width(), framebuffer.height()),
            });
        }

        self.display
            .set_pixels(
                0,
                0,
                DISPLAY_WIDTH - 1,
                DISPLAY_HEIGHT - 1,
                framebuffer.pixels().map(|Pixel(_, color)| color),
            )
            .map_err(St7789SurfaceError::Spi)
    }
}

impl Drop for St7789Surface {
    fn drop(&mut self) {
        // Best-effort: turn the backlight off on drop rather than leaving
        // a dead-looking-but-powered panel lit. Never expected to run in
        // normal operation (this surface lives for the process lifetime).
        let _ = self.backlight.set_low();
    }
}
