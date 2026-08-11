# UI Framework: Retire Both Existing GUIs, Rewrite Clean on Embedded-Graphics

**Date**: 2026-08-11
**Status**: Accepted

## Context

Two GUI engines exist in the codebase:

**`src/gui/`** — Dead code. An incomplete browser flexbox/DOM engine with:
- Auto-width calculation unsupported (`layout_box.rs:202`)
- Scroll functionality stubbed (`node.rs:127`)
- Never called in the app (every call site commented out at `src/main.rs:119,135,163`)

This was an early attempt to be "principled" with layout algorithms and never matured.

**`src/simple_gui/`** — Shipped on the 128x32 OLED, but architecturally compromised:
- Custom RGBA software rasterizer (`image_buffer.rs`, 4 bytes/px) with no color support
- Font rendering is baked and ASCII-only (`style/font.rs`); '•' renders as '?' (`simple_view.rs:103`)
- Recoloring is unimplemented (`render/canvas.rs:27`)
- Structurally blocks migration to a color display (the rasterizer makes no sense in RGB565 or RGB888)

The T-Embed migration (128x32 mono → 320x170 color, encoder) makes this the natural inflection point: both engines are obsolete, and the new hardware demands a clean redesign. Starting fresh on `embedded-graphics` avoids inheriting technical debt.

## Decision

**Verdict: Retire both `src/gui/` and `src/simple_gui/`.** Rewrite the GUI framework from scratch, targeting `embedded-graphics` as the rendering foundation.

### Salvage (Concepts, not Code)

Retain the high-level design patterns from `simple_gui`, reimplemented cleanly:

1. **Navigation stack** (`simple_gui/document/mod.rs`): a document model tracking a stack of views, supporting back/forward navigation. Keep this; reimplement on `embedded-graphics`.
2. **ComponentAction return value** (`simple_gui/components/mod.rs:21-28`): components return an action (view change, item selection) to the document. Clean abstraction; keep it.
3. **FocusEvent** (`simple_gui/components/mod.rs`): high-level focus state changes (Gained, Lost, Activated). Already formalized in `2026-01-21-focus-management-system.md`; retain it.

### New Framework Architecture

**Rendering target**: `embedded-graphics` + `embedded-graphics-framebuf` for the shared Rgb565 framebuffer (see `2026-08-11-presentation-surface-run-mode-seam.md`).

**Widget trait**:
```rust
pub trait Widget {
    fn draw(&self, target: &mut impl DrawTarget, bounds: Rectangle) -> Result<(), Self::Error>;
    fn on_input(&mut self, intent: NavIntent) -> Option<ComponentAction>;
    fn is_focusable(&self) -> bool { false }
    fn on_focus_event(&mut self, event: FocusEvent) {}
}
```

Widgets draw into an assigned viewport `Rectangle` using `DrawTarget` (from embedded-graphics). Clipping is achieved via `DrawTargetExt::clipped()`, eliminating the need for a custom rasterizer.

**Layout**: fixed chrome regions + linear stacks, NOT a flexbox engine.
- **Chrome**: fixed title bar, content area, hint/status bar.
- **Content**: linear vertical or horizontal stacks (enum-based, not a generic layout engine).
- **No auto-width, no grid, no flex.** Simplicity and compile-time predictability.

**Component library** (reimplement cleanly):
- Label (text)
- VerticalMenu (scrolling list with selection)
- Spacer / Divider
- CredentialListView, CredentialDetailView (domain-specific)

**Font support**: adopt `u8g2-fonts` for vector font rendering. Supports emoji and international glyphs; clean separation from raster baking.

### OSS Integration Strategy

