use serde::{Deserialize, Serialize};

/// Semantic navigation intent: the hardware-independent "Tier 2" input
/// abstraction the app core reacts to. Raw platform events (encoder ticks,
/// button clicks, keyboard keys, headless HTTP JSON) are mapped to this
/// enum by the platform/emulator layer; app code never mentions "encoder"
/// or "keyboard."
///
/// This is a placeholder module seam only: nothing in the core wires this
/// up to the (still-old) GUI engines yet, and no `InputSource` implementation
/// exists. It exists now so the boundary is visible in the workspace split,
/// per the ADR below.
///
/// See: .planning/decisions/2026-08-11-rotary-encoder-input-model.md
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavIntent {
    /// Move to next item (encoder CW, arrow down, button down).
    Next,
    /// Move to previous item (encoder CCW, arrow up, button up).
    Prev,
    /// Jump forward N items (fast encoder rotation, held button, Pg Dn).
    NextN(u16),
    /// Select the focused item (encoder short press, space, enter).
    Activate,
    /// Return to parent / dismiss modal (encoder long press, esc, back).
    Back,
}
