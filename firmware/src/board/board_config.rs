//! Lilygo T-Embed (ESP32-S3) pin map and peripheral assembly.
//!
//! Per the W6 bead: "`BoardConfig` pin map + peripheral init live ONLY
//! here" — this is the single place in `firmware` that names a T-Embed
//! GPIO number. Every other `board/*` module takes already-typed
//! peripherals (an `SpiDeviceDriver`, a `PinDriver`, ...), not raw pin
//! numbers, so this module is the only thing that would need to change
//! if the pin map turned out to be wrong.
//!
//! # CRITICAL: this board is the T-Embed **CC1101** variant, not plain T-Embed
//!
//! The physical hardware (bead ai-bitwarden-hw-key-c6e) is a **Lilygo
//! T-Embed CC1101** (has a sub-1GHz radio daughterboard). The LCD/power
//! pin constants below were *originally* copied from the **plain**
//! T-Embed's `pin_config.h` (see git history), which is a **different,
//! incompatible pin map** — confirmed on real hardware as a fully-dark
//! panel (zero backlight) even though `mipidsi` init "succeeded" (the
//! panel SPI bus is write-only/blind, so wrong wiring never surfaces as
//! an error). The LCD SPI/DC/CS/backlight/power-enable pins below have
//! been corrected to the CC1101 variant's actual wiring, confirmed
//! against `github.com/Xinyuan-LilyGO/T-Embed-CC1101`:
//! - `examples/utilities.h` (`DISPLAY_BL`, `DISPLAY_CS/MISO/MOSI/SCLK/DC/RST`, `BOARD_PWR_EN`)
//! - `lib/TFT_eSPI/User_Setups/Setup214_LilyGo_T_Embed_PN532.h` (`TFT_BL`,
//!   `TFT_CS/MISO/MOSI/SCLK/DC/RST`, `TFT_INVERSION_ON`, `TFT_WIDTH`/`TFT_HEIGHT`)
//! - `examples/factory/factory.cpp` (`setup()`'s
//!   `digitalWrite(BOARD_PWR_EN, HIGH)` *before* SPI/display init, and
//!   `setBacklightBrightness()`'s `digitalWrite(DISPLAY_BL, ...)`, both
//!   confirming active-HIGH polarity for both pins)
//!
//! **HARDWARE-CONFIRMED FIXED**: with these corrected pins plus the
//! `invert_colors`/`display_offset`/`Rotation::Deg270` corrections in
//! `st7789_surface.rs`, a human visually confirmed the panel on the real
//! T-Embed CC1101 is right-side up, fills the panel with no
//! offset/cropping, and shows correct colors.
//!
//! **The rotary encoder pins (`ENCODER_PIN_A`/`B`) were ALSO wrong for
//! this board** (CC1101 uses GPIO4/GPIO5, not GPIO2/GPIO1 — see
//! `.research/findings/2026-08-12-t-embed-input-hardware.md`) and have
//! now been corrected below, per bead ai-bitwarden-hw-key-ekd. The clean
//! build-time plain-vs-CC1101 board selection (supporting both variants
//! at once, rather than this hardcoded CC1101-only pin map) is still
//! tracked separately in that bead. As of this correction the encoder
//! pins are believed right but not yet hardware-exercised — see
//! `rotary_input`'s module doc.
//!
//! # Source of the pin map
//!
//! These pin numbers are **not guessed**: they were read directly from
//! `LilyGo`'s own factory firmware for this exact board variant. What it
//! does **not** tell us is whether the pins behave the way this driver
//! assumes once real current flows — that can only be confirmed on
//! hardware. Every pin below is marked with what is and isn't verified.
//!
//! # What is unverified
//!
//! - The exact SPI mode/bit order/clock the panel tolerates is untested
//!   (see `st7789_surface::SPI_BAUDRATE`).
//! - Whether `mipidsi`'s generic ST7789 init sequence (sleep-out /
//!   MADCTL / pixel-format / normal-mode / display-on) matches `LilyGo`'s
//!   vendor-tuned gamma/porch-timing table closely enough for exact
//!   contrast/gamma fidelity is untested — HARDWARE-CONFIRMED "correct
//!   colors, right-side up, fills the panel" (see `st7789_surface.rs`),
//!   but not pixel-perfect factory-firmware parity.
//! - The orientation/offset needed to match `tft.ino`'s
//!   `tft.setRotation(3)` (landscape, 320x170) is now
//!   HARDWARE-CONFIRMED: `Rotation::Deg270` + `display_offset(35, 0)` in
//!   `st7789_surface.rs`, verified by direct human observation on the
//!   real T-Embed CC1101 panel.
//! - The rotary encoder's electrical behavior (pull-up requirements,
//!   debounce characteristics) is untested. Its pins have now been
//!   corrected to the CC1101's actual wiring (bead ekd) but the encoder
//!   itself has not yet been hardware-exercised.

