# Project Progress

**Last Updated**: 2026-08-17 (M1.5 Web Companion Phase 1 complete; end-to-end real vault sync verified on emulator)

## Current Status (2026-08-17)

**M0 (Platform Migration) EMULATOR MILESTONE: COMPLETE**
All nine workstreams (W1–W9) merged to main. Fully operational 320x170 color shell, keyboard-drivable in windowed mode, fully agent-drivable in headless mode via HTTP NavIntent injection. Remaining M0 work: on-hardware validation on the physical T-Embed (bead `ai-bitwarden-hw-key-dvm`, now unblocked by hardware arrival).

**M1 (Vault Browse) DEVICE DISPLAY: COMPLETE**
Credential list and detail views designed and implemented on color + rotary encoder. Full render/nav/input stacks merged to main and verified headless. Real vault sync delivery complete via M1.5 (Web Companion).

**M1.5 (Web Companion) PHASE 1 COMPLETE: END-TO-END REAL VAULT SYNC VERIFIED**
Architecture: local Rust server (axum + tokio) linking the Bitwarden SDK, thin vanilla-JS web UI served over 127.0.0.1, server-owned DeviceTransport (Phase 1: HttpEmulatorTransport via CBOR /api/sync; Phase 2: native BLE/USB to T-Embed). Real flow verified end-to-end: live Bitwarden login with 2FA, vault sync of 24 items, push to emulated device, device renders credential list and detail view with masked passwords. Beads: epic ai-bitwarden-hw-key-eml with children eml.1 through eml.11 delivered. eml.1 (SDK feasibility on host, proven); eml.2 (axum server skeleton + bearer-token boundary); eml.3 (auth: login form, SDK login_password, 2FA support); eml.4 (vault read: SDK sync, metadata-only list); eml.5 (device transport: HttpEmulatorTransport); eml.6 (web UI: vanilla JS, login, vault list, sync button); eml.7 (integration tests); eml.8 (docs); eml.10 (startup banner); eml.11 (login fix). eml.9 and eml.12 closed as superseded/duplicate. Epic eml remains OPEN for Phase 2.

**Key Fix in eml.11:** Real login returned 401 until ClientSettings.bitwarden_client_version was set to Some(CARGO_PKG_VERSION). The live Bitwarden API rejects requests lacking the client-version header. This class of bug surfaces only against the live API, not compile-only checks or dev mocks. Fix committed; real vault sync now fully functional.

**Security Posture in Place (PoC):** Bind 127.0.0.1 only; per-process bearer token; passwords never sent to browser; metadata-only vault list; decrypted vault server-side only, pushed to device during sync, zeroized on lock/logout. A gitignored debug test account exists at data/dev-account.env for end-to-end testing (do not commit; never print the password in docs).

**Next: Phase 2 (native BLE/USB transport to physical T-Embed, firmware sync handler) overlaps M2 (Type it). Hardware has now arrived, so M0 on-hardware validation and Phase 2 are unblocked.**

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

### M0 (Platform Migration): Emulator Milestone Complete (2026-08-12)

**Bead `ai-bitwarden-hw-key-8d7` (epic, OPEN for on-hardware validation)** — all 9 workstreams W1–W9 completed and merged to main as of 2026-08-12:

