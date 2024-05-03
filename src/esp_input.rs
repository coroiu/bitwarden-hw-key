use button_driver::{Button, ButtonConfig, PinWrapper};
use esp_idf_hal::gpio::{Input, PinDriver};

use crate::gui::input::{InputEvent, InputInterface, KeyCode, KeyEvent};

pub struct EspInput {
    drivers: Vec<(KeyCode, Button<Box<dyn MyPinWrapper>>)>,
}

impl EspInput {
    pub fn new(drivers: Vec<(KeyCode, Box<dyn MyPinWrapper>)>) -> Self {
        // drivers.into_iter().for_each(|(_, pin)| {
        //     pin.set_pull(esp_idf_hal::gpio::Pull::Up);
        // });

        EspInput {
            drivers: drivers
                .into_iter()
                .map(|(key_code, pin)| {
                    (
                        key_code,
                        Button::new(
                            pin,
                            ButtonConfig {
                                // debounce: Duration::from_millis(100),
                                mode: button_driver::Mode::PullUp,
                                ..Default::default()
                            },
                        ),
                    )
                })
                .collect(),
        }
    }
}

impl InputInterface for EspInput {
    fn get_events(&mut self) -> Vec<InputEvent> {
        let mut events = Vec::new();

        for (key_code, driver) in self.drivers.iter_mut() {
            if driver.is_clicked() {
                events.push(InputEvent {
                    key_code: *key_code,
                    key_event: KeyEvent::Clicked,
                });
                driver.reset();
            } else if driver.is_double_clicked() {
                events.push(InputEvent {
                    key_code: *key_code,
                    key_event: KeyEvent::DoubleClicked,
                });
                driver.reset();
            } else if driver.is_triple_clicked() {
                events.push(InputEvent {
                    key_code: *key_code,
                    key_event: KeyEvent::TripleClicked,
                });
                driver.reset();
            } else if let Some(held_time) = driver.held_time() {
                events.push(InputEvent {
                    key_code: *key_code,
                    key_event: KeyEvent::LongPress(held_time),
                });
                driver.reset();
            }
        }

        events
    }

    fn update(&mut self) {
        for (_, driver) in self.drivers.iter_mut() {
            driver.tick();
        }
    }
}

pub type InputPinDriver<'a, P> = PinDriver<'a, P, Input>;

pub struct EspPinWrapper<'a, P: esp_idf_hal::gpio::Pin>(pub InputPinDriver<'a, P>);

// Used for erasing the type of the pin inside the driver
pub trait MyPinWrapper {
    fn is_high_wr(&self) -> bool;
    fn is_low_wr(&self) -> bool;
}

impl<'a, P: esp_idf_hal::gpio::Pin> MyPinWrapper for EspPinWrapper<'a, P> {
    fn is_high_wr(&self) -> bool {
        self.0.is_high()
    }

    fn is_low_wr(&self) -> bool {
        self.0.is_low()
    }
}

impl PinWrapper for Box<dyn MyPinWrapper> {
    fn is_high(&self) -> bool {
        self.is_high_wr()
    }

    fn is_low(&self) -> bool {
        self.is_low_wr()
    }
}
