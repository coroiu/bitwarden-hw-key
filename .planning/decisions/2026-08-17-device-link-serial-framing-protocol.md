# Device-Link Serial Framing Protocol and Multiplexed Message Types

**Date**: 2026-08-17
**Status**: Accepted

## Context

M1.5 Phase 2 requires a wire protocol over the serial byte stream (USB-Serial-JTAG in Phase 2, any byte stream later) that carries three concerns on a single shared link:

1. **Vault sync**: credentials pushed from companion to device for storage.
2. **Verify-seam**: navigation intent injection and framebuffer capture, enabling agents to drive and observe the real device (closes bead `ai-bitwarden-hw-key-dvm`).
3. **Device logs**: diagnostic and status messages from the firmware.

Both the firmware and the host web-companion depend on a common, portable protocol to avoid data corruption and protocol confusion on the shared link.

### Design Constraints

- **Byte-stream agnostic**: The protocol should work over USB-Serial-JTAG now and any byte-transport later (BLE, Wi-Fi, TCP).
- **Multiplexing**: Multiple message types (sync, verify, logs) share one link; must be unambiguous.
- **Robustness**: Recover from bit-flips, boot text, or transient noise. Magic bytes and CRC enable resync.
- **Reuse**: Avoid inventing a second data model; reuse existing push-protocol types where possible (SyncRequest, SyncResponse, Credential).
- **Agent-verifiable**: The verify-seam messages enable headless agents to inject NavIntent and capture framebuffers, treating the real device like the emulator.

## Decision

A portable `device-link` crate (merged to main, bead `ai-bitwarden-hw-key-2ox.1`) defining:

### Wire Format

**Fixed binary frame header:**
- Magic: `0xB1 0x7C` (2 bytes, recognizable, unambiguous)
- Type: u8 (message variant)
- Flags: u8 (bit 0: MORE flag for multi-frame payloads)
- Length: u32 LE (payload byte count)
- Payload: variable (0 to 2^32-1 bytes)
- CRC32: u32 LE (computed over type | flags | length | payload)

**Example:**
```
B1 7C 02 00 | 00 00 00 15 | [21 bytes of CBOR payload] | [CRC32]
```

### Message Multiplex (11 Variants)

**Host to Device:**
- `SyncBegin`: initiates a vault sync; includes metadata (item count, version).
- `SyncChunk`: one chunk of the sync payload (may be fragmented; MORE flag indicates continuation).
- `SyncEnd`: signals end of sync; device performs storage and state transitions.
- `InputInject`: Injects a `WireIntent` (navigation command) for testing / agent-driven verification.
- `FramebufferRequest`: Requests a current framebuffer dump (returns FramebufferData).
- `Ping`: Keep-alive; device responds with Pong.

**Device to Host:**
- `SyncAck`: Sync completed successfully; device is ready for next command.
- `SyncNack`: Sync failed (e.g., storage error, CRC mismatch); optional error details.
- `FramebufferData`: Framebuffer dump (320x170 Rgb565 = ~108KB; sent in multiple frames with MORE flag).
- `Log`: Diagnostic message (timestamped, level, text).
- `Pong`: Response to Ping.

### Payloads (CBOR-Encoded)

- **SyncBegin / SyncChunk / SyncEnd** reuse `push_protocol::{SyncRequest, SyncResponse}` to avoid inventing a second data model.
- **Credential storage** reuses `push_protocol::Credential` (name, username, password, url, notes).
- **InputInject** uses a `WireIntent` enum (Prev, Next, Activate, Back, NextN) that mirrors `bhk_core::NavIntent` and is mapped at the firmware boundary (established wire/domain-split pattern, same as Credential vs VaultItem).
- **FramebufferData** payload is the raw Rgb565 framebuffer bytes (serialized as a CBOR byte array).
- **Log** payload is a struct: timestamp (u64 ms since epoch), level (Debug/Info/Warn/Error), text (string).

### Streaming and Reassembly

- **Decoder**: Scans for magic bytes, validates frame header, checks CRC, reassembles multi-frame payloads (MORE flag), tolerates partial reads (buffering until a complete frame arrives).
- **Reassembler**: Collects chunks from SyncChunk messages; returns the complete CBOR payload when SyncEnd is received.
- **Resync on magic**: If a frame is corrupted or out-of-sync, the decoder resumes scanning for the next magic byte.
- **CRC validation**: Each frame's CRC is checked; malformed frames are dropped and a resync is logged (device can retry; if host doesn't hear SyncAck, it can assume failure).

## Rationale

