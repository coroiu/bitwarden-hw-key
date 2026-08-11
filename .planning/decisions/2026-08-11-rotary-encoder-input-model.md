# Rotary Encoder Input Model and Navigation Intent

**Date**: 2026-08-11
**Status**: Accepted

## Context

The migration from 3-button navigation (HUZZAH32) to rotary encoder (T-Embed) requires a new input abstraction that is:

1. **Hardware-agnostic**: the app shouldn't know or care whether input comes from buttons, encoder, keyboard, or headless HTTP injection.
2. **Semantic**: raw hardware events (rotation ticks, press duration) should be converted to high-level navigation intent (next, previous, activate, back).
3. **Extensible**: future input methods (3-button or capacitive) should work by implementing the same interface.
4. **Large-vault acceleration**: the encoder's continuous rotation should enable quick browsing of large credential lists without scrolling one item at a time.

The current focus-management system (`2026-01-21-focus-management-system.md`) operates at the level of `FocusEvent` (high-level). The transport layer (how focus changes are triggered) is now being replaced.

## Decision

Introduce a two-tier input model:

### Tier 1: Raw Platform Events
Platform-specific input drivers emit raw events:
- **Encoder**: `RotaryEvent { ticks: i16 }` (positive = CW, negative = CCW)
- **Encoder button**: `ButtonEvent { pressed: bool, duration_ms: u32 }`
- **Keyboard (desktop)**: `KeyboardEvent { key: KeyCode }`
- **HTTP (headless)**: `HttpInputEvent { intent: NavIntent }` (injected intents directly)

These remain driver-local and platform-specific.

### Tier 2: Semantic Navigation Intent
All raw events are mapped to a unified `NavIntent` enum:

```rust
pub enum NavIntent {
    Next,            // Move to next item (encoder CW, arrow down, button down)
    Prev,            // Move to previous item (encoder CCW, arrow up, button up)
    NextN(u16),      // Jump forward N items (fast encoder rotation, held down button, Pg Dn)
    Activate,        // Select focused item (encoder short press, space, enter)
    Back,            // Return to parent / modal dismiss (encoder long press, esc, back button)
}
```

**Encoder-specific mapping**:
- **Single rotation tick** (±1) → `Next` or `Prev`
- **Fast rotation** (≥4 ticks in 100ms) → `NextN(min(ticks * 2, 16))` (accelerates browsing, capped at 16 to prevent jumps larger than one screen)
- **Short press** (<500ms) → `Activate`
- **Long press** (≥500ms) → `Back`

**Desktop keyboard mapping**:
- Arrow Up → `Prev`
- Arrow Down → `Next`
- Page Up → `Prev` (or `NextN(5)`)
- Page Down → `Next` (or `NextN(5)`)
- Space / Enter → `Activate`
- Esc → `Back`

**Headless HTTP injection**:
- Endpoint `POST /api/input` accepts a JSON `NavIntent` and injects it directly into the input queue. This allows agents to drive the app without simulating hardware.

### App-Level Integration

The app's input handler receives `NavIntent` and routes to the document (via existing focus-management system):
- `Next` / `Prev` → `document.focus_next()` / `document.focus_previous()`
- `NextN(n)` → `document.focus_next_n(n)` (new method; skips n focusable items, clipped to bounds)
- `Activate` → `document.activate_focus()` (dispatches `FocusEvent::Activated`)
- `Back` → handled at document or app level (closes modals, dismisses detail views, etc.)

This **keeps the high-level focus-management system intact** (`FocusEvent`, opt-in focusability, auto-scroll) while **replacing the transport layer** that triggers focus changes.

## Rationale

- **Hardware independence**: app code never mentions "encoder" or "keyboard." It's always `NavIntent`.
- **Large-vault support**: `NextN` enables rapid scrolling (roadmap Open Question 6); without it, rotating through 500 items would require 500 UI frames.
- **Testability**: agents inject `NavIntent` directly (no need to simulate encoder hardware). Headless tests are fast and deterministic.
- **Desktop development**: keyboard is fast and intuitive; no need to click or drag in the emulator window.
- **Extensibility**: adding a 3-button input driver is a one-driver addition (map buttons to `NavIntent`), not a redesign.
- **Headless integrity**: injecting intents (not raw hardware events) ensures agents drive the app through the same semantic path as hardware users would.

## Alternatives Considered

- **Pass raw hardware events to the app.** App decides how to interpret encoder rotation and button press.
  - **Pros**: maximum app flexibility.
  - **Cons**: app depends on hardware specifics; encoder and 3-button UX would diverge; harder to test without hardware.
  - **Verdict**: Rejected. Semantic intent is the clean abstraction.

- **Keep the existing KeyCode transport (Up, Middle, Down).** Map encoder to virtual buttons.
  - **Pros**: minimal changes to existing code.
  - **Cons**: doesn't support acceleration (`NextN`); keycode semantics don't match encoder semantics (no notion of rotation speed).
  - **Verdict**: Rejected. `NavIntent` is more expressive.

- **Implement acceleration in the focus manager (app level).** Raw events go to the app; the app detects fast changes and jumps.
  - **Pros**: simpler input layer.
  - **Cons**: app is burdened with input timing logic; encoder driver doesn't know how the app interprets fast rotation.
  - **Verdict**: Rejected. Acceleration is a platform detail; let the input driver handle it.

## Consequences

### Positive
- Encoder rotation feels responsive and natural; fast spin jumps through lists.
- Agents can test UI without hardware by injecting `NavIntent`.
- Adding a different input method (buttons, capacitive) is isolated to the input driver.
- Focus-management system is unchanged and continues to work.

### Negative
- New `NavIntent` enum and input-driver layer to implement.
- Encoder timing logic (fast rotation detection) adds platform-specific complexity.
- App code must support `focus_next_n()` for acceleration (new method; simple but new).

## Implementation Notes

- The `InputSource` trait (from `2026-08-11-presentation-surface-run-mode-seam.md`) polls raw platform events and returns `Vec<InputEvent>` (could be `NavIntent` or raw events). An adapter layer converts raw → `NavIntent`.
- Or: `InputSource` directly returns `Vec<NavIntent>` (simpler; raw events don't leave the platform layer).
- Headless `POST /api/input` accepts JSON `NavIntent` and queues it. This is the agent injection point.

## Testing

- Desktop emulator: keyboard → `NavIntent` → document focus changes.
- Headless HTTP: `curl -X POST http://localhost:8080/api/input -d '{"intent": "Next"}'` → app responds.
- Real target: rotary encoder → `NavIntent` → document focus changes.

## References

- Owners: Fern (fe-architect), Ruby (rust-embedded-supervisor)
- Related decisions: [2026-08-11-presentation-surface-run-mode-seam.md](2026-08-11-presentation-surface-run-mode-seam.md), [2026-01-21-focus-management-system.md](2026-01-21-focus-management-system.md) (transport superseded; high-level focus events retained)
- Roadmap Open Question 6 (large vaults: enabled by `NextN` acceleration)
