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
///
/// `#[cfg(not(feature = "demo-seed"))]`: the `demo-seed` build uses
/// `DemoSeedSyncSource` instead (see its doc comment for why `NoSyncSource`
/// specifically -- always-empty -- can't just have its initial items
/// overridden), so this type would otherwise be unused dead code under
/// that feature.
#[cfg(not(feature = "demo-seed"))]
struct NoSyncSource;

#[cfg(not(feature = "demo-seed"))]
impl SyncSource for NoSyncSource {
    type Error = Infallible;

    fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
        Ok(Vec::new())
    }
}

/// **TEMPORARY hardware-test aid** (bead ai-bitwarden-hw-key-ekd), gated
/// behind the off-by-default `demo-seed` cargo feature. Seeds a handful
/// of placeholder credentials purely so the credential list has
/// something to scroll through, to verify the T-Embed CC1101
/// encoder-pin fix (`NavIntent`s reaching the app and moving selection)
/// on real hardware while there is still no sync transport to populate
/// the vault for real.
///
/// This does **not** relax the "`NoSyncSource` is honest" principle in
/// this module's doc comment above: `main`'s default build (this
/// feature OFF) still starts from an empty vault. This function only
/// exists, and is only called, when `demo-seed` is explicitly enabled.
#[cfg(feature = "demo-seed")]
fn demo_vault_items() -> Vec<VaultItem> {
    fn item(name: &str, username: &str) -> VaultItem {
        VaultItem {
            id: uuid::Uuid::new_v4(),
            name: name.to_string(),
            username: username.to_string(),
            password: "hunter2".to_string(),
            uri: None,
            notes: None,
        }
    }

    vec![
        item("GitHub", "octocat"),
        item("AWS Console", "root"),
        item("Gmail", "andreas@example.com"),
        item("Bank of Example", "acoroiu"),
        item("Home Wi-Fi", "router-admin"),
    ]
}

/// The `SyncSource` half of the `demo-seed` hardware-test aid.
///
/// **Root-cause note (first attempt at this feature got this wrong):**
/// passing seeded items only to `App::new`'s constructor is NOT enough
/// to make them appear on screen. `bhk_core::run`'s loop calls
/// `app.step(sync)` — which lands in `VaultStore::apply_sync_ok`,
/// replacing the store's items whenever they differ from the sync
/// result — on *every* frame, starting with frame 1, before the first
/// render. `NoSyncSource::sync()` always returns `Ok(Vec::new())`, which
/// differs from the 5 seeded items, so the very first loop iteration
/// wiped them back to empty before anything ever rendered. Confirmed on
/// real hardware: the boot-time seed warning fired but the list still
/// showed empty.
///
/// The fix is to seed through the same path the emulator's
/// `PushSyncSource` uses (see `emulator/src/desktop/push_sync_source.rs`):
/// a `SyncSource` whose `sync()` returns the **same persisted items every
/// call**, not a one-shot value handed only to `App::new`. Once
/// `VaultStore` has applied a `Vec<VaultItem>` once, every later
/// `sync()` call returning an equal `Vec` is a no-op in
/// `apply_sync_ok`'s `items != self.items` check, so the seed survives
/// indefinitely instead of being overwritten on the next frame.
#[cfg(feature = "demo-seed")]
struct DemoSeedSyncSource {
    items: Vec<VaultItem>,
}

#[cfg(feature = "demo-seed")]
impl SyncSource for DemoSeedSyncSource {
    type Error = Infallible;

    fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
        Ok(self.items.clone())
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
        peripherals.lcd_backlight,
        peripherals.peripheral_power_on,
    )
    .expect("failed to initialize the ST7789 display");

    let input = RotaryEncoderInput::new(peripherals.encoder_pin_a, peripherals.encoder_pin_b, peripherals.encoder_button)?;

    let nvs_partition = EspDefaultNvsPartition::take()?;
    let storage = NvsStorage::new(nvs_partition)?;

    // `initial_items` is pulled from `sync.sync()` up front (matching
    // exactly how `emulator/src/main.rs` seeds `App::new` from its
    // `PushSyncSource`), not handed to `App::new` independently of
    // `sync` -- see `DemoSeedSyncSource`'s doc comment for why that
    // distinction matters: `App::new`'s constructor argument alone does
    // NOT survive the run loop's first `app.step(sync)` call.
    #[cfg(feature = "demo-seed")]
    let (mut sync, initial_items) = {
        log::warn!("demo-seed feature ENABLED: vault seeded with placeholder credentials, not real synced data -- this build is a hardware-test aid only, never ship it as default");
        let mut sync = DemoSeedSyncSource { items: demo_vault_items() };
        let initial_items = sync.sync().expect("DemoSeedSyncSource::sync is Infallible");
        log::info!("demo-seed: sync produced {} placeholder item(s)", initial_items.len());
        (sync, initial_items)
    };
    #[cfg(not(feature = "demo-seed"))]
    let (mut sync, initial_items): (NoSyncSource, Vec<VaultItem>) = (NoSyncSource, Vec::new());

    let mut platform = BoardPlatform::new(display, input, storage);
    let mut app = App::new(u32::from(DISPLAY_WIDTH), u32::from(DISPLAY_HEIGHT), initial_items);

    log::info!("Entering main loop");
    run(&mut platform, &mut app, &mut sync, FRAME_BUDGET, || true);

    Ok(())
}
