# Project Roadmap

High-level vision and milestones for the Bitwarden Hardware Key proof-of-concept.

## Project Vision

Create a hardware-based Bitwarden key using an ESP32 microcontroller with an OLED display. The device will provide a secure, portable way to access Bitwarden credentials without relying on a smartphone or computer screen.

## Current Phase: Foundation (MVP)

**Goal**: Build the core UI framework and basic interaction model

### Completed
- ✅ ESP32 development environment setup
- ✅ OLED display driver integration (SSD1306 128x32)
- ✅ Basic GUI component system
- ✅ Vertical menu rendering
- ✅ Label component
- ✅ Component lifecycle scaffolding

### In Progress
- 🔄 Custom backing components for UI elements
- 🔄 Focus handling and navigation system

### Next Up
- Input handling architecture (buttons/keyboard)
- Menu navigation with hardware buttons
- Basic text rendering and wrapping
- List/scroll view components

## Phase 2: Core Functionality (TBD)

**Goal**: Implement basic Bitwarden integration

### Planned Features
- Bitwarden vault data structures
- Credential storage (encrypted)
- Search/filter interface for credentials
- Password display with timeout
- TOTP code generation (if applicable)

### Technical Requirements
- Secure storage implementation
- Memory-efficient data structures
- Low-power operation considerations

## Phase 3: Security & Polish (TBD)

**Goal**: Harden security and improve UX

### Planned Features
- Secure element integration (if hardware supports)
- PIN/password protection
- Session timeout/lock screen
- Better visual design and icons
- Battery optimization
- Error handling and recovery

### Security Considerations
- Key derivation and storage
- Secure boot configuration
- Side-channel attack mitigations
- Memory encryption

## Phase 4: Connectivity & Sync (Future)

**Goal**: Enable vault synchronization

### Potential Features
- WiFi/BLE connectivity
- Vault synchronization with Bitwarden server
- Device pairing/authentication
- OTA firmware updates

### Challenges
- Network security
- Power consumption during sync
- Error handling for connectivity issues

## Open Questions

1. **Input Method**: What's the best way to handle text input on limited hardware?
   - Hardware buttons only?
   - Bluetooth keyboard support?
   - Companion app?

2. **Vault Storage**: How to handle large vaults with limited memory?
   - Full vault in memory vs. on-demand loading?
   - Search/indexing strategy?

3. **Connectivity**: Should this device be online or offline-only?
   - Offline-only is more secure but less convenient
   - Online enables sync but increases attack surface

4. **Power Management**: What's the expected battery life?
   - Deep sleep between uses?
   - Display timeout?

## Success Criteria

### MVP Success
- Can navigate a UI menu with hardware buttons
- Can display static credential information
- Runs reliably on ESP32 hardware

### Full POC Success
- Can access a real Bitwarden vault
- Credentials are stored securely
- UX is usable for basic operations
- Code is well-documented and maintainable

## Timeline

This is a proof-of-concept project, so timelines are flexible. The focus is on learning and exploration rather than delivery dates.

## Notes

- This roadmap will evolve as we learn more about the technical constraints and possibilities
- Document major decisions in `.planning/decisions/` as they're made
- Update this roadmap when priorities or direction change
