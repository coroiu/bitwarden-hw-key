//! [`bhk_core::platform::InputSource`] for the T-Embed's rotary encoder,
//! per `.planning/decisions/2026-08-11-rotary-encoder-input-model.md`.
//!
//! # Quadrature decode: hardware GPIO-edge interrupts, not frame-rate polling
//!
//! **Replaces an earlier `rotary-encoder-hal`-based design** (bead
//! ai-bitwarden-hw-key-47d: "erratic scrolling, borderline unusable" on
//! real hardware). Root cause: that design called `Rotary::update()`
//! once per render frame (`bhk_core::run`'s ~30Hz/33ms loop) — a
//! hand-spun EC11 emits quadrature transitions far faster than 30Hz, so
//! polling at frame rate aliased/missed transitions and miscounted
//! ticks.
//!
//! The fix replicates the mechanism LilyGo's own T-Embed-CC1101 factory
//! firmware uses for this exact encoder module (confirmed by reading
//! `github.com/Xinyuan-LilyGO/T-Embed-CC1101`'s
//! `examples/factory/factory.cpp`, which the human independently
//! observed "handles this exact encoder great" on the same physical
//! hardware): **GPIO edge interrupts on both quadrature pins** (plain
//! GPIO edge interrupts via `esp-idf-hal`'s `PinDriver::subscribe` —
//! **not** the ESP32-S3's PCNT pulse-counter peripheral; the factory
//! firmware doesn't use PCNT either, so this doesn't), **decoded by a
//! software state-transition table, accumulated into a counter
//! completely decoupled from the render loop's timing.** The interrupt
//! fires on every single edge (via the ESP32-S3's own GPIO edge-detect
//! hardware) regardless of how fast the encoder spins or how slowly the
//! render loop happens to be running that frame; [`InputSource::poll`]
//! merely *drains* whatever the ISR has accumulated since the last call
//! — no ticks are ever missed between drains, only batched.
//!
//! [`ENC_TABLE`], the ISR's read-both-pins-then-lookup logic, and
//! [`EncoderIsrState::take_delta`]'s "divide accumulated half-steps by
//! 2, only consuming an exact multiple of 2" convention are a direct,
//! deliberate port of `factory.cpp`'s `kEncTable`, `onEncoderChange()`,
//! and `takeEncoderDelta()` — see each item's doc comment for the exact
//! correspondence. This is not a novel design: it's the one LilyGo
//! already validated works well against this specific encoder module,
//! which is the whole point of copying it rather than inventing another
//! polling-based scheme.
//!
//! The push-button (click = `Activate`, long-press = `Back`) is
//! unchanged from the original design: it still reuses `button-driver`
//! (already a dependency for the old 3-button `esp_input.rs` input
//! driver), polled once per frame via `poll()` as before — buttons
//! don't have the quadrature-aliasing problem a rotating encoder does
//! (a single press/release pair can't "alias" the way rapid
//! back-and-forth quadrature edges can), and the human's bug report was
//! specifically about scrolling, not button response.
//!
//! # No fast-rotation acceleration (intentionally dropped)
//!
//! An earlier version of this module had an `AccelerationWindow` that
//! turned fast rotation (the ADR's "≥4 ticks in 100ms") into
//! `NavIntent::NextN` multi-item jumps. **Deliberately removed** per the
//! human's explicit request after testing the GPIO-interrupt decoder on
//! real hardware: base (non-accelerated) scrolling was much better, but
//! fast continuous scrolling was still erratic, and the acceleration
//! path was the likely remaining cause (it also happened to be the
//! entire reason bead ai-bitwarden-hw-key-2ed existed:
//! `AccelerationWindow::classify` only ever emitted `NextN`, jump
//! *forward*, even for a fast counter-clockwise spin, since
//! `bhk_core::NavIntent` has no `PrevN`). Every detent now emits exactly
//! one `NavIntent::Next`/`Prev`, however many detents land in a single
//! poll — this module makes no attempt to detect or reward "fast"
//! rotation. Both this behavior and bead 2ed's quirk are moot for now.
//! Acceleration could be reintroduced properly later (with a real,
//! signed `PrevN`-equivalent) per beads ai-bitwarden-hw-key-47d and
//! ai-bitwarden-hw-key-2ed, but is out of scope until the base one-tick-
//! per-detent path is confirmed smooth.
//!
//! **Not yet exercised against real hardware** as of this revision (the
//! erratic-scrolling report predates the GPIO-interrupt decoder, and the
//! acceleration removal + direction fix below predate this exact commit)
//! — pending the human's next on-hardware pass to confirm smooth,
//! predictable one-step-per-detent scrolling in both directions.

