//! [`bhk_core::platform::Clock`] for the T-Embed.
//!
//! Per `core/src/platform.rs`'s own doc comment: "`std::time::Instant` is
//! available on both targets today (esp-idf-svc's `std` feature provides
//! it), so no custom time type is needed yet." `esp-idf-svc/std` is
//! already an unconditional feature of this crate (see `firmware/Cargo.toml`
//! `[features] std = [...]`), so this is a direct, untested-but-trivial
//! wrapper — there is no ESP-IDF-specific behavior to get wrong here.

use std::time::Instant;

use bhk_core::platform::Clock;

/// Wall-clock access backed by `std::time::Instant`, which on the `std`
/// ESP-IDF target is ultimately backed by `esp_timer_get_time()` (a
/// hardware timer), not wall-clock RTC — i.e. it is monotonic but not
/// meaningful across reboots. That matches what `Clock::now` is used for
/// in the core (durations, not calendar time).
#[derive(Debug, Default, Clone, Copy)]
pub struct EspClock;

impl Clock for EspClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
