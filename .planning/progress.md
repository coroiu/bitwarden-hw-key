# Project Progress

**Last Updated**: 2026-08-11 (Vision re-grounded, pivoting to M0 platform migration)

## Current Status
The project vision has been re-grounded to focus on the Lilygo T-Embed (ESP32-S3, 320x170 color ST7789, rotary encoder) as the first concrete target. The old 128x32 mono OLED prototype work is complete but treated as throwaway pending a green-field GUI redesign. Current focus: **M0 (Platform Migration)**, which has not started yet. M0 will be run as a beads epic and includes architect-led UI framework design, formalization of the three run modes (headless, windowed, real-target), and an SDK feasibility spike.

## Completed (Prototype Era: 128x32 Mono OLED + 3-Button Navigation)
Historical work from the initial Adafruit HUZZAH32 prototype. Treated as throwaway pending M0 green-field redesign. Salvageable components: credential data model, sync/storage layer concept.

- 2026-01-22: Phase 1.3 Credential List View complete
  - Created create_credential_list_view() function in simple_view.rs
  - Format: "Name (username)" for each credential
  - Empty state shows "No credentials" / "Sync from vault" (items marked unfocusable)
  - Desktop emulator recreates view when credentials change
  - Initial view loads credentials from storage on startup
  - Tested with 8 credentials - scrolling works perfectly
  - Added HTTP shutdown endpoint (POST /api/shutdown)
  - Implemented marquee text scrolling for long credential names:
    - Text automatically scrolls horizontally when focused and exceeds available width
    - Scroll speed: SCROLL_SPEED=3 frames (tunable constant)
    - Pauses at beginning and end before wrapping around
    - Resets to start when item loses focus
    - Uses character-skipping approach (skips chars from left as offset increases)
    - Added 1px horizontal margins to prevent text from touching borders
    - Note: Current text rendering system doesn't support true clipping; may need refactor later
- 2026-01-22: Phase 1.2 Storage complete
  - Added chrono dependency for timestamps
  - Created DesktopStorage module (src/desktop/storage.rs)
  - Implements JSON file storage at ./data/credentials.json
  - Credentials automatically loaded on startup
  - Credentials persisted on sync and cleared on clear
  - Added last_sync timestamp tracking
  - Added /data directory to .gitignore
  - Updated CLAUDE.md with safe emulator management practices
  - Tested persistence across emulator restarts
- 2026-01-22: Phase 1.1 Foundation complete
  - Added tiny_http, ciborium, serde, and serde_json dependencies
  - Created credential data model (Credential, SyncRequest, SyncResponse) in src/credentials/mod.rs
  - Implemented HTTP server with three endpoints:
    - POST /api/sync - Accepts CBOR-encoded credentials
    - GET /api/status - Returns server status and credential count
    - POST /api/clear - Clears stored credentials
  - Integrated HTTP server into desktop emulator (runs on localhost:8080)
  - Desktop emulator detects credential changes from HTTP sync
  - Created json_to_cbor example tool for testing
  - Successfully tested all endpoints with curl
- 2026-01-22: Defined Phase 1 roadmap and testable use-case
  - Decided on keyboard emulation first (BLE HID), FIDO2 in Phase 2
  - Created comprehensive technical design document for Phase 1
  - Designed emulator HTTP protocol (desktop runs server, Web Vault connects)
  - Updated roadmap with 4-phase approach and 4-week Phase 1 timeline
  - Documented architectural decisions in three new ADRs
- 2026-01-22: Completed comprehensive research on ESP32 NVS storage and BLE HID keyboard
  - Documented NVS API usage with esp-idf-svc Rust bindings
  - Researched storage capacity limitations and best practices
  - Identified esp32-nimble as the primary BLE stack for Rust
  - Documented BLE HID protocol requirements and specifications
  - Created implementation recommendations and phased roadmap
  - Updated references with all discovered resources
- 2026-01-21: Implemented focus management system for simple_gui
  - Created FocusEvent enum (Gained, Lost, Activated) for high-level focus events
  - Extended Component trait with focus methods (is_focusable, on_focus_event, on_input)
  - Added Document focus tracking and navigation (focus_next, focus_previous)
  - Implemented VerticalMenu focus handling with internal selection management
  - Added auto-scrolling to keep selected items visible in viewport
  - Created visual selection feedback with white borders on menu items
  - Fully integrated keyboard input through desktop emulator
- 2026-01-21: Implemented desktop emulation with minifb
  - Created separate binary for desktop development
  - Added keyboard input mapping (Arrow Up/Down, Space)
  - Implemented 8x scaling (128x32 → 1024x256 window)
  - Zero bloat on ESP32 binary through target-specific dependencies