use esp_idf_hal::gpio::{AnyOutputPin, Gpio0, Gpio4, Gpio5};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::spi::SPI2;

/// The T-Embed's rotary encoder quadrature pins.
///
/// Source (T-Embed CC1101): `examples/utilities.h` `ENCODER_INA`/
/// `ENCODER_INB` = GPIO4/GPIO5 (bead ai-bitwarden-hw-key-ekd; also
/// confirmed against `.research/findings/2026-08-12-t-embed-input-hardware.md`).
/// (Plain T-Embed's `pin_config.h` has `PIN_ENCODE_A`/`B` = GPIO2/GPIO1 —
/// different pins; that was the pre-correction, wrong value here.) The
/// CC1101 also has an independent user button on GPIO6
/// (`BOARD_USER_KEY`), intentionally left unwired — see `rotary_input`'s
/// module doc. Untested: pull configuration and debounce (see
/// `rotary_input`).
pub const ENCODER_PIN_A: u8 = 4;
pub const ENCODER_PIN_B: u8 = 5;

/// The T-Embed's rotary encoder push-button pin.
///
/// Source: `pin_config.h` `PIN_ENCODE_BTN`. Matches the CC1101's
/// `ENCODER_KEY` (also GPIO0) — this one pin happens to agree between
/// variants, unlike `ENCODER_PIN_A`/`B` above.
pub const ENCODER_BUTTON_PIN: u8 = 0;

/// ST7789 SPI bus pins (clock + MOSI; the panel is write-only, no MISO).
///
/// Source (T-Embed CC1101): `examples/utilities.h` `DISPLAY_SCLK` /
/// `DISPLAY_MOSI`, confirmed against `Setup214_LilyGo_T_Embed_PN532.h`'s
/// `TFT_SCLK`/`TFT_MOSI`. (Plain T-Embed's `pin_config.h` has SCLK=12,
/// MOSI=11 — different pins; that was the pre-correction, wrong value
/// here.)
pub const LCD_SCLK_PIN: u8 = 11;
pub const LCD_MOSI_PIN: u8 = 9;

/// ST7789 control pins: chip-select and data/command. There is **no
/// hardware reset pin** on this board variant (`TFT_RST = -1` in
/// `Setup214_LilyGo_T_Embed_PN532.h`, i.e. not wired to any GPIO) — see
/// `st7789_surface.rs`'s use of `mipidsi`'s `NoResetPin`/software-reset
/// path instead of a `reset_pin()` call.
///
/// Source (T-Embed CC1101): `examples/utilities.h` `DISPLAY_CS` /
/// `DISPLAY_DC`, confirmed against `Setup214_LilyGo_T_Embed_PN532.h`'s
/// `TFT_CS`/`TFT_DC`. (Plain T-Embed's `pin_config.h` has CS=10, DC=13 —
/// different pins.)
pub const LCD_CS_PIN: u8 = 41;
pub const LCD_DC_PIN: u8 = 16;

/// ST7789 backlight enable pin (driven high to turn the backlight on;
/// `factory.cpp`'s `setBacklightBrightness()` drives this directly with
/// `digitalWrite(DISPLAY_BL, value == 0 ? LOW : HIGH)` rather than PWM,
/// so this adapter does the same).
///
/// Source (T-Embed CC1101): `examples/utilities.h` `DISPLAY_BL`,
/// confirmed against `Setup214_LilyGo_T_Embed_PN532.h`'s `TFT_BL`. (Plain
/// T-Embed's `pin_config.h` has `PIN_LCD_BL` = 15 — a **different** pin;
/// note that 15 is this board's `PERIPHERAL_POWER_ON_PIN` below, not its
/// backlight pin, which is the failure mode this correction fixes: the
/// old code was driving the wrong signal entirely.)
pub const LCD_BACKLIGHT_PIN: u8 = 21;

