//! [`bhk_core::platform::DisplaySurface`] for the T-Embed's 320x170 ST7789
//! panel, via `mipidsi` over SPI for INIT, then a raw DMA-backed
//! full-frame blit for the hot per-frame path (see "Hot-path blit" below).
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
//! # Hardware status
//!
//! **HARDWARE-CONFIRMED** on a real Lilygo T-Embed CC1101 (bead
//! ai-bitwarden-hw-key-c6e / ai-bitwarden-hw-key-dvm): boots to a stable
//! main loop with no panic, and the panel is confirmed by direct human
//! observation to be right-side up, correctly filling the panel (no
//! offset/cropping), with correct colors. That covers `display_size`,
//! `display_offset`, `invert_colors`, and `orientation` below. What
//! remains unverified: whether `mipidsi`'s generic ST7789 init sequence
//! matches `LilyGo`'s vendor-tuned gamma/porch-timing table closely
//! enough for contrast/gamma fidelity (only confirmed "correct colors",
//! not pixel-perfect factory-firmware parity).
//!
//! # Hot-path blit: raw DMA write, bypassing `mipidsi::Display::set_pixels`
//!
//! Measured on real hardware (bead ai-bitwarden-hw-key-ego): raising
//! `SPI_BAUDRATE_MHZ` 20->40 (still below) barely helped refresh rate,
//! and instrumented per-frame timing showed `flush` (this module's job)
//! taking ~103ms in a release build — vs. a theoretical pure SPI-bus
//! transfer time of only ~22ms for a full 320x170x2-byte frame at
//! 40MHz. The gap is CPU-side, and tracked to two causes, both fixed
//! here:
//!
//! 1. **DMA was never enabled.** `esp_idf_hal::spi::config::DriverConfig`
//!    defaults to `Dma::Disabled`, and without DMA, ESP-IDF's SPI master
//!    driver caps a single hardware transaction at `TRANS_LEN` (64 bytes
//!    on this chip, confirmed by reading `esp-idf-hal-0.45.2`'s
//!    `src/spi.rs`) — every larger `write()` call gets silently
//!    re-chunked into many ~64-byte transactions internally, each with
//!    its own fixed per-transaction setup/completion overhead. A 108KB
//!    frame at 64 bytes/transaction is ~1700 transactions. Fixed by
//!    enabling DMA (see [`DMA_CHUNK_BYTES`] for the exact chunk size —
//!    ESP32-S3's SPI hardware has its own fixed 32,768-byte-per-
//!    transaction ceiling, so this is a handful of large DMA
//!    transactions per frame, not literally one).
//! 2. **`mipidsi::Display::set_pixels`'s per-pixel iterator.** Even with
//!    DMA, feeding pixels through `framebuffer.pixels()` (which computes
//!    an `embedded_graphics::Point` per pixel via `FrameBuf`'s
//!    `IntoIterator`, entirely unused for a full-frame linear blit) into
//!    `mipidsi::interface::Interface::send_pixels`'s buffer-chunk-then-
//!    write loop adds real per-pixel overhead the hot path doesn't need.
//!
//! The fix: `mipidsi::Builder`/`Display` are still used for **init only**
//! (unchanged) — that sequence is small, infrequent, and already proven
//! correct on hardware, not worth re-deriving by hand. Immediately after
//! `Builder::init()` succeeds, [`St7789Surface::new`] calls
//! `Display::release()` then `SpiInterface::release()` (both public
//! methods `mipidsi` exposes for exactly this kind of "hand the
//! peripheral back" use case) to recover the raw `SpiDeviceDriver` + DC
//! `PinDriver`, and never touches `mipidsi`'s `Display`/`Interface`
//! abstraction again. [`St7789Surface::flush`] then:
//! 1. Sends `WriteMemoryStart` (RAMWR, DCS `0x2C`) directly.
//! 2. Converts the whole framebuffer to panel-byte-order (big-endian)
//!    bytes via [`bhk_core::platform::FrameBuffer565::write_be_bytes`]
//!    into a persistent, pre-allocated `FRAME_BYTES`-sized buffer (a
//!    single tight per-pixel loop, no `Point` computation, no
//!    `Interface`/`InterfacePixelFormat` dispatch).
//! 3. Writes that whole buffer in **one** `SpiDevice::write` call.
//!
//! The `CASET`/`RASET` address-window command bytes ([`CASET_ARGS`],
//! [`RASET_ARGS`]) are sent **once**, in `new`, not per frame — this
//! adapter only ever draws the same full-panel rectangle, so the window
//! never needs to change. They are **hand-computed compile-time
//! constants**, not derived at runtime by `mipidsi` — see their doc
//! comments for the exact derivation (worked out by reading
//! `mipidsi-0.10.0`'s private `Display::set_address_window` and the
//! `SetColumnAddress`/`SetPageAddress` DCS wire encoding directly, since
//! that method isn't public and can't just be called). **This is
//! exactly the kind of change where a color/positioning bug would show
//! up on real hardware but not in `cargo build`** — hardware-verify
//! with a human watching the actual panel, not just that this compiles
//! and boots.
use bhk_core::platform::{DisplaySurface, FrameBuffer565};
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiDevice;
use esp_idf_hal::delay::Delay;
use esp_idf_hal::gpio::{AnyOutputPin, GpioError, Output, PinDriver};
use esp_idf_hal::prelude::*;
use esp_idf_hal::spi::{Dma, SpiDeviceDriver, SpiDriver};
use esp_idf_hal::sys::EspError;
use mipidsi::interface::{SpiError as MipidsiSpiError, SpiInterface};
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, ColorOrder, Orientation, Rotation};
use mipidsi::Builder;

