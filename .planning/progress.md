# Project Progress

**Last Updated**: 2026-08-17 (M0 on-hardware validation complete; M1.5 Phase 1 complete; M1.5 Phase 2 unblocked)

## Current Status (2026-08-17)

**M0 (Platform Migration) COMPLETE: ON-HARDWARE VALIDATED (2026-08-17)**
Epic `ai-bitwarden-hw-key-8d7` closed. All nine workstreams (W1-W9) merged to main. Full on-hardware validation completed on physical Lilygo T-Embed CC1101 board (merge commit ef3daf6). Fixes validated: ST7789 display bring-up (Rotation::Deg270, native unrotated 170x320 panel size, software reset, color inversion, display_offset(35,0)), CC1101 variant pinmap corrected (SPI SCLK/MOSI/CS GPIO11/9/41, DC GPIO16, backlight GPIO21, rotary encoder GPIO4/5, button GPIO0), quadrature decoding via GPIO edge interrupts, detail-view stack overflow fixed (sdkconfig.defaults relocated to workspace root, stack size raised to 32KB), demo-seed feature added (off-by-default). All display rendering, color accuracy, encoder response, and full navigation/detail-view stacks hardware-verified.

**M1 (Vault Browse) DEVICE DISPLAY: COMPLETE**
Credential list and detail views designed and implemented on color + rotary encoder. Full render/nav/input stacks merged to main and verified headless. Real vault sync delivery complete via M1.5 (Web Companion).

**M1.5 (Web Companion) PHASE 1 COMPLETE: END-TO-END REAL VAULT SYNC VERIFIED**
Architecture: local Rust server (axum + tokio) linking the Bitwarden SDK, thin vanilla-JS web UI served over 127.0.0.1, server-owned DeviceTransport (Phase 1: HttpEmulatorTransport via CBOR /api/sync; Phase 2: native BLE/USB to T-Embed). Real flow verified end-to-end: live Bitwarden login with 2FA, vault sync of 24 items, push to emulated device, device renders credential list and detail view with masked passwords. Beads: epic ai-bitwarden-hw-key-eml with children eml.1 through eml.11 delivered. eml.1 (SDK feasibility on host, proven); eml.2 (axum server skeleton + bearer-token boundary); eml.3 (auth: login form, SDK login_password, 2FA support); eml.4 (vault read: SDK sync, metadata-only list); eml.5 (device transport: HttpEmulatorTransport); eml.6 (web UI: vanilla JS, login, vault list, sync button); eml.7 (integration tests); eml.8 (docs); eml.10 (startup banner); eml.11 (login fix). eml.9 and eml.12 closed as superseded/duplicate. Epic eml remains OPEN for Phase 2.

**Key Fix in eml.11:** Real login returned 401 until ClientSettings.bitwarden_client_version was set to Some(CARGO_PKG_VERSION). The live Bitwarden API rejects requests lacking the client-version header. This class of bug surfaces only against the live API, not compile-only checks or dev mocks. Fix committed; real vault sync now fully functional.

**Security Posture in Place (PoC):** Bind 127.0.0.1 only; per-process bearer token; passwords never sent to browser; metadata-only vault list; decrypted vault server-side only, pushed to device during sync, zeroized on lock/logout. A gitignored debug test account exists at data/dev-account.env for end-to-end testing (do not commit; never print the password in docs).

**Next: Phase 2 (native BLE/USB transport to physical T-Embed, firmware sync handler) overlaps M2 (Type it). Hardware validation now complete, so Phase 2 is unblocked and can proceed.**

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

### M1.5 Phase 2 Design Complete (2026-08-17)

**ADRs and Decisions Accepted:**

Two architectural decisions formalize Phase 2 scope and close design uncertainty:

1. **USB-Serial-JTAG transport (ADR 2026-08-17-phase2-usb-transport-jtag-first)**
   - **Status**: Accepted
   - **Basis**: Feasibility spike (bead `ai-bitwarden-hw-key-8bi`). TinyUSB blocked by `esp-idf-sys#377` (unresolved since June 2025); adoption requires irreversible USB_PHY_SEL eFuse burn, losing easy flash/monitor on that unit. USB-Serial-JTAG is immediately viable, keeps dev loop intact.
   - **Outcome**: Phase 2 syncs over USB-Serial-JTAG (ROM CDC, no external controller). TinyUSB deferred to M2 with open re-evaluation.
   - **M2 note**: USB HID typing also needs TinyUSB + eFuse burn. Strong case to prefer BLE HID for M2 or accept dedicated typing unit. No pre-decision.