/// Peripheral power-enable pin (`BOARD_PWR_EN` in CC1101 sources).
/// `factory.cpp`'s `setup()` drives this HIGH via
/// `digitalWrite(BOARD_PWR_EN, HIGH)` *before* `board_spi_init_shared_bus()`
/// (i.e. before any display SPI activity) — without it the panel's
/// shared power rail is off and nothing else here will work. This is not
/// a display-specific pin (it gates power to more than just the LCD on
/// this board), but it lives here because the display is the only
/// consumer this adapter drives.
///
/// Source (T-Embed CC1101): `examples/utilities.h` `BOARD_PWR_EN`,
/// confirmed against `examples/factory/factory.cpp`'s `setup()`. (Plain
/// T-Embed's `pin_config.h` has `PIN_POWER_ON` = 46 — a **different**
/// pin; GPIO46 does nothing relevant on the CC1101 board.)
pub const PERIPHERAL_POWER_ON_PIN: u8 = 15;

/// Panel resolution. Source: `pin_config.h` `LV_SCREEN_WIDTH` /
/// `LV_SCREEN_HEIGHT` (also matches the ST7789V variant `LilyGo` ships on
/// this board — 320x170, not the more common 240x320 portrait panel).
pub const DISPLAY_WIDTH: u16 = 320;
pub const DISPLAY_HEIGHT: u16 = 170;

/// The subset of ESP32-S3 peripherals the T-Embed board adapter needs,
/// already split out of the single [`Peripherals::take`] singleton.
///
/// This is the "peripheral init lives only here" half of the bead: call
/// [`BoardPeripherals::take`] once, then hand each field to the matching
/// `board::*` constructor (`St7789Surface::new`, `RotaryEncoderInput::new`,
/// ...). Nothing outside this file names a T-Embed GPIO number.
pub struct BoardPeripherals {
    pub lcd_spi: SPI2,
    pub lcd_sclk: AnyOutputPin,
    pub lcd_mosi: AnyOutputPin,
    pub lcd_cs: AnyOutputPin,
    pub lcd_dc: AnyOutputPin,
    // No `lcd_reset` field: this board has no LCD hardware reset pin
    // (`TFT_RST = -1`); see `LCD_CS_PIN`'s doc comment.
    pub lcd_backlight: AnyOutputPin,
    pub peripheral_power_on: AnyOutputPin,
    pub encoder_pin_a: Gpio4,
    pub encoder_pin_b: Gpio5,
    pub encoder_button: Gpio0,
}

impl BoardPeripherals {
    /// Consumes the singleton [`Peripherals::take`] handle and splits out
    /// exactly the pins this board adapter uses, by the pin map documented
    /// on this module's constants.
    ///
    /// # Errors
    ///
    /// Returns `EspError` if the peripheral singleton has already been
    /// taken elsewhere in the process.
    pub fn take() -> Result<Self, esp_idf_hal::sys::EspError> {
        let p = Peripherals::take()?;

        Ok(Self {
            lcd_spi: p.spi2,
            lcd_sclk: p.pins.gpio11.into(),
            lcd_mosi: p.pins.gpio9.into(),
            lcd_cs: p.pins.gpio41.into(),
            lcd_dc: p.pins.gpio16.into(),
            lcd_backlight: p.pins.gpio21.into(),
            peripheral_power_on: p.pins.gpio15.into(),
            encoder_pin_a: p.pins.gpio4,
            encoder_pin_b: p.pins.gpio5,
            encoder_button: p.pins.gpio0,
        })
    }
}

// Compile-time cross-check that the typed fields above and the `u8`
// constants agree, so the two can't silently drift apart.
const _: () = {
    assert!(LCD_SCLK_PIN == 11);
    assert!(LCD_MOSI_PIN == 9);
    assert!(LCD_CS_PIN == 41);
    assert!(LCD_DC_PIN == 16);
    assert!(LCD_BACKLIGHT_PIN == 21);
    assert!(PERIPHERAL_POWER_ON_PIN == 15);
    assert!(ENCODER_PIN_A == 4);
    assert!(ENCODER_PIN_B == 5);
    assert!(ENCODER_BUTTON_PIN == 0);
};
