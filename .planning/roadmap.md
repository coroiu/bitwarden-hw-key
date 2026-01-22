# Project Roadmap

High-level vision and milestones for the Bitwarden Hardware Key proof-of-concept.

**Last Updated**: 2026-01-22

## Project Vision

Create a hardware-based Bitwarden key using an ESP32 microcontroller with an OLED display. The device will provide a secure, portable way to access Bitwarden credentials without relying on a smartphone or computer screen.

**Ultimate Goal**: Support FIDO2/passkeys with CTAP2 protocol for modern passwordless authentication.

**Phase 1 Goal**: Validate hardware UX viability with keyboard emulation before investing in FIDO2 implementation.

## Current Phase: Phase 1 - Keyboard Emulation PoC

**Goal**: Prove that browsing and using credentials on 128x32 display with 3-button navigation is practical and usable.

**Target Timeline**: 4 weeks

### Completed
- ✅ ESP32 development environment setup
- ✅ OLED display driver integration (SSD1306 128x32)
- ✅ Component-based GUI system (simple_gui)
- ✅ Vertical menu with scrolling
- ✅ Focus management system (gained/lost/activated events)
- ✅ Desktop emulator with minifb (128x32 → 1024x256 window)
- ✅ Keyboard input handling (Up/Down/Space)
- ✅ Auto-scrolling to keep focused items visible

### In Progress
- 🔄 HTTP server in desktop emulator for credential sync
- 🔄 Credential data model and CBOR encoding
- 🔄 NVS storage for ESP32
- 🔄 Web Vault integration (Angular component)

### Next Up (Phase 1 Remaining)
- Credential list view (reuse existing VerticalMenu)
- Credential detail view component
- Keyboard output (BLE HID on ESP32, simulation on desktop)
- BLE service for credential sync on ESP32
- End-to-end testing of credential sync → browse → type workflow

## Phase 1 Breakdown

### Phase 1.1: Foundation ✅ (Week 1)
- ✅ Desktop emulator working
- ✅ Focus system implemented
- 🔄 HTTP server in desktop emulator
- 🔄 Credential data model (Rust struct)
- 🔄 CBOR encoding/decoding

### Phase 1.2: Storage (Week 1-2)
- Desktop: JSON file storage at `~/.bitwarden-hw-key/credentials.json`
- ESP32: NVS storage with 16KB encrypted partition
- Credential persistence across restarts
- Handle storage full scenarios

### Phase 1.3: GUI Views (Week 2)
- Credential list view (reuse VerticalMenu with credential data)
- Credential detail view component
- Navigation between list and detail views
- Show/hide password toggle

### Phase 1.4: Web Vault Integration (Week 2-3)
- New Angular component: `sync-to-device.component.ts`
- CBOR encoding in TypeScript
- HTTP POST to desktop emulator
- Error handling and user feedback
- Integration into Web Vault settings/tools page

### Phase 1.5: Keyboard Output (Week 3)
- Desktop: Keyboard simulation library (enigo or autopilot-rs)
- Type username + Tab + password + Enter
- Handle special characters
- Timing between keystrokes

### Phase 1.6: ESP32 Hardware Port (Week 3-4)
- Replace HTTP with BLE characteristic writes
- Implement BLE HID keyboard with `esp32-nimble`
- Configure NVS encryption in sdkconfig
- BLE pairing flow with 6-digit passkey display
- Test on real hardware with iOS/Android/Windows/Mac

### Phase 1.7: Testing & Polish (Week 4)
- Test with 10, 50, 100, 500 credentials
- Measure scrolling performance
- Loading indicators and empty states
- Error messages
- Documentation

### Phase 1 Success Criteria
- ✅ User can sync 100+ credentials from Web Vault to device
- ✅ User can browse credentials with smooth scrolling
- ✅ User can select a credential and have it typed into a login form
- ✅ Credentials persist across device restarts
- ✅ Interaction feels natural and usable

## Phase 2: FIDO2/Passkey Support (Future)

**Goal**: Add CTAP2 authenticator capabilities for passwordless authentication

**Prerequisites**: Phase 1 complete and validated that hardware UX works

### Planned Features
- CTAP2 protocol implementation using `passkey-rs`
- BLE FIDO2 service (separate from HID)
- Credential creation (makeCredential)
- Credential assertion (getAssertion)
- User verification (PIN entry UI)
- Discoverable credentials support
- Resident key management

### Technical Requirements
- Integrate `passkey-rs` crate
- Implement CTAP2 over BLE transport
- Design PIN entry on 3-button interface
- Store FIDO2 credentials in NVS
- Handle credential selection for multiple accounts

### Design Challenges
- PIN entry UX on 128x32 display with 3 buttons
- Distinguishing FIDO2 from password credentials in UI
- Managing both keyboard emulation and FIDO2 modes
- User verification requirements

## Phase 3: Security Hardening (Future)

**Goal**: Production-grade security implementation

### Planned Features
- Credential encryption at rest (beyond NVS encryption)
- Key derivation from user master PIN
- Device authentication with Web Vault
- Auto-lock timeout
- Secure boot configuration
- Side-channel attack mitigations
- Secure memory clearing
- Tamper detection

