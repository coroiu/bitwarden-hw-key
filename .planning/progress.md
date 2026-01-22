# Project Progress

**Last Updated**: 2026-01-22 (Phase 1.2 complete)

## Current Status
Phase 1.2 complete! Credentials now persist to ./data/credentials.json and are automatically loaded on startup. The emulator is ready for Phase 1.3 (GUI implementation).

## Completed
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
None - ready to start Phase 1.3 (GUI)

## Next Steps

### Immediate (Phase 1.3 - GUI)
1. Refactor simple_view.rs to load credentials from shared state
2. Create credential list view using VerticalMenu
3. Display credential names in the list
4. Test scrolling with 10+ credentials

### Short-term (Phase 1.3 - GUI Detail View)
1. Create credential list view (reuse VerticalMenu with credential data)
2. Create credential detail view component
3. Add navigation between list and detail views
4. Show/hide password toggle

### Medium-term (Phase 1.4-1.5 - Weeks 2-3)
1. Build Web Vault Angular component (sync-to-device.component.ts)
2. Add CBOR encoding to Web Vault
3. Test end-to-end desktop sync flow
4. Implement keyboard output on desktop (enigo or autopilot-rs)
5. Test typing credentials into browser

### Long-term (Phase 1.6-1.7 - Weeks 3-4)
1. Port HTTP to BLE characteristic writes on ESP32
2. Implement BLE HID keyboard with esp32-nimble
3. Configure NVS encryption and BLE bonding
4. Test on real ESP32 hardware with iOS/Android/Windows/Mac
5. Polish, performance optimization, and documentation

## Blockers
None currently identified

## Notes
- WIP.md has been migrated to this progress tracking system (2026-01-21)
- Project structure modernized with .planning and .research directories (2026-01-21)