- **Binary header for robust delimiting**: Raw text console output before the protocol starts (e.g., ESP-IDF boot messages) would corrupt a text-based framing scheme. A magic-byte-based binary header (0xB1 0x7C) is unambiguous and easy to resync.
- **Reuse push_protocol payloads**: The sync data model (SyncRequest, SyncResponse, Credential) is already defined and tested. Reusing it avoids duplicating the wire contract and keeps the device-link crate lightweight.
- **Coarse end-to-end acking, not windowed per-chunk**: The link is already reliable (USB CDC guarantees in-order, lossless delivery). A single SyncAck/SyncNack at SyncEnd is sufficient; per-chunk windowing would add complexity without benefit on a reliable link.
- **WireIntent as a lightweight wire/domain split**: NavIntent (app-domain) and WireIntent (wire-domain) follow the same pattern as Credential (wire) vs VaultItem (app-domain). This keeps device-link's dependency tree lean: push_protocol, serde, ciborium, crc, only. NO bhk-core, which would drag embedded-graphics into the web-companion server and violate the boundary documented in ADR `eml.1`.
- **Verify-seam integration**: InputInject and FramebufferRequest/FramebufferData messages enable agents to drive and inspect the real device via the same link, closing the verification gap (bead dvm). The real device is exercisable in headless mode, not just windowed/emulated.

## Alternatives Considered

1. **Reuse the HTTP/CBOR push protocol verbatim.**
   - **Pros**: Minimal changes; single protocol stack.
   - **Cons**: HTTP is a text-based, request-response protocol; it does not work over a raw byte stream (no TCP). CBOR-on-HTTP still needs framing (Content-Length or chunked encoding) and relies on HTTP semantics (request/response pairing). On a raw USB-Serial-JTAG stream with boot text and logs, HTTP framing breaks.
   - **Verdict**: Rejected. HTTP is not suitable for raw byte streams.

2. **Use COBS (Consistent Overhead Byte Stuffing) framing.**
   - **Pros**: Simple framing; no magic bytes needed; reduces overhead.
   - **Cons**: USB CDC (the actual physical link) already guarantees in-order, lossless delivery. COBS is designed for unreliable links; it adds complexity without benefit here. Magic bytes + CRC are simpler and still efficient.
   - **Verdict**: Rejected. Over-engineered for a reliable link.

3. **Depend on bhk_core for NavIntent; skip the WireIntent wire/domain split.**
   - **Pros**: One code path; less duplication.
   - **Cons**: bhk_core imports embedded-graphics. The web-companion server is server-side only (no rendering, no UI framework on the host). Pulling embedded-graphics into the web-companion server violates the boundary documented in eml.1 and adds unnecessary dependencies. WireIntent is a thin enum; the cost of a wire/domain split is negligible.
   - **Verdict**: Rejected. Maintain the clean boundary; use WireIntent.

## Consequences

### Positive
- **Durable wire contract**: Both firmware and web-companion depend on the device-link protocol crate. Changes to the wire format require coordination and versioning; the contract is explicit and testable.
- **Verify-seam enabled**: Agents can inject NavIntent and request framebuffers via InputInject and FramebufferRequest, enabling real-target verification as described in bead dvm. The same headless testing strategy that works on the emulator now works on the real device.
- **Lean dependency tree**: device-link depends only on push_protocol (SyncRequest/Response/Credential), serde/ciborium (CBOR), crc32, and standard library. No platform-specific or UI dependencies.
- **Multiplexing clarity**: Three concerns (sync, verify, logs) are unambiguously separated; no confusion about what data is flowing.
- **Transport-agnostic foundation**: Swapping USB-Serial-JTAG for TinyUSB, BLE, or TCP at a later date only changes the byte-driver; the protocol above it is unchanged.

### Negative
- **Firmware console rework needed**: During normal operation (after boot), the firmware can no longer output raw text logs to the serial console. All logging must go through the `Log` message type on the shared link. A raw UART on GPIO19/20 (or a second debug port) can provide a fallback debug console if needed, but the primary serial link is owned by the protocol.
- **Multi-frame payloads add complexity**: Large framebuffer dumps (320x170 Rgb565, ~108KB) are split across multiple frames. The host and device both need buffering and reassembly logic. This is testable but non-trivial.
- **CRC per-frame overhead**: Each frame carries a 4-byte CRC. Over a reliable USB link, this is redundant (USB has its own error detection). The cost is acceptable for robustness, but it's acknowledged as not strictly necessary.

## Related Decisions

- **[2026-08-17-phase2-usb-transport-jtag-first.md](2026-08-17-phase2-usb-transport-jtag-first.md)**: This ADR describes the USB-Serial-JTAG transport that device-link rides on top of (in Phase 2). The protocol is transport-agnostic; the transport can change without affecting the framing or message types.
- **[2026-08-12-web-companion-local-server.md](2026-08-12-web-companion-local-server.md)** (eml.1): Defines the web-companion architecture and the boundary that device-link respects (server-side only, no host-side UI framework).

## References

- **Owners**: Andreas (decision-maker), Ada (architect), Ruby (rust-embedded-supervisor)
- **Implementation bead**: `ai-bitwarden-hw-key-2ox.1` (device-link crate, merged to main)
- **Protocol crate**: `src/push_protocol/device_link.rs` (or standalone `device-link/` crate if extracted)
- **Verify-seam bead**: `ai-bitwarden-hw-key-dvm` (agent-driven verification, enabled by this protocol)
- **Related epic**: `ai-bitwarden-hw-key-2ox` (M1.5 Phase 2, device transport and firmware sync handler)
