use embedded_graphics::{
    geometry::Point,
    mono_font::{ascii::FONT_10X20, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    text::{Baseline, Text},
    Drawable,
};
use esp_idf_svc::{
    hal::{
        delay::FreeRtos,
        gpio::PinDriver,
        i2c::*,
        prelude::{Peripherals, *},
    },
    sys::EspError,
};
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};

fn main() -> Result<(), EspError> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world!");

    /*******************
     *   PERIPHERALS   *
     *******************/
    let peripherals = Peripherals::take().unwrap();

    // Built in LED
    let mut led = PinDriver::output(peripherals.pins.gpio13)?;

    led.set_high()?; // Indicate program start phase

    // OLED Display
    let i2c = peripherals.i2c1;
    let sda = peripherals.pins.gpio23;
    let scl = peripherals.pins.gpio22;

    log::info!("Connecting to OLED");

    let config = I2cConfig::new().baudrate(200.kHz().into());
    let i2c = I2cDriver::new(i2c, sda, scl, &config)?;

    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    display.init().unwrap();
    display.flush().unwrap();

    log::info!("Setup finished");

    /*******************
     *      BOOT       *
     *******************/
    // Splash screen
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(BinaryColor::On)
        .build();

    Text::with_baseline("Bitwarden", Point::zero(), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();

    FreeRtos::delay_ms(2000);
    display.clear_buffer();
    display.flush().unwrap();

    /*******************
     *    MAIN LOOP    *
     *******************/
    led.set_low()?; // Indicate program main loop phase
    loop {
        // Sleeping here to make sure the watchdog isn't triggered
        FreeRtos::delay_ms(500);
    }
}
