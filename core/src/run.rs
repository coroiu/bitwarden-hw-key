//! The unified, `Platform`-generic main loop (W7): the one piece of code
//! that drives an [`App`] regardless of which run mode (headless, windowed,
//! real-target) it's given a [`Platform`] for. Per the presentation-surface
//! ADR, the three modes must differ *only* in which concrete
//! `DisplaySurface`/`InputSource`/`Clock`/`Storage` they hand to [`run`] —
//! this function itself never branches on which mode it's in.
//!
//! ```text
//! loop {
//!     let intents = input.poll();
//!     app.handle_input(intents);
//!     app.step(sync);
//!     if app.dirty() {
//!         let fb = app.render();
//!         display.flush(&fb);
//!     }
//!     sleep(frame_budget - elapsed);
//! }
//! ```
//!
//! Note the `if app.dirty()` gate: render+flush only happen when
//! something actually changed, not unconditionally every iteration — an
//! idle loop (no input, no sync changes) still spins at `frame_budget`
//! cadence but skips the expensive part entirely. The off-by-default
//! `frame-timing` feature (bead ai-bitwarden-hw-key-ego) logs a rolling
//! average of render/flush durations for exactly the frames that *do*
//! take this path — see [`FrameTiming`].
//!
//! # Why `should_continue` instead of an unconditional `loop`
//!
//! A bare infinite loop is exactly right for the real-target firmware
//! binary (it never exits) and is what the two host binaries use in
//! practice too — but host callers need *some* way to stop the loop
//! (closing the window, an HTTP shutdown signal, or — for headless
//! automated verification — "stop after N frames so a screenshot can be
//! taken and the process can exit"). Rather than hardcode any of those
//! conditions here (which would smuggle a run-mode-specific concept into
//! supposedly mode-generic code), the loop takes a `should_continue`
//! predicate and lets each binary decide what "keep going" means for it.
//! The firmware binary just passes `|| true`.
//!
//! # Why sleeping is `std::thread::sleep`, not a `Clock` method
//!
//! `Clock` (frozen in W1, see `crate::platform`) only exposes `now()`, not
//! a sleep primitive — deliberately: the trait models *reading* wall-clock
//! time, not scheduling. `std::thread::sleep` is available on both targets
//! today (the same `std`-on-ESP-IDF support that makes `Instant` available
//! per `crate::platform`'s doc comment covers `thread::sleep` too), so
//! there is no portability reason to route it through an injected trait.

use std::time::Duration;

use crate::app::App;
use crate::platform::{Clock, DisplaySurface, InputSource, Platform};
use crate::sync_source::SyncSource;

/// Rolling-average frame-timing accumulator, active only behind the
/// off-by-default `frame-timing` feature (bead ai-bitwarden-hw-key-ego).
/// Diagnostic-only: measures where a *rendered* (dirty) frame's time
/// actually goes -- render-into-framebuffer vs. flush-to-display -- using
/// the same injected [`Clock`] the loop itself uses for its frame-budget
/// sleep, so these numbers are directly comparable to `frame_budget`.
///
/// Only dirty frames are counted (a frame that skips render+flush
/// entirely has nothing meaningful to report for either duration), so
/// "30 frames" here means 30 *rendered* frames, however many total loop
/// iterations that spans.
#[cfg(feature = "frame-timing")]
struct FrameTiming {
    render_total: Duration,
    flush_total: Duration,
    count: u32,
}

/// Rate-limits `DisplaySurface::flush` failure logging so a
/// persistently-failing display doesn't spam the log at frame rate.
///
/// Bead ai-bitwarden-hw-key-mqk: a DMA misconfiguration once made
/// `flush` fail on *every single frame*, and because `run`'s loop
/// discarded the `Result` outright, the screen just silently froze on
/// the last good frame with no signal anywhere — it looked like a hang,
/// not a failure, until someone happened to look at the actual panel.
/// The loop must still stay infallible (per the presentation-surface
/// ADR: device-specific errors are absorbed at the surface adapter, not
/// propagated into the platform-free core) — this only adds
/// *visibility*, logging a warning on the first failure (so a
/// transition from healthy to broken is never silent) and then every
/// [`FlushErrorTracker::REPEAT_INTERVAL`]th consecutive failure after
/// that (so a *persistent* failure keeps showing up over serial without
/// drowning normal operation in per-frame noise), resetting on the next
/// success so a later failure logs fresh again.
#[derive(Default)]
struct FlushErrorTracker {
    consecutive_errors: u32,
}

