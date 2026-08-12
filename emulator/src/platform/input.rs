//! Host `InputSource`s: [`WindowedInput`] (keyboard, via the shared
//! `minifb::Window` — see `super::minifb_surface` for why it's shared),
//! [`HttpInput`] (headless mode, fed by `POST /api/input` — see
//! `emulator::desktop::http_server::SyncServer`), and [`NoopInput`] (a
//! minimal placeholder `InputSource` for callers/tests that need *some*
//! implementation without wiring up either of the above).
//!
//! Mapping (per the bead): arrow up / scroll up -> `Prev`, arrow down /
//! scroll down -> `Next`, Enter -> `Activate`, Backspace or Escape ->
//! `Back`.
//!
//! The key -> intent mapping is edge-triggered (fires once per press, not
//! once per frame the key is held), the same pattern
//! `desktop::input::DesktopInput` already uses for the old `KeyCode`
//! engine. The edge-detection itself ([`edge_triggered_intents`]) is a
//! pure function over a "what's down right now" snapshot, independent of
//! `minifb::Window`, specifically so it's unit-testable without opening a
//! real OS window.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use bhk_core::input::NavIntent;
use bhk_core::platform::InputSource;
use minifb::{Key, Window};

/// The fixed set of keys this project maps to a `NavIntent`, and which
/// intent each maps to on press.
const KEY_INTENTS: &[(Key, NavIntent)] = &[
    (Key::Up, NavIntent::Prev),
    (Key::Down, NavIntent::Next),
    (Key::Enter, NavIntent::Activate),
    (Key::Backspace, NavIntent::Back),
    (Key::Escape, NavIntent::Back),
];

/// Pure edge-detection: given the current down/up state of each mapped
/// key, updates `previous` in place and returns the `NavIntent`s for keys
/// that transitioned from up to down since the last call. Order follows
/// `KEY_INTENTS`.
fn edge_triggered_intents(currently_down: &HashMap<Key, bool>, previous: &mut HashMap<Key, bool>) -> Vec<NavIntent> {
    let mut intents = Vec::new();
    for (key, intent) in KEY_INTENTS {
        let now = currently_down.get(key).copied().unwrap_or(false);
        let was = previous.get(key).copied().unwrap_or(false);
        if now && !was {
            intents.push(*intent);
        }
        previous.insert(*key, now);
    }
    intents
}

/// Scroll-wheel deltas below this magnitude are treated as noise, not an
/// intentional scroll tick (trackpads in particular report tiny
/// continuous deltas).
const SCROLL_DEADZONE: f32 = 0.1;

pub struct WindowedInput {
    window: Rc<RefCell<Window>>,
    key_states: HashMap<Key, bool>,
}

impl WindowedInput {
    #[must_use]
    pub fn new(window: Rc<RefCell<Window>>) -> Self {
        Self { window, key_states: HashMap::new() }
    }
}

impl InputSource for WindowedInput {
    fn poll(&mut self) -> Vec<NavIntent> {
        let currently_down: HashMap<Key, bool> = {
            let window = self.window.borrow();
            KEY_INTENTS.iter().map(|(key, _)| (*key, window.is_key_down(*key))).collect()
        };
        let mut intents = edge_triggered_intents(&currently_down, &mut self.key_states);

        let scroll_dy = self.window.borrow().get_scroll_wheel().map_or(0.0, |(_, dy)| dy);
        if scroll_dy > SCROLL_DEADZONE {
            intents.push(NavIntent::Prev);
        } else if scroll_dy < -SCROLL_DEADZONE {
            intents.push(NavIntent::Next);
        }

        intents
    }
}

/// `InputSource` fed by `POST /api/input` (W5): drains whatever
/// `NavIntent`s an agent has queued over HTTP since the last poll. The
/// queue is an `Arc<Mutex<VecDeque<NavIntent>>>` shared with a
/// `SyncServer` (see
/// `emulator::desktop::http_server::SyncServer::get_input_queue_ref`) —
/// the HTTP server thread pushes onto the back as `POST /api/input`
/// requests arrive, this `poll()` (called every frame from the render-loop
/// thread) drains the whole queue in FIFO order. Mirrors the
/// `Arc<Mutex<Vec<Credential>>>` sharing pattern `PushSyncSource` already
/// uses for credentials.
pub struct HttpInput {
    queue: Arc<Mutex<VecDeque<NavIntent>>>,
}

