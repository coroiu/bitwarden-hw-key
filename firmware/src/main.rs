//! T-Embed (ESP32-S3) firmware entry point: assembles the real
//! board adapters (`board::BoardPlatform`) and hands them to the unified,
//! `Platform`-generic `bhk_core::run` loop — the same loop
//! `emulator/src/main.rs` drives for the windowed/headless run modes, per
//! `.planning/decisions/2026-08-11-three-mode-testability.md`.
//!
//! Replaces the old HUZZAH32 prototype's direct SSD1306 + 3-button wiring
//! (`gui`/`simple_gui`/`view`/`simple_view`/`esp_input`/`time`, all deleted
//! in this bead) — see `board`'s module doc for what is and isn't verified
//! about the new wiring.
//!
//! # No sync transport on real hardware yet
//!
//! There is no BLE/USB companion-push transport implemented for the
//! T-Embed yet (see
//! `.planning/decisions/2026-08-11-sync-direction-companion-push.md`) —
//! `emulator`'s `PushSyncSource` wraps an HTTP server that only exists on
//! the host. [`NoSyncSource`] below is an honest placeholder: an always-
//! empty vault, not a fake one, so the render pipeline stays real (the
//! M0 "empty-but-real" shell) while there is nothing yet to sync from.
//!
//! # Build-only
//!
//! This binary compiles and links for `xtensa-esp32s3-espidf`. It has not
//! run against real hardware (no T-Embed is attached to the machine this
//! was written on) — see `board`'s module doc for the full list of
//! per-adapter unverified assumptions.

mod board;

use std::convert::Infallible;
use std::time::Duration;

use bhk_core::{run, App, SyncSource, VaultItem};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::EspError;

use crate::board::{BoardPeripherals, BoardPlatform, NvsStorage, RotaryEncoderInput, St7789Surface, DISPLAY_HEIGHT, DISPLAY_WIDTH};

/// ~30fps, matching the emulator's `FRAME_BUDGET` — no product reason yet
/// for the two to differ (no animation, nothing latency-sensitive in this
/// bead's credential-list shell).
const FRAME_BUDGET: Duration = Duration::from_millis(33);

/// Placeholder `SyncSource` until a real companion-push transport
/// (BLE/USB) exists for the board. Always reports an empty vault rather
/// than fabricating data.
struct NoSyncSource;

impl SyncSource for NoSyncSource {
    type Error = Infallible;

    fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
        Ok(Vec::new())
    }
}

fn main() -> Result<(), EspError> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Bitwarden HW Key - T-Embed firmware starting");

    let peripherals = BoardPeripherals::take()?;

    let display = St7789Surface::new(
        peripherals.lcd_spi,
        peripherals.lcd_sclk,
        peripherals.lcd_mosi,
        peripherals.lcd_cs,
        peripherals.lcd_dc,
        peripherals.lcd_reset,
        peripherals.lcd_backlight,
        peripherals.peripheral_power_on,
    )
    .expect("failed to initialize the ST7789 display");

    let input = RotaryEncoderInput::new(peripherals.encoder_pin_a, peripherals.encoder_pin_b, peripherals.encoder_button)?;

    let nvs_partition = EspDefaultNvsPartition::take()?;
    let storage = NvsStorage::new(nvs_partition)?;

    let mut platform = BoardPlatform::new(display, input, storage);
    let mut app = App::new(u32::from(DISPLAY_WIDTH), u32::from(DISPLAY_HEIGHT), Vec::new());
    let mut sync = NoSyncSource;

    log::info!("Entering main loop");
    run(&mut platform, &mut app, &mut sync, FRAME_BUDGET, || true);

    Ok(())
}