impl FlushErrorTracker {
    /// Arbitrary, not tuned against a real failure's time-to-notice
    /// requirement: at the 33ms/frame budget this is roughly every 5
    /// seconds, which is frequent enough that a human watching serial
    /// output won't wait long to see it repeat, without being frequent
    /// enough to look like per-frame spam.
    const REPEAT_INTERVAL: u32 = 150;

    /// Records a failed `flush` and logs a warning if this is the first
    /// failure since the last success, or every `REPEAT_INTERVAL`th one
    /// after that.
    fn on_err(&mut self, error: &impl core::fmt::Debug) {
        self.consecutive_errors += 1;
        if self.consecutive_errors == 1 || self.consecutive_errors % Self::REPEAT_INTERVAL == 0 {
            log::warn!("DisplaySurface::flush failed ({} consecutive): {error:?}", self.consecutive_errors);
        }
    }

    /// Records a successful `flush`, resetting the consecutive-error
    /// count so a later failure is treated as a fresh "just started
    /// failing" event (and logged immediately) rather than a
    /// continuation of an old, already-resolved one.
    fn on_ok(&mut self) {
        self.consecutive_errors = 0;
    }
}

#[cfg(feature = "frame-timing")]
impl FrameTiming {
    const WINDOW: u32 = 30;

    const fn new() -> Self {
        Self { render_total: Duration::ZERO, flush_total: Duration::ZERO, count: 0 }
    }

    /// Records one dirty frame's render/flush durations; logs and resets
    /// the accumulator once `WINDOW` frames have been recorded.
    fn record(&mut self, render: Duration, flush: Duration) {
        self.render_total += render;
        self.flush_total += flush;
        self.count += 1;

        if self.count >= Self::WINDOW {
            let n = f64::from(self.count);
            let avg_render_ms = self.render_total.as_secs_f64() * 1000.0 / n;
            let avg_flush_ms = self.flush_total.as_secs_f64() * 1000.0 / n;
            let avg_total_ms = avg_render_ms + avg_flush_ms;
            let fps = if avg_total_ms > 0.0 { 1000.0 / avg_total_ms } else { 0.0 };
            log::info!(
                "frame-timing: render={avg_render_ms:.2}ms flush={avg_flush_ms:.2}ms total={avg_total_ms:.2}ms -> {fps:.1}fps (avg over {} rendered frames)",
                self.count
            );
            *self = Self::new();
        }
    }
}

