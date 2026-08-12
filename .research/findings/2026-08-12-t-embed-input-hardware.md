# T-Embed Input Hardware: Rotary Encoder Pinouts and Variants

**Date**: 2026-08-12
**Status**: Complete
**Confidence**: High

## Hardware Summary

Both Lilygo T-Embed variants (plain and CC1101) use a single **rotary encoder wheel** with an integrated center push-button for input. There is NO directional button cluster (d-pad or directional buttons); input is limited to encoder rotation and press.

## Plain T-Embed Pinout

**Encoder wheel**: Quadrature rotary encoder, approximately 24 detents on the standard variant, 1 integrated push-button.

**GPIO Mapping** (from LilyGo factory examples, `factory/pin_config.h`):
- `PIN_ENCODE_A = GPIO2` (encoder contact A, quadrature line)
- `PIN_ENCODE_B = GPIO1` (encoder contact B, quadrature line)
- `PIN_ENCODE_BTN = GPIO0` (encoder center button)

**Additional I/O**:
- `GPIO0` is also the ESP32-S3 **BOOT strapping pin**. Holding GPIO0 low during power-on enters the ROM bootloader (download mode). Any UX that involves holding the encoder button during power-up will trigger download mode; firmware design must account for this.
- One readable push-button (typically labeled "RST" or similar) + standard ESP32-S3 EN/RESET pin.

## T-Embed CC1101 Pinout

**Encoder wheel**: Same as plain variant, quadrature, 1 integrated push-button. Detent count unverified (estimated ~24, same as plain).

**GPIO Mapping** (from T-Embed-CC1101 examples/utilities.h and docs/pinmap_cn.md):
- `ENCODER_INA = GPIO4` (encoder contact A)
- `ENCODER_INB = GPIO5` (encoder contact B)
- `ENCODER_KEY = GPIO0` (encoder center button; also BOOT pin)

**Additional I/O**:
- `BOARD_USER_KEY = GPIO6` (independent user button, separate from encoder press)
- One readable push-button + EN/RESET pin
- CC1101 radio module occupies SPI pins (not relevant to input)

**GPIO0 caveat**: Same as plain variant; it is the BOOT pin.

## Current Firmware Target

The project's firmware (`firmware/src/board/board_config.rs`) currently targets the **plain T-Embed**:
- GPIO2 = encoder A
- GPIO1 = encoder B
- GPIO0 = encoder button + BOOT

This matches the LilyGo factory source and is the primary development target.

## Encoder Hardware Notes

### Quadrature Decoding
Both variants emit standard quadrature signals (A, B leads 90 degrees apart). The firmware input driver decodes ticks as:
- CW rotation: A rise before B rise (or symmetric decoding)
- CCW rotation: B rise before A rise

### Reliability Caveat
Cheap 2-contact encoders (common on low-cost devboards) are prone to:
- Missed or spurious steps due to contact bounce
- Inconsistent detent feel (mechanical variance between units)

**Mitigation**: The input driver should include debouncing (e.g., 10-50ms gate to filter noise) and be tolerant of dropped ticks. High-speed rotation that loses a tick or two should not break scrolling logic.

### Press Behavior
The integrated center push-button is a standard momentary switch (electrical contact when pressed, open when released). Debouncing is required; recommended debounce window is 20-50ms.

## Sources

- **Plain T-Embed**: github.com/Xinyuan-LilyGO/T-Embed (main examples/factory/pin_config.h)
- **T-Embed CC1101**: github.com/Xinyuan-LilyGO/T-Embed-CC1101 (examples/utilities.h, docs/pinmap_cn.md)
- **LilyGo Wiki**: wiki.lilygo.cc (T-Embed and T-Embed-CC1101 pages; community-maintained, occasionally outdated)
- **ESP Boards Database**: espboards.dev (community pinout reference; T-Embed and T-Embed-CC1101 entries)
- **CC1101 Encoder Reliability**: T-Embed-CC1101 GitHub issue #29 (user reports of spurious steps, recommends debouncing)

## Firmware Implications

1. **Board selection** is a build-time concern (e.g., feature flags or board selection in Cargo.toml). The app layer (`core`) remains board-agnostic.
2. **Input driver** (firmware/src/input_driver/) must debounce both quadrature and press signals.
3. **NavIntent** abstraction (from `2026-08-11-rotary-encoder-input-model.md`) shields the app from these details.
4. **GPIO0/BOOT conflict**: Any firmware update or test that might hold GPIO0 low should be sequenced after the device boots to avoid accidental entry into download mode.

## Testing Checklist for Future Hardware Changes

- Verify encoder ticks are decoded correctly (CW/CCW)
- Verify press is detected reliably (no ghost activations or missed presses)
- Check for bounce artifacts in high-frequency scenarios (fast rotation)
- Confirm GPIO0 BOOT conflict does not cause unexpected resets during normal UX
