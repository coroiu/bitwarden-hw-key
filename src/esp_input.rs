use std::time::Duration;

use button_driver::{Button, ButtonConfig};
use esp_idf_hal::gpio::{AnyInputPin, Input, PinDriver};

use crate::gui::input::{InputEvent, InputInterface, KeyCode, KeyEvent};

pub type InputPinDriver<'a> = PinDriver<'a, AnyInputPin, Input>;

pub struct EspInput<'a> {
    drivers: Vec<(KeyCode, Button<InputPinDriver<'a>>)>,
}

impl<'a> EspInput<'a> {
    pub fn new(drivers: Vec<(KeyCode, InputPinDriver<'a>)>) -> Self {
        EspInput {
            drivers: drivers
                .into_iter()
                .map(|(key_code, pin)| {
                    (
                        key_code,
                        Button::new(
                            pin,
                            ButtonConfig {
                                debounce: Duration::from_millis(50),
                                ..Default::default()
                            },
                        ),
                    )
                })
                .collect(),
        }
    }
}

impl<'a> InputInterface for EspInput<'a> {
    fn get_events(&mut self) -> Vec<InputEvent> {
        let mut events = Vec::new();

        for (key_code, driver) in self.drivers.iter_mut() {
            if driver.is_clicked() {
                events.push(InputEvent {
                    key_code: *key_code,
                    key_event: KeyEvent::Clicked,
                });
            } else if driver.is_double_clicked() {
                events.push(InputEvent {
                    key_code: *key_code,
                    key_event: KeyEvent::DoubleClicked,
                });
            } else if driver.is_triple_clicked() {
                events.push(InputEvent {
                    key_code: *key_code,
                    key_event: KeyEvent::TripleClicked,
                });
            } else if let Some(held_time) = driver.held_time() {
                events.push(InputEvent {
                    key_code: *key_code,
                    key_event: KeyEvent::LongPress(held_time),
                });
            }

            driver.reset();
        }

        events
    }

    fn update(&mut self) {
        for (_, driver) in self.drivers.iter_mut() {
            driver.tick();
        }
    }
}
