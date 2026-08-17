//! [`bhk_core::platform::InputSource`] for the T-Embed's rotary encoder,
//! per `.planning/decisions/2026-08-11-rotary-encoder-input-model.md`.
//!
//! Raw quadrature decoding is delegated to `rotary-encoder-hal` (a small,
//! well-established `embedded-hal`-based crate; no reason to hand-roll a
//! quadrature state machine). The push-button (click = `Activate`,
//! long-press = `Back`) reuses `button-driver`, already a dependency for
//! the old 3-button `esp_input.rs` input driver — its default
//! `ButtonConfig::hold` is 500ms, which happens to match the ADR's
//! long-press threshold exactly, so no custom config is needed there.
//!
//! What this module adds on top of both crates is the ADR's acceleration
//! rule: "fast rotation (≥4 ticks in 100ms) → `NextN`". Neither
//! dependency has a notion of rotation speed (`rotary-encoder-hal` reports
//! one `Direction` per `update()` call with no timing; `button-driver` is
//! button-only), so [`AccelerationWindow`] below is this module's own
//! rolling-window tick counter, driven by `std::time::Instant` (a
//! driver-local timing detail, not the injected `Clock` — the ADR treats
//! encoder acceleration timing as a platform/input-driver concern, not
//! something the app core should see).
//!
//! **Not yet exercised against a real encoder.** The GPIO pins below were
//! corrected to the T-Embed CC1101's actual encoder wiring (bead
//! ai-bitwarden-hw-key-ekd; see `board_config`'s `ENCODER_PIN_A`/`B` for
//! sourcing) alongside the display fix in bead ai-bitwarden-hw-key-c6e,
//! but as of that change nothing here has been exercised against the
//! real encoder yet: not the quadrature decoding, not the acceleration
//! thresholds (chosen to match the ADR's literal wording, not tuned
//! against real spin speeds), and not the pull-up assumption below.

use std::time::{Duration, Instant};

use bhk_core::{platform::InputSource, NavIntent};
use button_driver::{Button, ButtonConfig};
use esp_idf_hal::gpio::{Gpio0, Gpio4, Gpio5, Input, PinDriver, Pull};
use esp_idf_hal::sys::EspError;
use rotary_encoder_hal::{DefaultPhase, Direction, Rotary};

/// Rolling window the ADR specifies for acceleration: "≥4 ticks in
/// 100ms". A tick that arrives more than this long after the previous
/// one (or reverses direction) starts a new window instead of extending
/// the old one.
const FAST_ROTATION_WINDOW: Duration = Duration::from_millis(100);

/// Ticks-in-window threshold before `Next`/`Prev` upgrades to `NextN`.
const FAST_ROTATION_TICK_THRESHOLD: u16 = 4;

/// Upper bound on `NextN`, per the ADR ("capped at 16 to prevent jumps
/// larger than one screen").
const MAX_JUMP: u16 = 16;

/// Tracks same-direction ticks within a rolling window, to decide when a
/// single-tick `Next`/`Prev` should become an accelerated `NextN`.
///
/// This is this module's own interpretation of the ADR's "≥4 ticks in
/// 100ms → `NextN(min(ticks * 2, 16))`" rule: on every tick, if the
/// window is still open (same direction, within `FAST_ROTATION_WINDOW`
/// of the first tick in it), the window's tick count grows and the
/// resulting intent is `NextN(min(count * 2, 16))` once the count
/// reaches the threshold; otherwise a fresh window starts and the intent
/// is a plain `Next`/`Prev`. The ADR does not specify this algorithm at
/// the pseudocode level, and it has not been tuned against a real
/// encoder's rotation speeds.
struct AccelerationWindow {
    started_at: Instant,
    direction: Direction,
    ticks: u16,
}

impl AccelerationWindow {
    fn classify(window: &mut Option<Self>, direction: Direction, now: Instant) -> NavIntent {
        let reuse = matches!(window, Some(w) if w.direction == direction && now.duration_since(w.started_at) <= FAST_ROTATION_WINDOW);

        let ticks = if reuse {
            let w = window.as_mut().expect("checked by `reuse` above");
            w.ticks += 1;
            w.ticks
        } else {
            *window = Some(Self {
                started_at: now,
                direction,
                ticks: 1,
            });
            1
        };

        let base = match direction {
            Direction::Clockwise => NavIntent::Next,
            Direction::CounterClockwise => NavIntent::Prev,
            Direction::None => unreachable!("classify is only called for Clockwise/CounterClockwise"),
        };

        if ticks >= FAST_ROTATION_TICK_THRESHOLD {
            NavIntent::NextN((ticks * 2).min(MAX_JUMP))
        } else {
            base
        }
    }
}

