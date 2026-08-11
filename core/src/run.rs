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

/// Runs the app loop against `platform` until `should_continue` returns
/// `false`. `frame_budget` is the target time per iteration (input poll +
/// app step + render + flush); if an iteration finishes early, the
/// remainder of the budget is spent asleep so the loop doesn't spin.
///
/// Takes `platform`/`app`/`sync` by `&mut` (rather than by value) so
/// callers retain ownership after `run` returns — e.g. a headless caller
/// that wants to encode a PNG from its concrete `HeadlessSurface` once the
/// loop stops.
pub fn run<P: Platform>(
    platform: &mut P,
    app: &mut App,
    sync: &mut impl SyncSource,
    frame_budget: Duration,
    mut should_continue: impl FnMut() -> bool,
) {
    while should_continue() {
        let frame_start = platform.clock().now();

        let intents = platform.input().poll();
        app.handle_input(intents);
        app.step(sync);

        if app.dirty() {
            let framebuffer = app.render();
            // A flush failure (e.g. a real SPI write error on hardware) is
            // not something this loop can meaningfully recover from frame
            // to frame; per the presentation-surface ADR, device-specific
            // errors are absorbed at the surface adapter, not propagated
            // into the platform-free core. Dropping it here (rather than
            // panicking) keeps the loop itself infallible.
            let _ = platform.display().flush(framebuffer);
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

    struct EmptySyncSource;
    impl SyncSource for EmptySyncSource {
        type Error = Infallible;
        fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
            Ok(Vec::new())
        }
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
