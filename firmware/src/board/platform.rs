//! `BoardPlatform`: assembles the T-Embed board adapters (`St7789Surface`,
//! `RotaryEncoderInput`, `EspClock`, `NvsStorage`) into a concrete
//! `bhk_core::platform::Platform`, so `main.rs` can hand a single value to
//! the unified `bhk_core::run` loop — the real-target counterpart to
//! `emulator::platform::HostPlatform`.
//!
//! Unlike `HostPlatform` (generic over which `DisplaySurface`/`InputSource`
//! back it, because the emulator has two of each), this is concrete: the
//! real-target binary only ever has exactly one display and one input
//! driver.

use bhk_core::platform::Platform;

use super::clock::EspClock;
use super::nvs_storage::NvsStorage;
use super::rotary_input::RotaryEncoderInput;
use super::st7789_surface::St7789Surface;

pub struct BoardPlatform {
    display: St7789Surface,
    input: RotaryEncoderInput,
    clock: EspClock,
    storage: NvsStorage,
}

impl BoardPlatform {
    #[must_use]
    pub fn new(display: St7789Surface, input: RotaryEncoderInput, storage: NvsStorage) -> Self {
        Self { display, input, clock: EspClock, storage }
    }
}

impl Platform for BoardPlatform {
    type Display = St7789Surface;
    type Input = RotaryEncoderInput;
    type Clock = EspClock;
    type Storage = NvsStorage;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.display
    }

    fn input(&mut self) -> &mut Self::Input {
        &mut self.input
    }

    fn clock(&self) -> &Self::Clock {
        &self.clock
    }

    fn storage(&mut self) -> &mut Self::Storage {
        &mut self.storage
    }
}