use crate::board::board_config::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

type Spi = SpiDeviceDriver<'static, SpiDriver<'static>>;
type CtrlPin = PinDriver<'static, AnyOutputPin, Output>;
// No `type Interface = SpiInterface<'static, Spi, CtrlPin>` alias: it's
// only needed transiently inside `St7789Surface::new` (for
// `Builder::new(ST7789, interface)`, immediately `release()`d back into
// raw `Spi`/`CtrlPin` -- see the module doc's "Hot-path blit" section),
// with the concrete type inferred there rather than named, so a
// standalone alias would just be dead code.
type InterfaceError = MipidsiSpiError<esp_idf_hal::spi::SpiError, GpioError>;

/// Scratch buffer `SpiInterface` batches command/init data through.
///
/// **Only used transiently during `Builder::init()`** (bead
/// ai-bitwarden-hw-key-ego moved the per-frame pixel blit to a raw,
/// DMA-backed path that no longer goes through `SpiInterface` at all —
/// see the module doc's "Hot-path blit" section). `mipidsi`'s init
/// sequence never sends bulk pixel data (sleep-out/MADCTL/pixel-format/
/// inversion/etc. are all small, fixed-size DCS commands), so this can
/// stay small; it does not need to hold a frame's worth of data the way
/// it used to when it was also the per-frame transfer buffer.
const INIT_SCRATCH_BUFFER_LEN: usize = 256;

/// SPI clock for the display bus.
///
/// Bumped 20 -> 40 MHz (bead ai-bitwarden-hw-key-ego: refresh rate was
/// visibly poor on real hardware — the previous 20MHz was an untested,
/// deliberately conservative guess with a TODO to raise once verified).
/// 40MHz is not a guess either: it's LilyGo's own factory-validated
/// value for this exact panel/wiring, confirmed by reading
/// `github.com/Xinyuan-LilyGO/T-Embed-CC1101`'s
/// `lib/TFT_eSPI/User_Setups/Setup214_LilyGo_T_Embed_PN532.h`, which
/// sets `SPI_FREQUENCY 40000000` (with a *commented-out* `27000000`
/// fallback, i.e. LilyGo themselves moved past 27MHz to 40MHz for
/// writes; `SPI_READ_FREQUENCY` stays at 20MHz, but this driver is
/// write-only and never reads from the panel). Measured on real
/// hardware (bead ego) to barely move the needle on its own — DMA and
/// the raw-blit rewrite (see module doc) were the real fix — but this
/// is still correct/validated and worth keeping.
const SPI_BAUDRATE_MHZ: u32 = 40;

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
/// flash, per bead ai-bitwarden-hw-key-c6e. Hardware-verified: with this
/// fix, boot reaches a stable "Entering main loop" with no panic/reboot
/// loop.
const NATIVE_WIDTH: u16 = 170;
const NATIVE_HEIGHT: u16 = 320;