**Merged Workstreams:**
- **W1**: ADRs and interface freeze recorded (five M0 decisions in `.planning/decisions/`).
- **W2**: Cargo workspace (core/firmware/emulator) with compiler-enforced platform-free bhk_core.
- **W3**: Render core on embedded-graphics (FrameBuffer565 Rgb565, Widget/Navigator, chrome regions). Two render bugs found and fixed with regression tests (W3 sub-row text overflow; c2f windowed blank-window/minifb-pump).
- **W4**: Host surfaces (MinifbSurface, HeadlessSurface, HostClock, FileStorage, WindowedInput).
- **W5**: Headless HTTP NavIntent injection + screenshot capture (verified A→B→C credential-list movement via headless agent-driven screenshot inspection).
- **W6**: T-Embed ESP32-S3 board adapter (St7789 via mipidsi, rotary encoder, NVS) — BUILD-ONLY, physical hardware verification pending.
- **W7**: Unified Platform-generic run() loop + App + drivable credential-list shell. Old simple_gui/gui engines deleted.
- **W8**: SDK feasibility spike = NO-GO on-device (ring won't link on xtensa; bitwarden-crypto not modular). Outcome: pivot to companion-app push model (decision recorded in ADR).
- **W9**: SyncSource + PushSyncSource trait implementations.

**Emulator Milestone (Done):**
- Empty-but-real 320x170 color shell running and rendering correctly.
- Keyboard-drivable in windowed mode (verified by live screencapture).
- Fully agent-drivable + observable in headless mode via HTTP NavIntent injection and screenshot capture.
- Credential-list shell operational (A→B→C selection movement verified).

**On-Hardware Validation (Pending):**
- Physical T-Embed board verification. W6 is build-only; no hardware available yet. Tracked in bead `ai-bitwarden-hw-key-dvm` (on-device verification when hardware arrives).
- M0 epic remains OPEN until on-hardware drivability is confirmed.

**Open Follow-Up Beads:**
- `ai-bitwarden-hw-key-dvm`: On-device verification when T-Embed hardware arrives.
- `ai-bitwarden-hw-key-ci0`: Adopt u8g2-fonts for rendering.
- `ai-bitwarden-hw-key-5c8`: Broaden test coverage.
- `ai-bitwarden-hw-key-7h7`: Host-build ergonomics (workspace default target esp32s3 forces --target; fix README build commands).
- `ai-bitwarden-hw-key-1sg`: On-device SDK via private fork (DEFERRED, conditional on portable-vault validation).

---

### M1.5 (Web Companion) Epic Kickoff (2026-08-12)

**Bead ai-bitwarden-hw-key-eml (epic, OPEN)** — Web companion development. Phases:

**Phase 0 (Complete, 2026-08-12):**
- SDK feasibility spike: proven Bitwarden Rust SDK (bitwarden/sdk-internal@99ffb6ef) links on macOS host in isolated nested `web-companion/` workspace
- Isolation verified: SDK dependencies (tokio, reqwest, rustls, ring, etc.) do not entangle firmware build
- No on-device blocker; Phase 1 can proceed

**Phase 1 (In Progress, 2026-08-12):**
- eml.2 (merged): axum server skeleton + auth boundary (login form, bearer token, session management)
- eml.3 (in progress): auth implementation (email + password + 2FA via web UI, SDK login_password)
- eml.4 (queued): vault read (SDK sync, decrypt, metadata list)
- eml.5 (queued): device transport (HttpDeviceTransport for emulator; BleDeviceTransport / UsbDeviceTransport deferred to Phase 2)
- eml.6 (queued): web UI (vanilla JS or minimal framework; login form, vault list, sync button)
- eml.7 (queued): testing (integration tests via headless emulator; verify sync end-to-end)

**Phase 2 (Deferred, overlaps M2):**
- Device firmware sync handler (BLE/USB push protocol receiver, state machine, on-device storage)
- Swap HttpDeviceTransport to BleDeviceTransport / UsbDeviceTransport at server startup
- Web UI and server logic unchanged

**Companion-Push Validation Path:**
The web companion unblocks M1's real-vault requirement without on-device SDK (proven NO-GO in M0 spike). The emulator can sync and store real credentials immediately. Device display (M1) is already complete; web companion provides the sync. M2 (Type it) uses the same push protocol. This linear path validates the portable-vault concept before Phase 2 hardware complexity.

---

### SDK Spike Closure and Sync Direction Pivot (2026-08-11)

**Spike bead `ai-bitwarden-hw-key-8d7.2` (closed)** returned a definitive NO-GO for on-device Bitwarden Rust SDK sync on ESP32-S3:
- **ring's bundled C crypto links wrong-endian on xtensa** (C/LLVM level, no application-side fix; insurmountable).
- **bitwarden-crypto unconditionally pulls reqwest/rustls/ring/mockall stack** via bitwarden-api-key-connector; no feature-gated KDF-only seam.
- Compilation made to work with workarounds (`ring`'s `less-safe-getrandom-espidf`, `opt-level=0`), but link wall is fatal.

**Decision Pivot**: Device adopts **companion-app push model** for M0–M2 validation. A trusted companion app (desktop, mobile, or Web Vault) runs the full Bitwarden SDK, authenticates, syncs, and decrypts; it then pushes credentials to the device via `PushSyncSource`. Device is a secure display + HID peripheral (no SDK, TLS, HTTP, or crypto operations).

**Deferred**: On-device first-class SDK client via private fork of Bitwarden crates (epic `ai-bitwarden-hw-key-1sg`, conditional, only revived if portable-vault validates and business case holds).

**ADRs recorded**: 
- [2026-08-11-sync-direction-companion-push.md](./decisions/2026-08-11-sync-direction-companion-push.md) (post-spike decision; resolves the deferral from sync-source-abstraction ADR)
- Updated: [2026-08-11-sync-source-abstraction.md](./decisions/2026-08-11-sync-source-abstraction.md) (added post-spike update note)

**Impact on M0–M2**: Unblocked. M1 can proceed with real vault data via companion push. No embedded-crypto risk. Companion app scope: initial CLI or simple desktop GUI for testing; full Web Vault integration deferred post-M2.

---

### M0 (Platform Migration) Architectural Foundation Summary

**Design decisions** (recorded 2026-08-11, all five ADRs accepted):

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
