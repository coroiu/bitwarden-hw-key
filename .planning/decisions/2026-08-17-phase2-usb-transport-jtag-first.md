# Phase 2 Device USB Transport: USB-Serial-JTAG First, Defer TinyUSB to M2

**Date**: 2026-08-17
**Status**: Accepted

## Context

M1.5 Phase 2 requires a device USB transport to sync the vault to the real T-Embed CC1101. The board's single USB-C port can be driven by exactly one of two options: USB-Serial-JTAG (fixed-function ROM CDC, what espflash/monitor use), OR TinyUSB via USB-OTG (can present a composite CDC+HID device). M2 (keyboard output over USB HID) needs HID, which requires TinyUSB. This decision determines the Phase 2 approach.

### Feasibility Spike

A dedicated spike (bead `ai-bitwarden-hw-key-8bi`) was run to evaluate both paths and settle the question before Phase 2 implementation begins.

### Spike Findings

**1. TinyUSB from Rust is blocked (Confirmed)**

A real composite CDC+HID descriptor was implemented and tested at the C level. The descriptor compiles and links in C, but esp-idf-sys's bindgen code-generation pass aborts the entire crate build with a `packed` / `repr(align)` incompatibility error on a TinyUSB descriptor type. This is a known upstream issue: `esp-rs/esp-idf-sys#377` (open since June 2025, no fix available). The issue was independently reproduced during the spike.

**2. ESP32-S3 USB_PHY_SEL eFuse is one-time-programmable (Confirmed at register/docs level, inference for this board)**