| Dependency | Decision | Rationale |
|------------|----------|-----------|
| `embedded-graphics` | Adopt | De-facto standard for embedded displays. DrawTarget abstraction is clean and battle-tested. |
| `embedded-graphics-framebuf` | Adopt | Provides the Rgb565 framebuffer we need; memory-efficient. |
| `mipidsi` | Adopt | Mature ST7789 driver; integrates with embedded-graphics. |
| `u8g2-fonts` | Adopt | Vector fonts; eliminates custom rasterizer; supports i18n. |
| `embedded-layout` | Adopt | Lightweight layout combinators (optional; can build without if needed). |
| `embedded-menu` | Defer to M1 | Menu widgets with focus and scrolling; premature for M0 if we build VerticalMenu. Not blocking. |
| `slint` | Reject | Declarative UI framework (complex, resource-heavy for embedded, doesn't expose clean DrawTarget, breaks three-mode fidelity story). Overkill for a simple credential browser. |
| `lvgl` | Reject | Feature-rich, large footprint, not Rust-native. Over-engineered for M0. |
| `embedded-graphics-simulator` (SDL2) | Reject | Desktop simulator. Incompatible with our three-mode architecture (introduces an extra rendering path that differs from headless/minifb). Use headless + minifb surface adapters instead. |

### Escalation Path

If hand-rolled linear layout proves insufficient for future views (M1+), the escalation is:
1. Adopt `embedded-layout` combinators (lightweight).
2. If still insufficient, design a custom layout engine specific to our use case (credential lists, detail views) rather than a general-purpose flexbox.
3. **Never** adopt a framework that doesn't expose a clean `DrawTarget` abstraction, as it breaks the shared-framebuffer three-mode story.

## Rationale

- **Dead code debt**: `src/gui/` is purely negative value; removing it simplifies the codebase.
- **Architectural blocker**: `simple_gui`'s custom rasterizer makes color (and thus T-Embed) impossible without a rewrite anyway.
- **Embedded-graphics is proven**: widely adopted in the Rust embedded ecosystem; clean abstractions; active maintenance.
- **Three-mode fidelity**: `embedded-graphics` rendering into a shared framebuffer is platform-agnostic, supporting all three run modes naturally (see `2026-08-11-presentation-surface-run-mode-seam.md`).
- **Reduced complexity**: no custom rasterizer, no baked fonts, no half-finished layout engine. Linear layout is sufficient for M0 and M1.
- **Concept salvage**: the high-level patterns (navigation stack, ComponentAction, FocusEvent) are worth keeping; re-implementing them cleanly is faster than patching the old ones.

## Alternatives Considered

- **Attempt to salvage `simple_gui` with incremental color support.** Extend the rasterizer to RGB565.
  - **Pros**: reuses existing code and structure.
  - **Cons**: the rasterizer design doesn't map to RGB565 efficiently; still need to replace fonts; half measures don't reduce complexity.
  - **Verdict**: Rejected. A rewrite is faster than incremental repair.

- **Adopt `slint` or `lvgl` off-the-shelf.** Reduce custom code.
  - **Pros**: fewer lines of code written.
  - **Cons**: large footprint (ESP32-S3 memory risk); neither exposes a `DrawTarget` (breaks three-mode fidelity); overkill for a list-and-detail app; limited control over rendering and input flow.
  - **Verdict**: Rejected. Custom lightweight framework is justified for this constrained domain.

- **Keep `src/gui/` as a reference and try again later.** Maybe flexbox will be useful for M2+.
  - **Pros**: preserves prior work.
  - **Cons**: dead code accumulates; no signal that it's a real alternative (comment says so, but easy to ignore); clutters the codebase.
  - **Verdict**: Rejected. Delete it. If flexbox is needed later, a fresh implementation will be faster.

## Consequences

### Positive
- No architectural blocker to color (custom rasterizer is gone).
- Cleaner codebase: zero dead code, zero technical debt from prior experiments.
- Simpler font handling: vector fonts from `u8g2-fonts` are production-ready.
- Clear separation of concerns: `embedded-graphics` handles rendering, the framework handles layout and state.
- Replicable: the hand-rolled framework is simple enough for colleagues to understand and extend.
- Proven platform: embedded-graphics is used in many Rust embedded projects; questions/help are easier to find.

### Negative
- Up-front development cost: rewrite is higher effort than patch.
- No WYSIWYG editor or declarative syntax (not a blocker for M0; can be added later).
- Linear layout is inflexible if M2+ adds complex multi-column views (mitigated by escalation path).

## Testing & Verification

- Unit tests: widgets render correctly into a test framebuffer.
- Integration tests: navigation stack behaves correctly (back, forward, actions).
- Screenshot tests (headless): UI looks correct across multiple scenarios (list, detail, empty state).
- Desktop emulator: visual inspection and interactive testing.

## Implementation Roadmap (Part of W1/W2)

1. Create `core/src/gui/` with trait definitions (Widget, ComponentAction, FocusEvent).
2. Implement DisplaySurface + framebuf integration (core foundation).
3. Implement basic widgets (Label, VerticalMenu, Spacer) on embedded-graphics.
4. Integrate navigation stack and FocusEvent.
5. Build CredentialListView and CredentialDetailView (domain-specific).
6. Test on emulator (headless + windowed).
7. Port to real T-Embed hardware.

## References

- Owners: Fern (fe-architect), Ruby (rust-embedded-supervisor), Tess (tester)
- Related decisions: [2026-08-11-presentation-surface-run-mode-seam.md](2026-08-11-presentation-surface-run-mode-seam.md), [2026-08-11-portability-boundary-and-workspace-split.md](2026-08-11-portability-boundary-and-workspace-split.md), [2026-01-21-focus-management-system.md](2026-01-21-focus-management-system.md) (concepts retained)
- Research: [2026-08-11-ui-framework-survey.md](../../.research/findings/2026-08-11-ui-framework-survey.md) — Full analysis of existing engines, OSS survey, and design rationale
- Roadmap Open Question 2 (answered by this decision)