/// Total bytes in one full-panel Rgb565 frame (320 * 170 * 2 =
/// 108,800). Used as the size of [`St7789Surface`]'s persistent
/// raw-bytes scratch buffer. **Not** used directly as the DMA
/// `max_transfer_sz` — see [`DMA_CHUNK_BYTES`] for why a single
/// transaction can't be this big.
const FRAME_BYTES: usize = DISPLAY_WIDTH as usize * DISPLAY_HEIGHT as usize * 2;

/// The DMA channel's `max_transfer_sz`, and therefore the largest single
/// SPI transaction `esp_idf_hal::spi::SpiDeviceDriver::write` will issue
/// per chunk (its own inherent `write()` internally splits any buffer
/// larger than this into multiple chunk-sized transactions — confirmed
/// by reading `esp-idf-hal-0.45.2`'s `src/spi.rs`).
///
/// **Hardware-confirmed constraint, not a config choice**: a first
/// attempt set this to the full `FRAME_BYTES` (108,800), aiming for
/// exactly one transaction per frame, and it failed on every single
/// frame with `E spi_master: check_trans_valid(1083): txdata transfer >
/// hardware max supported len` — the screen silently stopped updating
/// entirely (`bhk_core::run`'s loop drops `flush` errors), which is
/// worse than the pre-DMA baseline, not better. Traced to ESP-IDF's own
/// source (`components/hal/esp32s3/include/hal/spi_ll.h`):
/// `SPI_LL_DMA_MAX_BIT_LEN = 1 << 18` bits = 32,768 bytes — a genuine
/// SPI *hardware register width* limit on this chip for a single DMA
/// transaction's length field, enforced in
/// `components/esp_driver_spi/src/gpspi/spi_master.c`'s
/// `check_trans_valid`, regardless of how large `max_transfer_sz` (a
/// separate, buffer-*allocation*-size config knob) is set to. This is
/// NOT something `Dma::Auto(size)` can override by picking a bigger
/// `size` — 32,768 bytes is the hard ceiling for this SoC.
///
/// `FRAME_BYTES / 4 = 27,200` bytes (a clean 4-way split, comfortably
/// under the 32,768 cap, and a multiple of 4 as `Dma::max_transfer_size`
/// requires) — still a massive reduction from the pre-DMA baseline's
/// ~64-byte-capped FIFO-only transactions (bead ego's original
/// diagnosis), just not the single-transaction ideal originally aimed
/// for.
const DMA_CHUNK_BYTES: usize = FRAME_BYTES / 4;

/// `CASET` (Set Column Address, DCS `0x2A`) argument bytes for this
/// panel's full-frame address window: `[start_hi, start_lo, end_hi,
/// end_lo]`, big-endian, per `mipidsi-0.10.0`'s
/// `dcs::SetColumnAddress::fill_params_buf`.
///
/// **Hand-derived**, not `mipidsi`-computed (its `Display::
/// set_address_window` is a private method — this exact computation is
/// what it does internally, replicated here since we bypass it for the
/// hot path; see the module doc). For this board's fixed configuration
/// (`display_offset(35, 0)`, `Rotation::Deg270`, `display_size`
/// (native) `(170, 320)`), reading `mipidsi`'s `Display::
/// set_address_window`:
/// - `MemoryMapping::from_orientation(Deg270, mirrored=false)` gives
///   `reverse_rows=true`, `reverse_columns=false`,
///   `swap_rows_and_columns=true` (`Deg270.is_vertical()`).
/// - Starting `offset = display_offset = (35, 0)`.
/// - `reverse_columns` is false: `offset.0` unchanged (35).
/// - `reverse_rows` is true: `offset.1 = FRAMEBUFFER_SIZE.1 -
///   (display_size.1 + offset.1) = 320 - (320 + 0) = 0`.
/// - `swap_rows_and_columns` is true: `offset = (offset.1, offset.0) =
///   (0, 35)`.
/// - `flush`'s full-frame call is `set_pixels(sx=0, sy=0,
///   ex=DISPLAY_WIDTH-1=319, ey=DISPLAY_HEIGHT-1=169, ...)`; adding the
///   final offset `(0, 35)`: `(sx, sy, ex, ey) = (0, 35, 319, 204)`.
/// - `ST7789::update_address_window` (the default `Model` impl, no
///   override in `models/st7789.rs`) sends `CASET` from `(sx, ex)` and
///   `RASET` from `(sy, ey)` — always this fixed axis assignment; only
///   the *offset* gets swapped above, not which of sx/ex vs sy/ey maps
///   to which DCS command. (This looks like it should be impossible —
///   CASET's `ex=319` exceeds the ST7789's native 240-wide GRAM axis —
///   until you account for MADCTL's "MV" row/column-exchange bit, which
///   `Deg270`'s orientation sets during init: with MV=1 the controller
///   itself reinterprets CASET as walking the physical 320-long axis
///   and RASET the physical ~240 axis, which is exactly why a "vertical"
///   rotation is called that.)
///
/// So: CASET args for `(sx=0, ex=319)` = `0x0000` / `0x013F`.
const CASET_ARGS: [u8; 4] = [0x00, 0x00, 0x01, 0x3F];