/// Runs the app loop against `platform` until `should_continue` returns
/// `false`. `frame_budget` is the target time per iteration (input poll +
/// app step + render + flush); if an iteration finishes early, the
/// remainder of the budget is spent asleep so the loop doesn't spin.
///
/// Takes `platform`/`app`/`sync` by `&mut` (rather than by value) so
/// callers retain ownership after `run` returns — e.g. a headless caller
/// that wants to encode a PNG from its concrete `HeadlessSurface` once the
/// loop stops.
///
/// `<P::Display as DisplaySurface>::Error: Debug` (bead
/// ai-bitwarden-hw-key-mqk) is required so a persistently-failing
/// `flush` can be logged (see [`FlushErrorTracker`]) — every concrete
/// `DisplaySurface` in this codebase already satisfies this (firmware's
/// `St7789SurfaceError` derives `Debug`; the emulator surfaces use
/// `Infallible`, which is `Debug`), so this is not expected to be a
/// breaking bound for any real caller.
pub fn run<P: Platform, S: SyncSource>(
    platform: &mut P,
    app: &mut App,
    sync: &mut S,
    frame_budget: Duration,
    mut should_continue: impl FnMut() -> bool,
) where
    S::Error: std::fmt::Display,
    <P::Display as DisplaySurface>::Error: core::fmt::Debug,
{
    #[cfg(feature = "frame-timing")]
    let mut frame_timing = FrameTiming::new();
    let mut flush_errors = FlushErrorTracker::default();

    while should_continue() {
        let frame_start = platform.clock().now();

        let intents = platform.input().poll();
        app.handle_input(intents);
        app.step(sync);

        if app.dirty() {
            #[cfg(feature = "frame-timing")]
            let render_start = platform.clock().now();

            let framebuffer = app.render();

            #[cfg(feature = "frame-timing")]
            let render_end = platform.clock().now();

            // A flush failure (e.g. a real SPI write error on hardware) is
            // not something this loop can meaningfully recover from frame
            // to frame; per the presentation-surface ADR, device-specific
            // errors are absorbed at the surface adapter, not propagated
            // into the platform-free core. Still not panicking here (the
            // loop stays infallible) -- but no longer silently discarded
            // either: `FlushErrorTracker` makes a persistent failure
            // visible over serial (rate-limited) instead of looking like
            // an inexplicable frozen screen (bead ai-bitwarden-hw-key-mqk).
            match platform.display().flush(framebuffer) {
                Ok(()) => flush_errors.on_ok(),
                Err(error) => flush_errors.on_err(&error),
            }

            #[cfg(feature = "frame-timing")]
            {
                let flush_end = platform.clock().now();
                frame_timing.record(
                    render_end.saturating_duration_since(render_start),
                    flush_end.saturating_duration_since(render_end),
                );
            }
        }

        let elapsed = platform.clock().now().saturating_duration_since(frame_start);
        if let Some(remaining) = frame_budget.checked_sub(elapsed) {
            std::thread::sleep(remaining);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::NavIntent;
    use crate::platform::FrameBuffer565;
    use crate::vault_item::VaultItem;
    use std::cell::RefCell;
    use std::convert::Infallible;
    use std::rc::Rc;
    use std::time::Instant;
    use uuid::Uuid;

    struct StubDisplay {
        flush_count: Rc<RefCell<u32>>,
    }
    impl DisplaySurface for StubDisplay {
        type Error = Infallible;
        fn flush(&mut self, _framebuffer: &FrameBuffer565) -> Result<(), Self::Error> {
            *self.flush_count.borrow_mut() += 1;
            Ok(())
        }
    }

    /// Error type for [`FailingStubDisplay`]. `Debug`-only (no
    /// `Display`/`std::error::Error` impl) -- deliberately the bare
    /// minimum `run`'s new trait bound requires, so this test doesn't
    /// accidentally prove more than the bound actually demands.
    #[derive(Debug)]
    struct StubFlushError;

    /// A `DisplaySurface` whose `flush` always fails -- for proving
    /// `run` tolerates a *persistently* failing display (bead
    /// ai-bitwarden-hw-key-mqk) without panicking, as opposed to
    /// `StubDisplay`'s always-succeeds `Infallible` case above.
    struct FailingStubDisplay;
    impl DisplaySurface for FailingStubDisplay {
        type Error = StubFlushError;
        fn flush(&mut self, _framebuffer: &FrameBuffer565) -> Result<(), Self::Error> {
            Err(StubFlushError)
        }
    }

    struct QueuedInput(Vec<Vec<NavIntent>>);
    impl InputSource for QueuedInput {
        fn poll(&mut self) -> Vec<NavIntent> {
            if self.0.is_empty() {
                Vec::new()
            } else {
                self.0.remove(0)
            }
        }
    }

    #[derive(Default)]
    struct StubStorage;
    impl crate::platform::Storage for StubStorage {
        type Error = Infallible;
        fn get(&self, _key: &str) -> Option<Vec<u8>> {
            None
        }
        fn set(&mut self, _key: &str, _value: Vec<u8>) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[derive(Default, Clone, Copy)]
    struct StubClock;
    impl Clock for StubClock {
        fn now(&self) -> Instant {
            Instant::now()
        }
    }

    struct StubPlatform {
        display: StubDisplay,
        input: QueuedInput,
        clock: StubClock,
        storage: StubStorage,
    }
    impl Platform for StubPlatform {
        type Display = StubDisplay;
        type Input = QueuedInput;
        type Clock = StubClock;
        type Storage = StubStorage;

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

    /// Mirrors `StubPlatform`, but with `FailingStubDisplay` in place of
    /// `StubDisplay` -- used only by the flush-error-tolerance test
    /// below, so `run`'s new `<P::Display as DisplaySurface>::Error:
    /// Debug` bound is exercised against a real (non-`Infallible`) error
    /// type, not just satisfied vacuously.
    struct FailingStubPlatform {
        display: FailingStubDisplay,
        input: QueuedInput,
        clock: StubClock,
        storage: StubStorage,
    }
    impl Platform for FailingStubPlatform {
        type Display = FailingStubDisplay;
        type Input = QueuedInput;
        type Clock = StubClock;
        type Storage = StubStorage;

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

    struct EmptySyncSource;
    impl SyncSource for EmptySyncSource {
        type Error = Infallible;
        fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn a_persistently_failing_flush_does_not_panic_and_the_loop_keeps_running() {
        // Bead ai-bitwarden-hw-key-mqk's actual failure mode: a display
        // that fails on EVERY frame, not just an occasional one (that's
        // what a DMA misconfiguration making every SPI transaction
        // invalid looks like). Every queued frame carries a `Next`
        // intent so `app.dirty()` is true and `flush` (which always
        // errors) is genuinely attempted on every single iteration, not
        // skipped by the dirty-gate.
        const ITERATIONS: usize = 10;
        let mut platform = FailingStubPlatform {
            display: FailingStubDisplay,
            input: QueuedInput(vec![vec![NavIntent::Next]; ITERATIONS]),
            clock: StubClock,
            storage: StubStorage,
        };
        let items = vec![
            VaultItem { id: Uuid::new_v4(), name: "a".into(), username: "a".into(), password: String::new(), uri: None, notes: None },
            VaultItem { id: Uuid::new_v4(), name: "b".into(), username: "b".into(), password: String::new(), uri: None, notes: None },
        ];
        let mut app = App::new(320, 170, items);
        let mut sync = EmptySyncSource;

        let mut iterations = 0;
        // The absence of a panic across every one of these iterations
        // IS the assertion: `run` took the `Err` branch (not `Ok`)
        // `ITERATIONS` times in a row and kept going regardless, per the
        // presentation-surface ADR's "loop stays infallible" contract.
        run(&mut platform, &mut app, &mut sync, Duration::from_millis(0), || {
            iterations += 1;
            iterations <= ITERATIONS
        });

        assert_eq!(iterations, ITERATIONS + 1, "should_continue is checked once more after the last real iteration");
    }

    #[test]
    fn run_stops_when_should_continue_returns_false() {
        let flush_count = Rc::new(RefCell::new(0));
        let mut platform = StubPlatform {
            display: StubDisplay { flush_count: Rc::clone(&flush_count) },
            input: QueuedInput(Vec::new()),
            clock: StubClock,
            storage: StubStorage,
        };
        let mut app = App::new(10, 10, Vec::new());
        let mut sync = EmptySyncSource;

        let mut iterations = 0;
        run(&mut platform, &mut app, &mut sync, Duration::from_millis(0), || {
            iterations += 1;
            iterations <= 3
        });

        assert_eq!(iterations, 4, "should_continue is checked once more after the last real iteration");
        // Only the first iteration is dirty (fresh `App`); the rest have
        // nothing new to render.
        assert_eq!(*flush_count.borrow(), 1);
    }

    #[test]
    fn polled_intents_are_forwarded_to_the_app_and_trigger_a_flush() {
        let flush_count = Rc::new(RefCell::new(0));
        let mut platform = StubPlatform {
            display: StubDisplay { flush_count: Rc::clone(&flush_count) },
            input: QueuedInput(vec![vec![], vec![NavIntent::Next], vec![]]),
            clock: StubClock,
            storage: StubStorage,
        };
        let items = vec![
            VaultItem { id: Uuid::new_v4(), name: "a".into(), username: "a".into(), password: String::new(), uri: None, notes: None },
            VaultItem { id: Uuid::new_v4(), name: "b".into(), username: "b".into(), password: String::new(), uri: None, notes: None },
        ];
        let mut app = App::new(320, 170, items);
        let mut sync = EmptySyncSource;

        let mut iterations = 0;
        run(&mut platform, &mut app, &mut sync, Duration::from_millis(0), || {
            iterations += 1;
            iterations <= 3
        });

        // Frame 1: fresh app, dirty -> flush. Frame 2: `Next` intent ->
        // dirty -> flush. Frame 3: no new input -> not dirty -> no flush.
        assert_eq!(*flush_count.borrow(), 2);
    }
}