- 2024-05-31: Implemented vertical menu rendering (commit dc52222)
- 2024-05-31: Added "hello world" label component (commit e62c78a)
- 2024-05-31: Ported render functionality (commit 22a74a5)
- 2024-05-31: Scaffolded lifecycle update functions (commit ae7c3fb)
- 2024-05-31: Scaffolded component creation (commit afa3e57)
- ESP32 platform setup with esp-rs framework
- OLED display driver integration (128x32 SSD1306)
- Basic GUI component system

## In Progress

### M0 (Platform Migration): Design Complete, Foundation Implementation Underway

**2026-08-11**: M0 foundational architecture has been designed and formally documented. Five M0 decisions (ADRs) recorded in `.planning/decisions/`:

1. **Presentation Surface and Run-Mode Seam** — Platform abstraction with four injected traits (DisplaySurface, InputSource, Clock, Storage). Canonical pixel format: Rgb565 throughout the core. Headless, windowed, and real-target differ only in surface implementation.
2. **Portability Boundary and Workspace Split** — Three-layer Cargo workspace (`core` / `firmware` / `emulator`) enforcing platform separation at compile time. No platform-specific dependencies in the core; trait implementations live in platform-specific crates.
3. **Rotary Encoder Input Model and Navigation Intent** — Two-tier input abstraction: raw platform events → semantic `NavIntent` enum. Encoder mapping: rotation → Next/Prev, fast rotation → NextN (acceleration), short press → Activate, long press → Back. Headless agents inject `NavIntent` directly; app is hardware-agnostic.
4. **Sync Source Abstraction and Deferred SDK Decision** — `SyncSource` trait allows `PushSyncSource` (fallback, works now) and `SdkSyncSource` (spike research) to coexist. M0 uses push; spike (W8) runs in parallel; final choice (SDK viable or fallback) recorded in post-spike ADR.
5. **UI Framework: Retire Both Existing GUIs, Rewrite Clean** — Both `src/gui/` (dead code) and `src/simple_gui/` (architecturally incompatible with color) are deprecated. Rewrite on `embedded-graphics` + `embedded-graphics-framebuf` + `u8g2-fonts` + `mipidsi`. Salvage high-level concepts only (navigation stack, ComponentAction, FocusEvent); re-implement cleanly. Layout: fixed chrome + linear stacks, no flexbox.

Beads epic `ai-bitwarden-hw-key-8d7` created with workstreams:
- **W1 (complete)**: Record M0 ADRs and planning updates (this bead).
- **W2**: Implement platform traits and workspace split (Ruby).
- **W3**: Build new GUI framework on embedded-graphics (Fern, Ruby).
- **W4**: Implement three run modes (headless, windowed, real-target) (Tess, Ruby).
- **W5**: Port to T-Embed hardware and verify (Ruby, Tess).
- **W6–W7**: UI design (rotary encoder UX, color layout) (Uma).
- **W8 (parallel)**: Bitwarden Rust SDK feasibility spike (Ruby).
- **W9**: M0 integration and closure.

Next: W2–W7 (foundation implementation) proceed in parallel; W8 (spike) runs independently.

## Next Steps

### M0 (Platform Migration) Breakdown
This will be structured as a beads epic once Ada (architect) and Fern (fe-architect) complete their green-field design. Anticipated work domains:

1. **UI Framework Design & Implementation** (Ada, Fern): Decide on reuse vs rewrite vs OSS. Design the component model, layout engine, and input/navigation pipeline for color display + rotary encoder.
2. **Three Run Modes Formalization** (Tess): Implement headless (agent-driven, screenshot-captured), windowed (minifb, human), and real-target (T-Embed) harnesses as equivalent, testable code paths.
3. **SDK Feasibility Spike** (Ruby): Can the Bitwarden Rust SDK (async, HTTP, TLS, KDF) link and fit on ESP32-S3? Identify unlock latency risks and session-key design needs.
4. **Empty UI Shell on Hardware**: Get a minimal but real color UI running and drivable via rotary encoder on both emulator and T-Embed hardware.

See `.planning/roadmap.md` for the full milestone sequence and gating questions.

## Blockers
None currently identified

## Notes
- **2026-08-11 Reconciliation**: Vision re-grounded. Old prototype (128x32 SSD1306, 3-button nav, simple_gui) is complete as historical work but treated as throwaway pending M0 green-field redesign. Roadmap updated to milestone-based structure (M0 through M4) centered on T-Embed first target and color + rotary encoder UX. Work will proceed via beads epic workflow.
- WIP.md has been migrated to this progress tracking system (2026-01-21)
- Project structure modernized with .planning and .research directories (2026-01-21)