/// `InputSource` for the T-Embed's EC11 rotary encoder + integrated
/// push-button (`board::board_config::{ENCODER_PIN_A, ENCODER_PIN_B,
/// ENCODER_BUTTON_PIN}`).
///
/// Owns and configures its three GPIOs itself (rather than taking
/// pre-configured `PinDriver`s) because the pull-up requirement below is
/// encoder wiring knowledge, not board-specific pin-map knowledge — it
/// belongs with the driver, not `board_config`.
///
/// The T-Embed CC1101 also has an independent user button on GPIO6
/// (`BOARD_USER_KEY`, separate from the encoder's own press), a
/// candidate for a dedicated `Back` in the future. It is intentionally
/// **not** wired up here — out of scope for this pass, tracked in bead
/// ai-bitwarden-hw-key-ekd alongside the rest of the multi-variant board
/// selection.
/// Type order matches the `(b, a)` argument order `Rotary::new` is
/// actually called with below (see that call site's comment for why) —
/// `rotary-encoder-hal`'s `Rotary<A, B, _>` decodes direction from
/// whichever pin is passed first as leading/second as lagging, so the
/// generic order here must track the constructor call's argument order,
/// not `ENCODER_PIN_A`/`ENCODER_PIN_B`'s naming.
pub struct RotaryEncoderInput {
    rotary: Rotary<PinDriver<'static, Gpio5, Input>, PinDriver<'static, Gpio4, Input>, DefaultPhase>,
    button: Button<PinDriver<'static, Gpio0, Input>, Instant>,
    fast_window: Option<AccelerationWindow>,
}

impl RotaryEncoderInput {
    /// # Errors
    ///
    /// Returns `EspError` if any of the three GPIOs can't be configured
    /// as pulled-up digital inputs.
    ///
    /// # Untested assumption
    ///
    /// All three pins are configured with an internal pull-up, active
    /// low. This matches the wiring convention `esp_input.rs` uses for
    /// the old 3-button input and is the common wiring for EC11-style
    /// encoders, but the T-Embed's specific encoder module has not been
    /// inspected on hardware to confirm it doesn't already provide its
    /// own pull-ups (which would make this redundant but harmless) or,
    /// worse, pull-downs (which would make this wrong).
    pub fn new(pin_a: Gpio4, pin_b: Gpio5, button_pin: Gpio0) -> Result<Self, EspError> {
        let mut a = PinDriver::input(pin_a)?;
        a.set_pull(Pull::Up)?;
        let mut b = PinDriver::input(pin_b)?;
        b.set_pull(Pull::Up)?;
        let mut btn = PinDriver::input(button_pin)?;
        btn.set_pull(Pull::Up)?;

        Ok(Self {
            // Hardware-confirmed direction correction (bead
            // ai-bitwarden-hw-key-bgl): passed as `Rotary::new(b, a)`,
            // NOT `(a, b)`. On real T-Embed CC1101 hardware, wiring
            // `ENCODER_PIN_A`/`ENCODER_PIN_B` (GPIO4/GPIO5, per their own
            // vendor-sourced doc comments in `board_config.rs`) to
            // `Rotary::new` in that A-then-B order produced CCW ->
            // `NavIntent::Next` (backwards from the spec: CW should be
            // `Next`/"move down"). `rotary-encoder-hal` derives CW/CCW
            // purely from which of its two input pins it's told leads the
            // other, so swapping the two arguments here flips the
            // decoded direction to match physical rotation without
            // touching `ENCODER_PIN_A`/`ENCODER_PIN_B` themselves (those
            // still correctly name GPIO4/GPIO5 per the CC1101's actual
            // wiring) or `AccelerationWindow::classify`'s
            // `Direction`->`NavIntent` mapping (still the natural
            // CW=Next/CCW=Prev the ADR specifies) -- the swap belongs at
            // the decode boundary, not layered on top of it.
            rotary: Rotary::new(b, a),
            button: Button::new(
                btn,
                ButtonConfig {
                    mode: button_driver::Mode::PullUp,
                    ..Default::default()
                },
            ),
            fast_window: None,
        })
    }
}

impl InputSource for RotaryEncoderInput {
    fn poll(&mut self) -> Vec<NavIntent> {
        let mut intents = Vec::new();
        let now = Instant::now();

        match self.rotary.update() {
            Ok(direction @ (Direction::Clockwise | Direction::CounterClockwise)) => {
                intents.push(AccelerationWindow::classify(&mut self.fast_window, direction, now));
            }
            Ok(Direction::None) => {}
            Err(_) => {
                // `Rotary::update()` only fails if reading pin_a/pin_b
                // fails (`Either<A::Error, B::Error>`); on esp-idf-hal
                // that's effectively infallible for a configured input
                // pin, so this is logged rather than propagated (the
                // `InputSource` trait has no error channel to propagate
                // to per the ADR's `poll(&mut self) -> Vec<NavIntent>`
                // signature).
                log::warn!("RotaryEncoderInput: encoder pin read failed");
            }
        }

        self.button.tick();
        if self.button.is_clicked() {
            intents.push(NavIntent::Activate);
        } else if self.button.held_time().is_some() {
            intents.push(NavIntent::Back);
        }
        self.button.reset();

        // Debug aid for on-hardware verification (bead ekd): confirms the
        // encoder->NavIntent path is alive over serial, independent of
        // whatever the screen shows. Not feature-gated (unlike
        // `main`'s `demo-seed`) since it's not fake data, just a log
        // line -- cheap enough to leave in permanently, but revisit if it
        // turns out to spam the log once real usage starts.
        for intent in &intents {
            log::info!("RotaryEncoderInput: emitting {intent:?}");
        }

        intents
    }
}