### Technical Requirements
- Implement KDF for encryption keys
- Store master key securely
- Flash encryption
- Secure element integration (if hardware supports)
- Security audit

## Phase 4: Advanced Features (Future)

**Goal**: Enhanced usability and functionality

### Potential Features
- TOTP/2FA code generation
- Search/filter for large vaults
- Favorite credentials (quick access)
- Multiple vault support
- OTA firmware updates
- Battery optimization
- Better visual design with icons
- WiFi sync (alternative to BLE)

### Challenges
- Display size constraints for search UI
- Memory usage with large vaults
- Power consumption
- Secure OTA implementation

## Architecture Decisions

Key architectural decisions are documented in `.planning/decisions/`:

- **[2026-01-22: Keyboard Emulation First](decisions/2026-01-22-keyboard-emulation-first.md)** - Why we're starting with BLE HID keyboard instead of FIDO2
- **[2026-01-22: Emulator HTTP Protocol](decisions/2026-01-22-emulator-http-protocol.md)** - Desktop emulator runs HTTP server for Web Vault to connect to
- **[2026-01-21: Desktop Emulation](decisions/2026-01-21-desktop-emulation.md)** - Separate binary with target-specific dependencies
- **[2026-01-21: Focus Management System](decisions/2026-01-21-focus-management-system.md)** - High-level focus events instead of raw input

## Open Questions

### Phase 1
1. **Desktop keyboard simulation**: Which Rust library?
   - `enigo`? `autopilot-rs`? `rdev`?
   - Need cross-platform support (macOS, Linux, Windows)

2. **Large vaults**: How to handle 500+ credentials?
   - Pagination in UI? Lazy loading? Search?
   - Performance testing needed

3. **Duplicate credentials**: Multiple logins for same site?
   - Show submenu? Number them?

4. **Special characters**: How to type non-ASCII passwords over BLE HID?
   - Unicode keyboard support? Fallback to clipboard?

### Phase 2 (FIDO2)
1. **PIN entry UX**: How to enter 4-8 digit PIN with 3 buttons?
   - Scroll through digits 0-9?
   - T9-style input?
   - Companion app for setup?

2. **User verification**: PIN only, or biometric via phone?
   - ESP32 has no biometric sensors
   - Could leverage paired phone's biometrics?

3. **Credential selection**: FIDO2 allows multiple credentials per RP
   - How to disambiguate on small display?

## Success Criteria

### Phase 1 Success (Keyboard Emulation)
- ✅ Can sync 100+ credentials from Web Vault in < 5 seconds
- ✅ Can browse credentials with < 50ms scroll latency
- ✅ Can select credential and type it into login form successfully
- ✅ Credentials persist across restarts
- ✅ BLE pairing works on iOS/Android/Windows/Mac
- ✅ Typing speed feels natural (not too fast or slow)
- ✅ Hardware form factor is validated as usable

### Phase 2 Success (FIDO2)
- Can register passkey on test website
- Can authenticate with passkey
- User verification flow works
- Multiple passkeys managed correctly
- CTAP2 protocol compliance

### Phase 3 Success (Security)
- Credentials encrypted at rest with user PIN
- Auto-lock after timeout
- Secure boot enabled
- Security audit passed
- No credentials leak in memory dumps

### Full PoC Success
- End-to-end passwordless authentication works
- Security model is sound
- UX is pleasant and efficient
- Performance is acceptable
- Code is maintainable and documented

## Timeline

### Phase 1: 4 weeks (estimated)
- Week 1: Foundation + Storage
- Week 2: GUI Views + Web Vault
- Week 3: Keyboard Output + ESP32 Port
- Week 4: Testing + Polish

### Phase 2: TBD
- Depends on Phase 1 validation
- Estimated 4-6 weeks

### Phase 3+: Future
- Timeline TBD based on priorities

**Note**: This is a proof-of-concept project. Timelines are estimates for planning purposes. The focus is on learning and validation rather than strict delivery dates.

## Key Milestones

- 🎯 **Phase 1.1 Complete**: Desktop emulator can receive credentials via HTTP
- 🎯 **Phase 1.3 Complete**: Can browse credentials on device
- 🎯 **Phase 1.5 Complete**: Can type credentials into browser
- 🎯 **Phase 1.6 Complete**: Works on real ESP32 hardware
- 🎯 **Phase 1 Complete**: Hardware UX validated as viable
- 🎯 **Phase 2 Complete**: FIDO2 authentication working
- 🎯 **Phase 3 Complete**: Production-ready security

## Related Documentation

- **Technical Design**: [technical-design-phase1.md](technical-design-phase1.md) - Detailed Phase 1 architecture
- **Research Findings**: [.research/findings/](../.research/findings/) - Research on NVS, BLE HID, etc.
- **Decision Logs**: [decisions/](decisions/) - ADRs for major decisions
- **Progress Tracking**: [progress.md](progress.md) - Current status and next steps

## Notes

- This roadmap will evolve as we learn more about the technical constraints and possibilities
- Document major decisions in `.planning/decisions/` as they're made
- Update this roadmap when priorities or direction change
- Phase 1 is focused on **validation**, not perfection - we're testing the core hypothesis
- FIDO2 (Phase 2) only happens if Phase 1 proves the hardware UX is viable
