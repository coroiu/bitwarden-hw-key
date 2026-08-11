# Three Run Modes for Agent-Testable Development

**Date**: 2026-08-11
**Status**: Accepted

**Supporting Decisions**: Implemented via [2026-08-11-presentation-surface-run-mode-seam.md](2026-08-11-presentation-surface-run-mode-seam.md) (display/input abstraction), [2026-08-11-portability-boundary-and-workspace-split.md](2026-08-11-portability-boundary-and-workspace-split.md) (compiler-enforced boundary), and [2026-08-11-rotary-encoder-input-model.md](2026-08-11-rotary-encoder-input-model.md) (headless input injection).

## Context

Development is shifting to a multi-agent workflow where agents (not just Andreas) need to build, run, and verify the device software. Two forces drive this decision:

1. **Agents must be able to test autonomously.** "It compiles" is not proof of behavior. Agents need to run the UI and observe what it actually renders and how it responds to input.
2. **No windows popping up.** Andreas works on other things while agents run. GUI windows appearing unbidden are disruptive, so the default agent-driven mode must be windowless.

At the same time, humans still need a visible way to try the device without buying hardware, and the real device must remain the ground truth. The current desktop emulator (`src/desktop/`, `src/bin/desktop.rs`) uses `minifb` directly and always opens a window, which satisfies the human case but not the agent case.

This requirement lands right before the Lilygo T-Embed migration (128x32 mono → 320x170 color, ST7789; buttons → rotary encoder), which already forces a UI-layer redesign — the right moment to make the presentation surface pluggable.

## Decision

The device software will support **three run modes** over a shared app/render core:

1. **Headless** — renders to an in-memory framebuffer, opens no window. Agents capture PNG screenshots on demand and inject input programmatically (e.g. via the existing HTTP server, extended with input/screenshot endpoints). This is the default mode for automated agent verification.
2. **Windowed** — the `minifb` emulator window, for Andreas and colleagues to test without hardware.
3. **Real target** — the actual T-Embed (ESP32-S3 + ST7789 + rotary encoder), for real use and demos.

The architecture must decouple the **app + render core** (layout, components, input handling, framebuffer production) from the **presentation surface** (headless PNG dump vs `minifb` window vs on-device ST7789). Windowed and headless should differ only in the surface, so a screenshot in headless faithfully represents what the window would show. The detailed design of the surface abstraction and the headless input/screenshot protocol is owned by **Fern (fe-architect)** and implemented by **Ruby**, verified by **Tess**.

## Rationale

- Lets agents prove behavior with visual evidence, closing the "compiles ≠ works" gap.
- Keeps Andreas undisturbed (no surprise windows) during agent-driven work.
- Lowers the barrier for colleagues to try the device (no hardware purchase).
- Sharing a render core across headless/windowed keeps the three modes honest — a headless screenshot is trustworthy because it's the same pipeline as the window.
- Bundling this with the T-Embed UI redesign avoids doing the layout/surface work twice.

## Alternatives Considered

- **Windowed only (status quo).** Simplest, but agents can't verify UI autonomously and windows pop up. Rejected — fails both driving forces.
- **Headless only, no window.** Great for agents, but removes the human-friendly way to try it without hardware. Rejected — colleagues and Andreas lose the easy demo path.
- **Screen-scrape the minifb window.** Capture the OS window instead of a framebuffer. Rejected — still opens a window, is OS-dependent and flaky, and doesn't give a clean programmatic surface.

## Consequences

### Positive
- Agents can run end-to-end verification with screenshots as evidence.
- No disruptive windows during background work.
- Clean surface abstraction makes the on-device (ST7789) target just another surface implementation.

### Negative
- Up-front architecture cost: introducing the surface abstraction and a headless input/screenshot protocol.
- Two host render paths (headless + windowed) to keep in parity; parity must be guarded by tests (Tess's job).
- Slightly more surface area in the emulator's HTTP server (input injection + screenshot endpoints).

## References

- Owners: `.claude/agents/fe-architect.md` (Fern), `.claude/agents/tester.md` (Tess), `.claude/agents/rust-embedded-supervisor.md` (Ruby)
- Related: [2026-01-21-desktop-emulation.md](2026-01-21-desktop-emulation.md), [2026-01-22-emulator-http-protocol.md](2026-01-22-emulator-http-protocol.md)
- To be scheduled as part of the T-Embed migration epic.
