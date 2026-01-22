# Keyboard Emulation Before FIDO2 for PoC

**Date**: 2026-01-22
**Status**: Accepted

## Context

The ultimate goal is to support FIDO2/passkey credentials on the hardware key. However, implementing the full CTAP2 protocol, credential storage, and testing infrastructure is complex. We need to decide what to implement first to validate the core value proposition: "Is browsing and using credentials on a 128x32 display with 3 buttons practical?"

Two approaches were considered:
1. **Start with FIDO2/CTAP2** - Implement full passkey support immediately
2. **Start with Keyboard Emulation** - Implement BLE HID keyboard to type username/password

## Decision

We will implement **keyboard emulation (BLE HID) first**, then add FIDO2 support in Phase 2.

## Rationale

### Why Keyboard Emulation First

1. **Faster to implement**: BLE HID is well-supported on ESP32 with `esp32-nimble`, can be working in days
2. **Tests core UX question**: Validates whether the hardware form factor (display size, button navigation) is usable
3. **Universal compatibility**: Works with ANY website/app, not just FIDO2-compatible sites
4. **Simpler emulator**: Desktop emulator can simulate keystrokes easily, no CTAP2 protocol needed
5. **Lower risk**: If hardware UX doesn't work, we haven't wasted time on CTAP2 implementation
6. **Foundation for FIDO2**: GUI components, credential storage, and BLE infrastructure will be reused

### Why Not FIDO2 First

1. **CTAP2 complexity**: While `passkey-rs` helps, full implementation still requires significant work
2. **Testing complexity**: Need mock CTAP2 server, browser integration, authenticator registration flow
3. **Limited validation**: Can't test until entire CTAP2 stack works end-to-end
4. **Delayed learnings**: Won't discover UX issues until late in development

### Addressing FIDO2 Goal

- FIDO2 is still the end goal and will be implemented in Phase 2
- `passkey-rs` will significantly simplify CTAP2 implementation when we get there
- BLE infrastructure and credential management built for keyboard emulation will directly support FIDO2
- Can maintain both modes: keyboard emulation for legacy sites, FIDO2 for modern sites

## Alternatives Considered

### Alternative 1: FIDO2 Only
- **Pros**: Tests the actual end goal, more secure, future-proof
- **Cons**: Much longer to first working prototype, higher risk, complex emulation

### Alternative 2: WiFi + Direct Server Integration
- **Pros**: Tests standalone device mode, no companion app needed
- **Cons**: Requires crypto implementation, auth UX nightmare on tiny display, weeks/months to testable

## Consequences

### Positive
- Can validate hardware UX in days/weeks instead of months
- Lower implementation risk
- Simpler testing and debugging
- Easier to demonstrate PoC value
- Foundation infrastructure supports future FIDO2 work

### Negative
- Keyboard emulation is less secure (credentials visible as keystrokes)
- Need to implement FIDO2 separately later
- Two different credential use modes to maintain
- May need to refactor some code when adding FIDO2

### Mitigations
- Keep architecture extensible for multiple credential types
- Design credential storage to support both username/password and passkey data
- Plan BLE service structure to accommodate both HID and FIDO2 profiles
- Document transition plan to FIDO2 in technical design

## Implementation Phases

### Phase 1: Keyboard Emulation PoC (This Decision)
1. BLE HID keyboard implementation
2. Credential sync from Web Vault via HTTP
3. NVS storage for credentials
4. GUI for browsing and selecting credentials
5. Desktop emulator with HTTP server

### Phase 2: FIDO2 Support (Future)
1. Integrate `passkey-rs` for CTAP2 protocol
2. Implement BLE FIDO2 service (separate from HID)
3. Handle credential creation and assertion flows
4. Add user verification (PIN? biometric?)
5. Test with real authenticator-enabled websites

## References

- Research findings: [2026-01-22-esp32-nvs-and-ble-hid.md](../../.research/findings/2026-01-22-esp32-nvs-and-ble-hid.md)
- `esp32-nimble`: https://github.com/taks/esp32-nimble
- `passkey-rs`: https://github.com/1password/passkey-rs
- BLE HID specification: https://www.bluetooth.org/docman/handlers/downloaddoc.ashx?doc_id=245141