impl HttpInput {
    /// Wraps the shared queue handle a `SyncServer` hands out via
    /// `get_input_queue_ref()`. Cloning the `Arc` means this `HttpInput`
    /// always drains whatever has been queued up to the moment of each
    /// `poll()` call, including intents injected after construction.
    #[must_use]
    pub fn new(queue: Arc<Mutex<VecDeque<NavIntent>>>) -> Self {
        Self { queue }
    }
}

impl InputSource for HttpInput {
    fn poll(&mut self) -> Vec<NavIntent> {
        self.queue.lock().unwrap().drain(..).collect()
    }
}

/// Minimal placeholder `InputSource`: always empty. Useful for tests and
/// call sites (e.g. `host_platform`'s own unit test) that need *some*
/// `InputSource` to satisfy the `Platform` bundle without wiring up either
/// `WindowedInput` or `HttpInput`.
#[derive(Debug, Default)]
pub struct NoopInput;

impl NoopInput {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl InputSource for NoopInput {
    fn poll(&mut self) -> Vec<NavIntent> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_transitioning_from_up_to_down_fires_its_intent_once() {
        let mut previous = HashMap::new();

        let mut down = HashMap::new();
        down.insert(Key::Down, true);
        let intents = edge_triggered_intents(&down, &mut previous);
        assert_eq!(intents, vec![NavIntent::Next]);

        // Held down on the next poll: no repeat.
        let intents_while_held = edge_triggered_intents(&down, &mut previous);
        assert!(intents_while_held.is_empty());
    }

    #[test]
    fn releasing_and_repressing_fires_again() {
        let mut previous = HashMap::new();
        let mut down = HashMap::new();
        down.insert(Key::Enter, true);
        edge_triggered_intents(&down, &mut previous);

        down.insert(Key::Enter, false);
        let released = edge_triggered_intents(&down, &mut previous);
        assert!(released.is_empty());

        down.insert(Key::Enter, true);
        let repressed = edge_triggered_intents(&down, &mut previous);
        assert_eq!(repressed, vec![NavIntent::Activate]);
    }

    #[test]
    fn all_five_mapped_keys_produce_the_expected_intents() {
        let mut previous = HashMap::new();
        let mut down = HashMap::new();
        for (key, _) in KEY_INTENTS {
            down.insert(*key, true);
        }
        let mut intents = edge_triggered_intents(&down, &mut previous);
        intents.sort_by_key(|intent| format!("{intent:?}"));

        let mut expected = vec![
            NavIntent::Prev,
            NavIntent::Next,
            NavIntent::Activate,
            NavIntent::Back, // Backspace
            NavIntent::Back, // Escape
        ];
        expected.sort_by_key(|intent| format!("{intent:?}"));

        assert_eq!(intents, expected);
    }

    #[test]
    fn noop_input_never_produces_intents() {
        let mut input = NoopInput::new();
        assert!(input.poll().is_empty());
        assert!(input.poll().is_empty());
    }

    #[test]
    fn http_input_polls_empty_when_nothing_has_been_queued() {
        let mut input = HttpInput::new(Arc::new(Mutex::new(VecDeque::new())));
        assert!(input.poll().is_empty());
    }

    #[test]
    fn http_input_drains_queued_intents_in_fifo_order_and_is_empty_after() {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        queue.lock().unwrap().push_back(NavIntent::Next);
        queue.lock().unwrap().push_back(NavIntent::Activate);
        let mut input = HttpInput::new(Arc::clone(&queue));

        let polled = input.poll();

        assert_eq!(polled, vec![NavIntent::Next, NavIntent::Activate]);
        assert!(queue.lock().unwrap().is_empty(), "poll must drain, not just read, the queue");
        assert!(input.poll().is_empty(), "a second poll with nothing new queued is empty");
    }

    #[test]
    fn http_input_reflects_intents_queued_after_construction() {
        // The whole point of holding an `Arc` rather than a snapshot: the
        // HTTP server thread pushes onto the same `Mutex` concurrently as
        // requests arrive, and `HttpInput` must see those pushes on its
        // next `poll()` without being reconstructed.
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let mut input = HttpInput::new(Arc::clone(&queue));

        assert!(input.poll().is_empty());

        queue.lock().unwrap().push_back(NavIntent::Back);

        assert_eq!(input.poll(), vec![NavIntent::Back]);
    }
}