/// `RASET` (Set Page Address, DCS `0x2B`) argument bytes for this
/// panel's full-frame address window — see [`CASET_ARGS`]'s doc comment
/// for the full derivation. `(sy=35, ey=204)` = `0x0023` / `0x00CC`.
const RASET_ARGS: [u8; 4] = [0x00, 0x23, 0x00, 0xCC];

/// DCS instruction bytes this module sends directly (bypassing
/// `mipidsi::dcs`'s typed command structs, which are private/internal to
/// that crate's own dispatch): `CASET`, `RASET`, and `RAMWR`
/// (`WriteMemoryStart`), all confirmed against `mipidsi-0.10.0`'s
/// `src/dcs/set_column_address.rs`, `set_page_address.rs`, and the
/// `dcs_basic_command!(WriteMemoryStart, 0x2C)` invocation in
/// `src/dcs.rs`.
mod dcs_command {
    pub const CASET: u8 = 0x2A;
    pub const RASET: u8 = 0x2B;
    pub const RAMWR: u8 = 0x2C;
}

/// Sends `command` (DC low) then `args` (DC high) — the same two-write
/// DCS command shape `mipidsi::interface::SpiInterface::send_command`
/// uses internally, replicated here since this module talks to the raw
/// `Spi`/`CtrlPin` directly (see the module doc's "Hot-path blit"
/// section). A single helper so every command send in this file goes
/// through the same, obviously-correct DC-toggle sequence.
///
/// Calls the `embedded_hal` trait methods explicitly (fully-qualified),
/// not `spi.write(...)`/`dc.set_low()`/`set_high()` directly: both
/// `SpiDeviceDriver` and `PinDriver` also have INHERENT methods of the
/// same name returning `EspError` (esp-idf-hal's own, non-`embedded_hal`
/// API), which Rust's method resolution prefers over a trait method of
/// the same name — calling them directly would silently return the
/// wrong error type instead of the `InterfaceError`
/// (`MipidsiSpiError<esp_idf_hal::spi::SpiError, GpioError>`) this
/// module's error types are built from.
fn send_command(spi: &mut Spi, dc: &mut CtrlPin, command: u8, args: &[u8]) -> Result<(), InterfaceError> {
    OutputPin::set_low(dc).map_err(InterfaceError::Dc)?;
    SpiDevice::write(spi, &[command]).map_err(InterfaceError::Spi)?;
    OutputPin::set_high(dc).map_err(InterfaceError::Dc)?;
    SpiDevice::write(spi, args).map_err(InterfaceError::Spi)
}

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
    ///
    /// The reset-pin error type is `core::convert::Infallible`, not
    /// `GpioError`: this board has no LCD hardware reset pin, so `mipidsi`
    /// uses its `NoResetPin` marker type, whose `OutputPin` impl can
    /// never fail.
    Display(mipidsi::InitError<InterfaceError, core::convert::Infallible>),
    /// Sending the one-time `CASET`/`RASET` address-window commands
    /// (after `mipidsi` init, before entering the raw per-frame blit
    /// path — see the module doc) failed.
    AddressWindow(InterfaceError),
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
///
/// Owns the raw SPI + DC handles directly (not a `mipidsi::Display`) —
/// see the module doc's "Hot-path blit" section for why.
pub struct St7789Surface {
    spi: Spi,
    dc: CtrlPin,
    /// Persistent, pre-allocated scratch buffer for
    /// [`FrameBuffer565::write_be_bytes`]'s output — reused every frame
    /// rather than allocated fresh, since its size never changes.
    frame_bytes: Vec<u8>,
    backlight: CtrlPin,
    /// Kept alive for as long as the display is: dropping this would
    /// (per T-Embed-CC1101's own `factory.cpp`, which drives
    /// `BOARD_PWR_EN` low on shutdown) cut power to the panel. Never read
    /// after `new`, hence `#[allow(dead_code)]`-shaped usage — its job is
    /// existing, not being called.
    #[allow(dead_code)]
    _peripheral_power: CtrlPin,
}

