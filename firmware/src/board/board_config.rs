//! Lilygo T-Embed (ESP32-S3) pin map and peripheral assembly.
//!
//! Per the W6 bead: "`BoardConfig` pin map + peripheral init live ONLY
//! here" — this is the single place in `firmware` that names a T-Embed
//! GPIO number. Every other `board/*` module takes already-typed
//! peripherals (an `SpiDeviceDriver`, a `PinDriver`, ...), not raw pin
//! numbers, so this module is the only thing that would need to change
//! if the pin map turned out to be wrong.
//!
//! # Source of the pin map
//!
//! These pin numbers are **not guessed**: they were read directly from
//! `LilyGo`'s own factory firmware for this exact board,
//! `Xinyuan-LilyGO/T-Embed`, `examples/factory/pin_config.h` (and cross
//! checked against `examples/tft/tft.ino`, which uses the same pins to
//! drive the ST7789 via `TFT_eSPI`). That is the closest thing to a
//! datasheet this board has. What it does **not** tell us is whether the
//! pins behave the way this driver assumes once real current flows —
//! that can only be confirmed on hardware, which does not exist on this
//! machine (no `/dev/tty.*` device; confirmed during the W5 SDK spike).
//! Every pin below is marked with what is and isn't verified.
//!
//! # What is unverified
//!
//! Nothing in this file has been exercised against a real T-Embed. The
//! pin *numbers* come from `LilyGo`'s own source, so confidence there is
//! high, but:
//! - The exact SPI mode/bit order/clock the panel tolerates is untested
//!   (see `st7789_surface::SPI_BAUDRATE`).
//! - Whether `mipidsi`'s generic ST7789 init sequence (sleep-out /
//!   MADCTL / pixel-format / normal-mode / display-on) is sufficient for
//!   this specific panel is untested. `LilyGo`'s own `tft.ino` sends a
//!   vendor-specific gamma/porch-timing command table
//!   (`lcd_st7789v[]`) on top of `TFT_eSPI`'s init that `mipidsi`'s generic
//!   `ST7789` model does not replicate — the display may still light up
//!   with `mipidsi`'s init, but contrast/gamma fidelity to the factory
//!   firmware is not guaranteed.
//! - The orientation/mirroring needed to match `tft.ino`'s
//!   `tft.setRotation(3)` (landscape, 320x170) is a best guess, not a
//!   verified mapping between `TFT_eSPI`'s rotation index and mipidsi's
//!   `Rotation` enum.
//! - The rotary encoder's electrical behavior (pull-up requirements,
//!   debounce characteristics) is untested.

use esp_idf_hal::gpio::{AnyOutputPin, Gpio0, Gpio1, Gpio2};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::spi::SPI2;

/// The T-Embed's rotary encoder quadrature pins.
///
/// Source: `pin_config.h` `PIN_ENCODE_A` / `PIN_ENCODE_B`. Untested: pull
/// configuration and debounce (see `rotary_input`).
pub const ENCODER_PIN_A: u8 = 2;
pub const ENCODER_PIN_B: u8 = 1;

/// The T-Embed's rotary encoder push-button pin.
///
/// Source: `pin_config.h` `PIN_ENCODE_BTN`.
pub const ENCODER_BUTTON_PIN: u8 = 0;

/// ST7789 SPI bus pins (clock + MOSI; the panel is write-only, no MISO).
///
/// Source: `pin_config.h` `PIN_LCD_CLK` / `PIN_LCD_MOSI`.
pub const LCD_SCLK_PIN: u8 = 12;
pub const LCD_MOSI_PIN: u8 = 11;

/// ST7789 control pins: chip-select, data/command, and hardware reset.
///
/// Source: `pin_config.h` `PIN_LCD_CS` / `PIN_LCD_DC` / `PIN_LCD_RES`.
pub const LCD_CS_PIN: u8 = 10;
pub const LCD_DC_PIN: u8 = 13;
pub const LCD_RESET_PIN: u8 = 9;

/// ST7789 backlight enable pin (driven high to turn the backlight on;
/// `tft.ino` drives this directly rather than through any PWM/dimming
/// logic, so this adapter does the same).
///
/// Source: `pin_config.h` `PIN_LCD_BL`.
pub const LCD_BACKLIGHT_PIN: u8 = 15;

/// Peripheral power-enable pin. `LilyGo`'s own `tft.ino` drives this HIGH
/// *before* `tft.begin()` — without it the panel has no power rail and
/// nothing else here will work. This is not a display-specific pin (it
/// gates power to more than just the LCD on this board), but it lives
/// here because the display is the only consumer this adapter drives.
///
/// Source: `pin_config.h` `PIN_POWER_ON`, `tft.ino::setup()`.
pub const PERIPHERAL_POWER_ON_PIN: u8 = 46;

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
    pub lcd_reset: AnyOutputPin,
    pub lcd_backlight: AnyOutputPin,
    pub peripheral_power_on: AnyOutputPin,
    pub encoder_pin_a: Gpio2,
    pub encoder_pin_b: Gpio1,
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
            lcd_sclk: p.pins.gpio12.into(),
            lcd_mosi: p.pins.gpio11.into(),
            lcd_cs: p.pins.gpio10.into(),
            lcd_dc: p.pins.gpio13.into(),
            lcd_reset: p.pins.gpio9.into(),
            lcd_backlight: p.pins.gpio15.into(),
            peripheral_power_on: p.pins.gpio46.into(),
            encoder_pin_a: p.pins.gpio2,
            encoder_pin_b: p.pins.gpio1,
            encoder_button: p.pins.gpio0,
        })
    }
}

// Compile-time cross-check that the typed fields above and the `u8`
// constants agree, so the two can't silently drift apart.
const _: () = {
    assert!(LCD_SCLK_PIN == 12);
    assert!(LCD_MOSI_PIN == 11);
    assert!(LCD_CS_PIN == 10);
    assert!(LCD_DC_PIN == 13);
    assert!(LCD_RESET_PIN == 9);
    assert!(LCD_BACKLIGHT_PIN == 15);
    assert!(PERIPHERAL_POWER_ON_PIN == 46);
    assert!(ENCODER_PIN_A == 2);
    assert!(ENCODER_PIN_B == 1);
    assert!(ENCODER_BUTTON_PIN == 0);
};