use std::sync::atomic::{AtomicI32, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;

use bhk_core::{platform::InputSource, NavIntent};
use button_driver::{Button, ButtonConfig};
use esp_idf_hal::gpio::{Gpio0, Gpio4, Gpio5, Input, InterruptType, PinDriver, Pull};
use esp_idf_hal::sys::{gpio_get_level, gpio_intr_enable, EspError};

/// Signed per-transition delta table, indexed `[prev_state][cur_state]`,
/// both 2-bit `(lead << 1 | lag)` values `0..=3`. A `0` entry means "not
/// a valid single-step quadrature transition" (a repeated state, or an
/// electrically-impossible double-bit jump) — this is the actual
/// anti-glitch mechanism: bounce/noise on the lines is silently ignored
/// (contributes zero) rather than miscounted, with no separate debounce
/// timer needed for the quadrature signal itself (unlike the push-button,
/// which still needs `button-driver`'s own timed debounce).
///
/// Copied byte-for-byte from LilyGo's own T-Embed-CC1101 factory
/// firmware, `examples/factory/factory.cpp`'s `kEncTable` — see the
/// module doc for why this is a deliberate port, not a fresh design.
const ENC_TABLE: [[i32; 4]; 4] = [
    [0, -1, 1, 0],
    [1, 0, 0, -1],
    [-1, 0, 0, 1],
    [0, 1, -1, 0],
];

/// The ISR-shared quadrature decode state: everything
/// [`EncoderIsrState::on_edge`] (running in GPIO-interrupt context) and
/// [`EncoderIsrState::take_delta`] (running in [`RotaryEncoderInput::poll`],
/// i.e. the normal task context) both touch. `Arc`'d so both the two
/// per-pin ISR closures and the owning `RotaryEncoderInput` can hold a
/// reference; every field is lock-free (`Atomic*`) since an ISR cannot
/// block on a mutex.
///
/// Mirrors `factory.cpp`'s `FactoryState::encRaw`/`encPrevAB` (`volatile
/// int32_t`/`volatile uint8_t` fields updated from `onEncoderChange()`,
/// drained by the main loop via `takeEncoderDelta()`) — `Ordering::Relaxed`
/// throughout matches the C code's plain `volatile` semantics: this is a
/// single monotonic accumulator with one drainer, not a multi-field
/// invariant that needs acquire/release fencing.
struct EncoderIsrState {
    /// Running raw quadrature-transition accumulator, in HALF-STEPS (2
    /// raw units = 1 logical detent tick) — matches `takeEncoderDelta`'s
    /// `/ 2` convention exactly (see [`EncoderIsrState::take_delta`]),
    /// empirically tuned by LilyGo for this specific EC11 module's
    /// detent behavior.
    raw: AtomicI32,
    /// The last-seen 2-bit `(lead << 1 | lag)` pin state, for the
    /// transition table lookup on the next edge.
    prev_state: AtomicU8,
    /// The physical GPIO fed as the table's "leading" bit.
    ///
    /// **Direction, round 2** (bead ai-bitwarden-hw-key-bgl): the first
    /// GPIO-interrupt decoder revision fed `ENCODER_PIN_B` (GPIO5) here,
    /// carrying over the *assumption* that the old `rotary-encoder-hal`
    /// `Rotary::new(b, a)` swap's correction would transfer unchanged to
    /// this completely different decode algorithm. Hardware testing
    /// proved that assumption wrong — direction came out backwards
    /// again. `ENC_TABLE` is antisymmetric (`ENC_TABLE[x][y] ==
    /// -ENC_TABLE[y][x]` for every `x`/`y`), so swapping which pin feeds
    /// `lead` vs `lag` exactly negates every decoded delta; going back
    /// to the vendor's own natural `ENCODER_INA`(a)/`ENCODER_INB`(b)
    /// order here (`lead = a`/GPIO4, `lag = b`/GPIO5) is therefore the
    /// other, opposite direction from the previous (wrong) swap, and is
    /// what should now make CW decode as `NavIntent::Next`. Pending the
    /// human's on-hardware confirmation.
    lead_pin: i32,
    /// The physical GPIO fed as the table's "lagging" bit — see
    /// `lead_pin`'s doc comment.
    lag_pin: i32,
}

impl EncoderIsrState {
    /// Reads both quadrature pins, looks up the signed transition delta,
    /// accumulates it, and re-arms both pins' interrupts.
    ///
    /// # Safety / ISR context
    ///
    /// Called only from the GPIO ISR callbacks [`RotaryEncoderInput::new`]
    /// installs on `lead_pin`/`lag_pin` (both already configured as
    /// pulled-up inputs before any callback can fire). The two raw FFI
    /// calls are both standard ESP-IDF GPIO driver primitives documented
    /// as ISR-safe:
    /// - `gpio_get_level` is the same raw register read Arduino's
    ///   `digitalRead` (and `factory.cpp`'s `onEncoderChange`) uses.
    /// - `gpio_intr_enable` re-arms the interrupt immediately, matching
    ///   Arduino's `attachInterrupt(..., CHANGE)` continuous-refire
    ///   semantics. This is deliberately the raw driver call, not
    ///   `esp_idf_hal::gpio::PinDriver::enable_interrupt` (which wraps
    ///   the heavier `gpio_isr_handler_add` — reinstalling a handler
    ///   that's already installed — and esp-idf-hal's own doc comment on
    ///   `enable_interrupt` says to call it "from a non-ISR context").
    ///   `gpio_intr_enable` is the exact primitive ESP-IDF's own internal
    ///   GPIO ISR dispatch uses for level-triggered interrupts' automatic
    ///   re-arming, and is safe to call for an edge-triggered interrupt
    ///   specifically because (unlike a level-triggered one) the
    ///   condition that fired it (an edge) cannot still be "true" a
    ///   moment later — the pin is now at a stable level until the next
    ///   real edge, so re-arming immediately cannot cause an infinite
    ///   ISR storm the way it could for `LowLevel`/`HighLevel`.
    fn on_edge(&self) {
        // SAFETY: see this method's doc comment.
        let lead = unsafe { gpio_get_level(self.lead_pin) };
        let lag = unsafe { gpio_get_level(self.lag_pin) };
        let cur_state = ((lead << 1) | lag) as u8;

        let prev_state = self.prev_state.load(Ordering::Relaxed);
        self.raw.fetch_add(ENC_TABLE[prev_state as usize][cur_state as usize], Ordering::Relaxed);
        self.prev_state.store(cur_state, Ordering::Relaxed);

        // SAFETY: see this method's doc comment.
        unsafe {
            gpio_intr_enable(self.lead_pin);
            gpio_intr_enable(self.lag_pin);
        }
    }

    /// Drains whatever has accumulated since the last call, in whole
    /// logical ticks. Matches `factory.cpp`'s `takeEncoderDelta`
    /// exactly: divides the raw half-step accumulator by 2, and only
    /// ever consumes an exact multiple of 2 raw units (`delta * 2`) so a
    /// dangling half-tick remainder is never lost between calls — it
    /// stays in `raw` to be combined with the next edge instead.
    fn take_delta(&self) -> i32 {
        let raw = self.raw.load(Ordering::Relaxed);
        let delta = raw / 2;
        if delta != 0 {
            self.raw.fetch_sub(delta * 2, Ordering::Relaxed);
        }
        delta
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
pub struct RotaryEncoderInput {
    /// Kept alive for as long as this `RotaryEncoderInput` is: dropping
    /// either would tear down its pull-up/interrupt configuration and
    /// (per `esp-idf-hal`'s `PinDriver::unsubscribe`-on-drop behavior)
    /// the ISR subscription itself. Never read directly after `new` —
    /// `EncoderIsrState`'s ISR closures read the pin levels via raw
    /// `gpio_get_level` calls by pin number instead (see
    /// `EncoderIsrState::on_edge`), not through these `PinDriver`
    /// handles — so these fields exist purely to keep the underlying
    /// GPIO/interrupt configuration alive, hence `#[allow(dead_code)]`.
    #[allow(dead_code)]
    pin_a: PinDriver<'static, Gpio4, Input>,
    #[allow(dead_code)]
    pin_b: PinDriver<'static, Gpio5, Input>,
    encoder: Arc<EncoderIsrState>,
    button: Button<PinDriver<'static, Gpio0, Input>, Instant>,
}

impl RotaryEncoderInput {
    /// # Errors
    ///
    /// Returns `EspError` if any of the three GPIOs can't be configured
    /// as pulled-up digital inputs, or if the interrupt subscription
    /// fails to install.
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

        // Direction: see `EncoderIsrState::lead_pin`'s doc comment for
        // the full history. `lead = a` (GPIO4/ENCODER_PIN_A), `lag = b`
        // (GPIO5/ENCODER_PIN_B) — the vendor's own natural order, the
        // opposite of the first GPIO-interrupt decoder revision's (wrong)
        // swap.
        let initial_lead = i32::from(a.is_high());
        let initial_lag = i32::from(b.is_high());
        let initial_state = ((initial_lead << 1) | initial_lag) as u8;

        let encoder = Arc::new(EncoderIsrState {
            raw: AtomicI32::new(0),
            prev_state: AtomicU8::new(initial_state),
            lead_pin: a.pin(),
            lag_pin: b.pin(),
        });

        a.set_interrupt_type(InterruptType::AnyEdge)?;
        b.set_interrupt_type(InterruptType::AnyEdge)?;

        // SAFETY: the closures only touch `Arc<EncoderIsrState>` (atomics
        // + plain `i32`s, `Send + Sync`) and call the two ISR-safe raw
        // FFI primitives documented on `EncoderIsrState::on_edge` — no
        // heap allocation, no locking, no STD/libc/FreeRTOS calls beyond
        // those two, matching `PinDriver::subscribe`'s safety contract.
        unsafe {
            let encoder_for_a = Arc::clone(&encoder);
            a.subscribe(move || encoder_for_a.on_edge())?;
            let encoder_for_b = Arc::clone(&encoder);
            b.subscribe(move || encoder_for_b.on_edge())?;
        }
        a.enable_interrupt()?;
        b.enable_interrupt()?;

        Ok(Self {
            pin_a: a,
            pin_b: b,
            encoder,
            button: Button::new(
                btn,
                ButtonConfig {
                    mode: button_driver::Mode::PullUp,
                    ..Default::default()
                },
            ),
        })
    }
}

impl InputSource for RotaryEncoderInput {
    fn poll(&mut self) -> Vec<NavIntent> {
        let mut intents = Vec::new();

        // Drain whatever the ISR accumulated since the last poll — could
        // be 0 (no rotation), a small number (normal-speed rotation), or
        // several ticks at once if the encoder spun fast enough to
        // outrun this frame's ~33ms budget (which the frame-rate-polled
        // predecessor design would have aliased/miscounted instead of
        // ever actually observing all of; see the module doc). Every
        // tick emits exactly one `Next`/`Prev` — no acceleration/`NextN`
        // upgrade (deliberately removed, see the module doc's "No
        // fast-rotation acceleration" section).
        let delta = self.encoder.take_delta();
        let intent = if delta > 0 {
            Some(NavIntent::Next)
        } else if delta < 0 {
            Some(NavIntent::Prev)
        } else {
            None
        };
        if let Some(intent) = intent {
            for _ in 0..delta.unsigned_abs() {
                intents.push(intent);
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
