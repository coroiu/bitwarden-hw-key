mod esp_input;
mod gui;
mod time;
mod view;

use std::time::Duration;

use embedded_graphics::{geometry::Point, image::Image, Drawable};
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

use crate::{
    esp_input::{EspInput, EspPinWrapper},
    gui::icons::BITWARDEN_LOGO,
    time::timer::Timer,
    view::create_view,
};

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
    Image::new(&BITWARDEN_LOGO.as_image_raw(), Point::new(0, 3))
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();

    FreeRtos::delay_ms(1000);
    display.clear_buffer();
    display.flush().unwrap();

    /*******************
     *    MAIN LOOP    *
     *******************/
    led.set_low()?; // Indicate program main loop phase

    /*******************
     *   SETUP VIEWS   *
     *******************/

    let mut pin_driver_15 = PinDriver::input(peripherals.pins.gpio15)?;
    pin_driver_15.set_pull(esp_idf_hal::gpio::Pull::Up)?;
    let mut pin_driver_32 = PinDriver::input(peripherals.pins.gpio32)?;
    pin_driver_32.set_pull(esp_idf_hal::gpio::Pull::Up)?;
    let mut pin_driver_14 = PinDriver::input(peripherals.pins.gpio14)?;
    pin_driver_14.set_pull(esp_idf_hal::gpio::Pull::Up)?;

    let input = Box::new(EspInput::new(vec![
        (
            gui::input::KeyCode::Up,
            Box::new(EspPinWrapper(pin_driver_15)),
        ),
        (
            gui::input::KeyCode::Middle,
            Box::new(EspPinWrapper(pin_driver_32)),
        ),
        (
            gui::input::KeyCode::Down,
            Box::new(EspPinWrapper(pin_driver_14)),
        ),
    ]));

    let mut document = create_view(128, 32, input);

    let mut update_timer = Timer::new(Duration::from_millis(25), true);
    let mut draw_timer = Timer::new(Duration::from_millis(50), true);

    update_timer.start();
    draw_timer.start();

    let mut turn_off_timer = Timer::new(Duration::from_secs(30), false);
    turn_off_timer.start();

    let mut debug_timer = Timer::new(Duration::from_millis(200), true);
    debug_timer.start();

    loop {
        document.update_input();

        // if debug_timer.run() {
        //     log::info!(
        //         "Pin 15 {:?}, pin 32 {:?}, pin 14 {:?}",
        //         pin_driver_15.get_level(),
        //         pin_driver_32.get_level(),
        //         pin_driver_14.get_level()
        //     );
        // }

        if update_timer.run() {
            document.update();
        }

        if draw_timer.run() {
            let canvas = document.draw();
            canvas.draw(&mut display).unwrap();
            display.flush().unwrap();
        }

        if turn_off_timer.run() {
            break;
        }

        // Sleeping here to make sure the watchdog isn't triggered
        // There is probably a better way to do this
        FreeRtos::delay_ms(1);
    }

    log::info!("Test draw finished");

    log::info!("Clearing display");
    display.clear_buffer();
    display.flush().unwrap();

    loop {
        FreeRtos::delay_ms(500);
    }
}