The ESP32-S3 USB PHY can be reassigned from USB-Serial-JTAG (ROM CDC) to USB-OTG (TinyUSB) via a one-time-programmable eFuse write (USB_PHY_SEL). Once burned, it is irreversible: the internal USB PHY is permanently locked to OTG mode, and the normal flash/monitor development path (over USB-Serial-JTAG) is lost. For the physical T-Embed-CC1101 board, the single USB-C port wires directly to GPIO19/20 with no external PHY (confirmed in the board's committed schematic pinmap; physical verification against the printed board PCB and official PDF schematic is still pending).

**3. Consequence: TinyUSB adoption requires irreversible hardware commitment (Confirmed)**

To use TinyUSB on the T-Embed-CC1101, the USB_PHY_SEL eFuse must be burned, permanently losing easy flashing and monitoring on that physical unit. No fallback exists short of replacing the board.

## Decision

**Use USB-Serial-JTAG as the Phase 2 sync data channel. Defer TinyUSB to M2 and re-evaluate then.**

The sync system is built transport-agnostic (the device-link protocol crate, the receiver state machine, the host UsbSerialTransport, the verify-seam), so only the device byte-driver (WS2) would change if TinyUSB is adopted later.

### Phase 2 Implementation

1. Device firmware gains a sync handler over USB-Serial-JTAG (the ROM CDC, no external controller).
2. The web companion connects to the device over the USB-Serial transport (via a UART-over-USB adapter or native USB-Serial-JTAG driver on the host).
3. The device takes exclusive ownership of the USB-Serial-JTAG peripheral and re-expresses its logs as framed `Log` messages in the binary frame stream. Raw text console output would corrupt the binary frame stream on the shared link.
4. The sync protocol (device-link crate, framing, message multiplex, state machine) is transport-independent; changing the byte-driver to TinyUSB later does not require protocol rework.

## Rationale

- **JTAG-first is the only currently-viable Rust path**: TinyUSB-from-Rust is blocked by `esp-idf-sys#377` (no workaround available; fix requires upstream resolution).
- **Avoids irreversible hardware commitment**: Using USB-Serial-JTAG keeps the eFuse unmolested and preserves the flash/monitor dev loop on the physical board. Future hardware units can opt for TinyUSB if desired; existing boards retain full dev capability.
- **Pragmatic validation path**: Phase 2 can proceed immediately without waiting for TinyUSB to be unblocked. Real vault sync to the device is validated over USB-Serial-JTAG while the fix or workaround is researched.
- **Transport abstraction holds up**: The sync protocol (device-link, framing, multiplex) is already designed to be transport-agnostic, so the decision to change byte-drivers at M2 is low-risk and non-invasive.
- **M2 decision point is explicit**: At M2, when USB HID typing is planned, the team can revisit: TinyUSB (if unblocked by then), BLE HID (wireless, no eFuse cost, but pairing/bonding complexity), or a dedicated typing unit. The choice is not pre-determined now.

## Alternatives Considered

1. **Adopt TinyUSB now (composite CDC+HID) to preempt M2 rework.**
   - **Pros**: CDC and HID on the same device; M2 composition is pre-planned; single USB cable for both.
   - **Cons**: Blocked by `esp-idf-sys#377` (no fix); requires burning USB_PHY_SEL eFuse (irreversible, loses easy flashing); delays Phase 2 pending TinyUSB unblock or workaround. Risk: if TinyUSB is never unblocked at Rust level, hardware units are locked into that choice with no easy fallback.
   - **Verdict**: Rejected. Blocking is unresolved; eFuse cost is too high for speculative M2 prep.

2. **Sync over BLE/Wi-Fi instead of USB (alternative transport).**
   - **Pros**: Wireless, no cable, no USB_PHY_SEL needed.
   - **Cons**: Out of scope for the accepted "USB serial first" direction; introduces wireless pairing/bonding complexity and power consumption (BLE) or connectivity (Wi-Fi). Suitable as a fallback or alternate if USB encounters show-stoppers, but not the primary Phase 2 path.
   - **Verdict**: Out of scope. Wireless transports remain valid alternatives for future exploration if USB encounters issues.

## Consequences

### Positive
- **Phase 2 unblocked**: No waiting for TinyUSB fix or workaround. Device USB sync is ready immediately.
- **Dev loop preserved**: Flashing, monitoring, and debugging remain easy on all existing boards.
- **Low-risk transport swap**: The protocol crate, state machine, and host transport are already transport-independent. Changing byte-drivers at M2 (if TinyUSB is unblocked or BLE is chosen) is a self-contained change.
- **M2 flexibility**: At M2, the team has three options (TinyUSB if fixed, BLE HID, dedicated typing unit) instead of being locked into one pre-chosen path.

### Negative
- **Firmware console complexity**: Logs must be framed and multiplexed on the shared USB-Serial-JTAG link. A raw text console is unavailable during operation (though UART on GPIO can provide an alternative if needed).
- **Single transport per board**: The device cannot present both CDC and HID on the same USB-Serial-JTAG channel. M2 keyboard output cannot use USB HID unless TinyUSB is adopted or a second transport (BLE) is added.
- **M2 rework needed**: At M2, if USB HID is chosen for keyboard output, the byte-driver must be changed from USB-Serial-JTAG to TinyUSB (assuming it is unblocked). This is a known rework; Phase 2 accepts it.

## M2 Implications

**Important open decision for M2:**

USB HID typing requires TinyUSB (if USB is the chosen output channel) and therefore the irreversible USB_PHY_SEL eFuse burn. This is a strong argument to consider alternatives for M2:

- **BLE HID**: Wireless, no eFuse cost, but pairing/bonding complexity and power draw. Worth serious evaluation.
- **Dedicated typing unit**: A second board with TinyUSB+HID reserved for typing, leaving the primary device on USB-Serial-JTAG for vault sync and reads. Acceptable if the product story is "carry two units" (one for browsing, one for typing).
- **Keyboard only via BLE, vault via USB**: Hybrid approach (vault over USB-Serial-JTAG, typing over BLE HID) avoids eFuse burn but adds wireless complexity and dual-transport management.

**No decision is pre-determined; all paths remain open and will be re-evaluated at M2.**

## Related Decisions

- **[2026-08-17-device-link-serial-framing-protocol.md](2026-08-17-device-link-serial-framing-protocol.md)**: The device-link protocol crate and framing system that rides on top of this transport (USB-Serial-JTAG now, flexible to other byte-drivers later).

## References

- **Owners**: Andreas (decision-maker), Ada (architect), Ruby (rust-embedded-supervisor)
- **Spike bead**: `ai-bitwarden-hw-key-8bi` (feasibility spike for USB transport options)
- **Spike findings**: Spike branch with TinyUSB POC (demonstrates the `esp-idf-sys#377` block)
- **Hardware**: Lilygo T-Embed CC1101 (single USB-C, internal USB PHY, GPIO19/20 connected; no external USB PHY per schematic)
- **Upstream issue**: `esp-rs/esp-idf-sys#377` (TinyUSB bindgen block, June 2025, unresolved)
- **M2 preview**: [./roadmap.md](../roadmap.md) references M2 keyboard output and notes the eFuse cost / BLE alternative