impl St7789Surface {
    /// Powers on the T-Embed's shared peripheral rail, brings up the SPI
    /// bus (with DMA enabled — see the module doc), initializes the
    /// ST7789 via `mipidsi`, sends the one-time `CASET`/`RASET` address
    /// window, and turns on the backlight.
    ///
    /// # Errors
    ///
    /// Returns [`St7789SurfaceInitError`] if any GPIO/SPI peripheral
    /// fails to configure, if the panel doesn't respond to `mipidsi`'s
    /// init sequence the way the generic ST7789 model expects, or if the
    /// post-init address-window commands fail to send.
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
        backlight: AnyOutputPin,
        peripheral_power_on: AnyOutputPin,
    ) -> Result<Self, St7789SurfaceInitError> {
        let mut peripheral_power = PinDriver::output(peripheral_power_on)?;
        // Per T-Embed-CC1101's own `factory.cpp` `setup()`: `BOARD_PWR_EN`
        // must be driven high before `board_spi_init_shared_bus()` or the
        // panel's shared power rail is off entirely. The settle delay
        // before touching SPI is a guess (untested) — long enough to be
        // safe on a busy-wait, not derived from a rail spec sheet we
        // don't have.
        peripheral_power.set_high()?;
        Delay::new_default().delay_ms(10);

        // DMA enabled (bead ai-bitwarden-hw-key-ego): see the module
        // doc's "Hot-path blit" section for why this alone was a large
        // part of the fix (without it, ESP-IDF's SPI master driver caps
        // a single hardware transaction at 64 bytes on this chip,
        // silently re-chunking any larger `write()`). `max_transfer_sz`
        // is `DMA_CHUNK_BYTES`, not the full `FRAME_BYTES` -- see that
        // constant's doc comment for the hardware register-width ceiling
        // (32,768 bytes on this chip) that makes a single whole-frame
        // transaction impossible, discovered when a first attempt at
        // exactly that silently broke every `flush` on real hardware.
        let driver_config = esp_idf_hal::spi::config::DriverConfig::new().dma(Dma::Auto(DMA_CHUNK_BYTES));
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
        let mut backlight_pin = PinDriver::output(backlight)?;

        // `SpiInterface` needs a `&'static mut [u8]` scratch buffer, and
        // borrowing it from a field of `Self` would make this struct
        // self-referential (not expressible in safe Rust without extra
        // indirection). `Box::leak` is the standard, deliberate way
        // around that for a peripheral meant to live for the entire
        // program: it trades a one-time, bounded heap allocation that's
        // never freed for avoiding a self-referential struct or an
        // `unsafe` pinned-borrow workaround. No `unsafe` involved. Only
        // `INIT_SCRATCH_BUFFER_LEN` (256 bytes) now, not a frame's worth
        // — see that constant's doc comment for why.
        let spi_buffer: &'static mut [u8] = Box::leak(Box::new([0u8; INIT_SCRATCH_BUFFER_LEN]));
        let interface = SpiInterface::new(spi_device, dc_pin, spi_buffer);

        let mut delay = Delay::new_default();
        let display = Builder::new(ST7789, interface)
            // No `.reset_pin(...)` call: this board has no LCD hardware
            // reset line (`TFT_RST = -1` in LilyGo's own
            // `Setup214_LilyGo_T_Embed_PN532.h`). With no reset pin
            // configured, `mipidsi`'s `Builder::init` sends a
            // `SoftReset` DCS command instead of toggling a GPIO — this
            // is the intended, supported "no hardware reset" path, not a
            // workaround.
            .display_size(NATIVE_WIDTH, NATIVE_HEIGHT)
            // Column/row start offset for this exact 170-wide ST7789
            // panel: the T-Embed-CC1101 repo's vendored
            // `lib/TFT_eSPI/TFT_Drivers/ST7789_Rotation.h` `setRotation()`
            // table gives different `colstart`/`rowstart` per TFT_eSPI
            // rotation index because TFT_eSPI pre-computes the offset
            // already remapped into each rotation's register frame.
            // `mipidsi` instead wants the offset in the NATIVE
            // (rotation-0/portrait) frame and remaps it itself at address-
            // window time — so the right source row is TFT_eSPI's
            // rotation index **0** (Portrait), not 1/3 (Landscape): for
            // `_init_width == 170` that's `colstart = 35, rowstart = 0`,
            // i.e. `display_offset(35, 0)`. HARDWARE-CONFIRMED CORRECT on
            // real T-Embed CC1101: fills the panel exactly, no
            // cropping/blank margin. (See `CASET_ARGS`'s doc comment for
            // how this feeds into the hand-derived address-window bytes
            // this module now sends directly.)
            .display_offset(35, 0)
            .color_order(ColorOrder::Rgb)
            // Required for this panel: LilyGo's own T-Embed-CC1101
            // `Setup214_LilyGo_T_Embed_PN532.h` sets `TFT_INVERSION_ON`.
            // HARDWARE-CONFIRMED CORRECT on real T-Embed CC1101: colors
            // match expectations with this set.
            .invert_colors(ColorInversion::Inverted)
            // `Rotation::Deg90` rendered the image upside down on real
            // hardware; `Deg270` is the other of the two 320x170
            // landscape choices. HARDWARE-CONFIRMED CORRECT on real
            // T-Embed CC1101: right-side up, matching the factory
            // firmware's orientation.
            .orientation(Orientation::new().rotate(Rotation::Deg270))
            .init(&mut delay)
            .map_err(St7789SurfaceInitError::Display)?;

        // Init is done; hand mipidsi's Display/Interface abstraction back
        // and take direct ownership of the raw SPI + DC pin for the
        // per-frame hot path (see the module doc's "Hot-path blit"
        // section for why). `release()` is a public, intended-for-this
        // "get the peripheral back" API on both types -- not a hack.
        let (interface, _model, _reset) = display.release();
        let (mut spi, mut dc) = interface.release();

        // One-time CASET/RASET address-window setup (see `CASET_ARGS`/
        // `RASET_ARGS`'s doc comments for the exact derivation). Sent
        // once here, not per frame, since this adapter only ever draws
        // the same full-panel rectangle -- `flush` only needs to resend
        // RAMWR + pixel data.
        send_command(&mut spi, &mut dc, dcs_command::CASET, &CASET_ARGS).map_err(St7789SurfaceInitError::AddressWindow)?;
        send_command(&mut spi, &mut dc, dcs_command::RASET, &RASET_ARGS).map_err(St7789SurfaceInitError::AddressWindow)?;

        backlight_pin.set_high()?;

        Ok(Self {
            spi,
            dc,
            frame_bytes: vec![0u8; FRAME_BYTES],
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

        // RAMWR: resets the controller's internal GRAM write pointer
        // back to the (CASET, RASET) window's start, then every
        // subsequent data byte (until the window's pixel count is
        // reached) is interpreted as pixel data. Resent every frame
        // (unlike CASET/RASET, sent once in `new`) since it's what
        // actually triggers "start writing pixels now". No args (per
        // `dcs_basic_command!(WriteMemoryStart, 0x2C)` in mipidsi's own
        // `src/dcs.rs`), so this leaves DC high afterward, ready for the
        // raw pixel-data write below.
        send_command(&mut self.spi, &mut self.dc, dcs_command::RAMWR, &[]).map_err(St7789SurfaceError::Spi)?;

        // Convert the whole frame to panel byte order (big-endian) in one
        // tight pass -- no `Point` computation, no `mipidsi::Interface`
        // dispatch (see the module doc's "Hot-path blit" section).
        framebuffer.write_be_bytes(&mut self.frame_bytes);

        // One logical call for the whole frame -- `SpiDeviceDriver::write`
        // (called here via the `SpiDevice` trait) internally splits any
        // buffer bigger than the configured DMA `max_transfer_sz`
        // (`DMA_CHUNK_BYTES`) into that many chunk-sized DMA
        // transactions itself (confirmed by reading
        // `esp-idf-hal-0.45.2`'s `src/spi.rs`), so this is a handful of
        // large DMA transactions per frame, not hundreds of tiny
        // FIFO-only ones -- this is the change bead ego measured render/
        // flush timing to target. Fully-qualified `SpiDevice::write` for
        // the same reason `send_command` is (see its doc comment):
        // `Spi`'s inherent `write()` would silently return the wrong
        // error type.
        SpiDevice::write(&mut self.spi, &self.frame_bytes).map_err(InterfaceError::Spi).map_err(St7789SurfaceError::Spi)
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