2. **Device-Link Serial Framing Protocol (ADR 2026-08-17-device-link-serial-framing-protocol)**
   - **Status**: Accepted (implemented in Phase 2 WS1, merged to main, bead ai-bitwarden-hw-key-2ox.1)
   - **Content**: Binary frame header (magic 0xB1 0x7C, type u8, flags, len u32 LE, payload, crc32 LE), 11-variant message multiplex (host->device: SyncBegin, SyncChunk, SyncEnd, InputInject, FramebufferRequest, Ping; device->host: SyncAck, SyncNack, FramebufferData, Log, Pong), CBOR-encoded structured payloads reusing `push_protocol::{SyncRequest, SyncResponse, Credential}` plus a `WireIntent` enum.
   - **Outcome**: Portable protocol over any byte stream (USB-Serial-JTAG now, swappable later). Multiplexes sync, verify-seam (agent-driven NavIntent injection + framebuffer capture), and device logs. Closed bead `ai-bitwarden-hw-key-dvm` (real-target verification).

**Phase 2 Epic (ai-bitwarden-hw-key-2ox):**
- **WS1** (device-link crate, merged to main): Design and implement binary framing, message types, CBOR encoding/decoding, streaming reassembly.
- **WS2-WS6** (scoped, hardware-gated): Firmware USB-Serial-JTAG driver, device-side message receiver, sync state machine, on-device storage integration, web-companion host-side USB client, verify-seam integration.

**Follow-up beads (open):**
- `ai-bitwarden-hw-key-8kx`: Credential detail-view scroll (deferred from M0 closure).
- `ai-bitwarden-hw-key-2ed`, `2ox`, `nrv`, `dje`, `ekd`: M1.5 Phase 2 workstreams (transport, firmware handler, web-companion client).

**M2 Output Direction Decided (2026-08-17):**
M2 output direction decided = BLE HID (avoids the irreversible USB eFuse burn; makes Phase 2 USB-serial sync durable). Rationale in ADR and roadmap.md M2 section.

---

### M0 (Platform Migration): COMPLETE (2026-08-17)

**Bead `ai-bitwarden-hw-key-8d7` (epic, CLOSED)** — all 9 workstreams W1-W9 completed and merged to main as of 2026-08-17:

**Merged Workstreams:**
- **W1**: ADRs and interface freeze recorded (five M0 decisions in `.planning/decisions/`).
- **W2**: Cargo workspace (core/firmware/emulator) with compiler-enforced platform-free bhk_core.
- **W3**: Render core on embedded-graphics (FrameBuffer565 Rgb565, Widget/Navigator, chrome regions). Two render bugs found and fixed with regression tests (W3 sub-row text overflow; c2f windowed blank-window/minifb-pump).
- **W4**: Host surfaces (MinifbSurface, HeadlessSurface, HostClock, FileStorage, WindowedInput).
- **W5**: Headless HTTP NavIntent injection + screenshot capture (verified A-B-C credential-list movement via headless agent-driven screenshot inspection).
- **W6**: T-Embed ESP32-S3 board adapter (ST7789 via mipidsi, rotary encoder, NVS).
- **W7**: Unified Platform-generic run() loop + App + drivable credential-list shell. Old simple_gui/gui engines deleted.
- **W8**: SDK feasibility spike = NO-GO on-device (ring won't link on xtensa; bitwarden-crypto not modular). Outcome: pivot to companion-app push model (decision recorded in ADR).
- **W9**: SyncSource + PushSyncSource trait implementations.

**On-Hardware Validation (COMPLETE 2026-08-17):**
- Physical T-Embed CC1101 board brought up on real hardware for the first time (merge commit ef3daf6, branch bd-ai-bitwarden-hw-key-c6e, independent code review, no blockers).
- **Display**: ST7789 bring-up completed. Fixed: mipidsi display_size must be native unrotated panel size (170x320, not logical 320x170). Passing 320x170 exceeded framebuffer (boot-loop InvalidDisplaySize). Final config: Rotation::Deg270, display_offset(35,0), ColorInversion::Inverted, NoResetPin (board has no LCD reset, uses software reset). Right-side-up, colors correct, offset correct, all hardware-verified.
- **CC1101 Pinmap**: Corrected SPI (SCLK GPIO11, MOSI GPIO9, CS GPIO41), DC GPIO16, backlight GPIO21 (plain T-Embed code drove GPIO15, which is panel POWER-ENABLE on CC1101; GPIO21 is the real backlight, was never driven = dark panel). Encoder A/B corrected to GPIO4/GPIO5 (button GPIO0).
- **Rotary Encoder**: Replaced frame-rate-polled decode (rotary-encoder-hal, 30Hz, aliased/missed EC11 transitions, erratic) with GPIO edge-interrupt quadrature decoding + software quadrature table (copied from LilyGo factory firmware). Shared ISR/poll state via Arc of atomics (reviewed sound). Fast-scroll acceleration (NextN) removed per Andreas's request (one item per detent). Direction confirmed CW=Next/down.
- **Detail-View Crash**: Opening credential detail view overflowed ESP-IDF main task stack (device-only; emulator host has huge stack, never showed). Root: legitimately deeper detail-view render call graph plus latent bug: firmware/sdkconfig.defaults had been SILENTLY DEAD since workspace split (esp-idf-sys resolves sdkconfig.defaults relative to CARGO WORKSPACE ROOT, not firmware crate dir). Fixed: moved sdkconfig.defaults to workspace root, raised CONFIG_ESP_MAIN_TASK_STACK_SIZE to 32768.
- **Demo-Seed Feature**: Added off-by-default `demo-seed` cargo feature seeding 5 placeholder VaultItems through the SyncSource path (run loop calls app.step(sync) before first render; one-shot vec gets wiped). Default build remains honest empty vault (NoSyncSource).

**Open Follow-Up Beads:**
- `ai-bitwarden-hw-key-ekd`: Build-time plain-vs-CC1101 board selection + wire CC1101 extra user button GPIO6.
- `ai-bitwarden-hw-key-dvm`: Agent-driven real-target verification via serial NavIntent injection + framebuffer capture (now folded into Phase 2).
- `ai-bitwarden-hw-key-ego`: Display refresh rate (full-frame SPI blit at 20MHz is bottleneck).
- `ai-bitwarden-hw-key-2ed`: Encoder acceleration removed/mooted; revisit with real PrevN if reintroduced.
- Backlog: `ai-bitwarden-hw-key-nrv` (runtime screen-rotation setting), `ai-bitwarden-hw-key-dje` (idle screensaver / display-off).

---

### M1.5 (Web Companion) Epic Kickoff (2026-08-12)

**Bead ai-bitwarden-hw-key-eml (epic, OPEN)** — Web companion development. Phases:

**Phase 0 (Complete, 2026-08-12):**
- SDK feasibility spike: proven Bitwarden Rust SDK (bitwarden/sdk-internal@99ffb6ef) links on macOS host in isolated nested `web-companion/` workspace
- Isolation verified: SDK dependencies (tokio, reqwest, rustls, ring, etc.) do not entangle firmware build
- No on-device blocker; Phase 1 can proceed

**Phase 1 (Complete, 2026-08-17):**
- eml.2 (merged): axum server skeleton + auth boundary (login form, bearer token, session management)
- eml.3 (merged): auth implementation (email + password + 2FA via web UI, SDK login_password)
- eml.4 (merged): vault read (SDK sync, decrypt, metadata list)
- eml.5 (merged): device transport (HttpDeviceTransport for emulator)
- eml.6 (merged): web UI (vanilla JS; login form, vault list, sync button)
- eml.7 (merged): testing (integration tests via headless emulator; verify sync end-to-end)
- eml.8 (merged): docs
- eml.10 (merged): startup banner
- eml.11 (merged): login fix (client-version header)

**Phase 2 (Queued, overlaps M2, now unblocked by hardware arrival):**
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

**Decision Pivot**: Device adopts **companion-app push model** for M0-M2 validation. A trusted companion app (desktop, mobile, or Web Vault) runs the full Bitwarden SDK, authenticates, syncs, and decrypts; it then pushes credentials to the device via `PushSyncSource`. Device is a secure display + HID peripheral (no SDK, TLS, HTTP, or crypto operations).

**Deferred**: On-device first-class SDK client via private fork of Bitwarden crates (epic `ai-bitwarden-hw-key-1sg`, conditional, only revived if portable-vault validates and business case holds).

**ADRs recorded**: 
- [2026-08-11-sync-direction-companion-push.md](./decisions/2026-08-11-sync-direction-companion-push.md) (post-spike decision; resolves the deferral from sync-source-abstraction ADR)
- Updated: [2026-08-11-sync-source-abstraction.md](./decisions/2026-08-11-sync-source-abstraction.md) (added post-spike update note)

**Impact on M0-M2**: Unblocked. M1 can proceed with real vault data via companion push. No embedded-crypto risk. Companion app scope: initial CLI or simple desktop GUI for testing; full Web Vault integration deferred post-M2.

---

### M0 (Platform Migration) Architectural Foundation Summary

**Design decisions** (recorded 2026-08-11, all five ADRs accepted):

1. **Presentation Surface and Run-Mode Seam** - Platform abstraction with four injected traits (DisplaySurface, InputSource, Clock, Storage). Canonical pixel format: Rgb565 throughout the core. Headless, windowed, and real-target differ only in surface implementation.
2. **Portability Boundary and Workspace Split** - Three-layer Cargo workspace (`core` / `firmware` / `emulator`) enforcing platform separation at compile time. No platform-specific dependencies in the core; trait implementations live in platform-specific crates.
3. **Rotary Encoder Input Model and Navigation Intent** - Two-tier input abstraction: raw platform events - semantic `NavIntent` enum. Encoder mapping: rotation - Next/Prev, fast rotation - NextN (acceleration), short press - Activate, long press - Back. Headless agents inject `NavIntent` directly; app is hardware-agnostic.
4. **Sync Source Abstraction and Deferred SDK Decision** - `SyncSource` trait allows `PushSyncSource` (fallback, works now) and `SdkSyncSource` (spike research) to coexist. M0 uses push; spike (W8) runs in parallel; final choice (SDK viable or fallback) recorded in post-spike ADR.
5. **UI Framework: Retire Both Existing GUIs, Rewrite Clean** - Both `src/gui/` (dead code) and `src/simple_gui/` (architecturally incompatible with color) are deprecated. Rewrite on `embedded-graphics` + `embedded-graphics-framebuf` + `u8g2-fonts` + `mipidsi`. Salvage high-level concepts only (navigation stack, ComponentAction, FocusEvent); re-implement cleanly. Layout: fixed chrome + linear stacks, no flexbox.

Beads epic `ai-bitwarden-hw-key-8d7` created with workstreams:
- **W1 (complete)**: Record M0 ADRs and planning updates (this bead).
- **W2**: Implement platform traits and workspace split (Ruby).
- **W3**: Build new GUI framework on embedded-graphics (Fern, Ruby).
- **W4**: Implement three run modes (headless, windowed, real-target) (Tess, Ruby).
- **W5**: Port to T-Embed hardware and verify (Ruby, Tess).
- **W6-W7**: UI design (rotary encoder UX, color layout) (Uma).
- **W8 (parallel)**: Bitwarden Rust SDK feasibility spike (Ruby).
- **W9**: M0 integration and closure.

Next: W2-W7 (foundation implementation) proceed in parallel; W8 (spike) runs independently.

## Next Steps

### M1.5 Phase 2 (Device Transport: USB Serial, Real T-Embed)

Device firmware gains a sync handler for native USB or BLE push from the web companion. Web UI and server logic unchanged. Agent-driven real-target verification via serial NavIntent injection + framebuffer capture integrated into this phase.

## Blockers
None currently identified

## Notes
- **2026-08-17 M0 Closure**: On-hardware validation complete. Physical T-Embed CC1101 board brought up successfully (merge ef3daf6). All display, encoder, stack management, and feature issues fixed and merged. Epic ai-bitwarden-hw-key-8d7 closed. Phase 2 (device transport, overlaps M2) is now unblocked.
- **2026-08-11 Reconciliation**: Vision re-grounded. Old prototype (128x32 SSD1306, 3-button nav, simple_gui) is complete as historical work but treated as throwaway pending M0 green-field redesign. Roadmap updated to milestone-based structure (M0 through M4) centered on T-Embed first target and color + rotary encoder UX. Work will proceed via beads epic workflow.
- WIP.md has been migrated to this progress tracking system (2026-01-21)
- Project structure modernized with .planning and .research directories (2026-01-21)
