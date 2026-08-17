//! Message multiplex: one [`MessageType`] enum shared by both directions of
//! the host<->device link, plus the (de)serialization helpers for each
//! message's payload.
//!
//! Structured payloads are CBOR (ciborium). A few payloads are deliberately
//! *not* CBOR: `SyncChunk`'s payload IS a raw slice of the push-protocol
//! CBOR `SyncRequest` blob (not itself wrapped in another CBOR envelope),
//! `Log` is raw UTF-8, `FramebufferRequest`/`Ping` are empty, and
//! `FramebufferData` is a small fixed binary sub-header followed by a raw
//! big-endian pixel stream (CBOR-wrapping raw pixels would be pure
//! overhead with no benefit).

use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// One enum for both directions of the link. The decoder doesn't care
/// about direction; callers know which messages they expect to send or
/// receive. Host->Device values use the low byte range, Device->Host use
/// the high bit set (0x80+) purely as a mnemonic for humans reading a hex
/// dump -- the decoder treats them identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageType {
    // Host -> Device
    /// Announces an incoming CBOR `SyncRequest` blob (see [`SyncBegin`]),
    /// before any `SyncChunk` frames arrive.
    SyncBegin = 0x01,
    /// Raw slice of the CBOR `SyncRequest` blob. Chunked via `MORE`; see
    /// [`crate::chunk`].
    SyncChunk = 0x02,
    /// Closes a `SyncChunk` sequence; payload is CBOR [`SyncEnd`].
    SyncEnd = 0x03,
    /// CBOR-encoded [`WireIntent`].
    InputInject = 0x04,
    /// Empty payload; requests a `FramebufferData` sequence in reply.
    FramebufferRequest = 0x05,
    /// Empty payload; expects a `Pong` in reply.
    Ping = 0x06,

    // Device -> Host
    /// CBOR-encoded `push_protocol::SyncResponse`, reused verbatim.
    SyncAck = 0x81,
    /// CBOR [`SyncNack`]: sync failed, with a machine-readable code and a
    /// human-readable message.
    SyncNack = 0x82,
    /// Fixed [`FramebufferHeader`] sub-header (first frame only) followed
    /// by raw big-endian Rgb565 pixel bytes. Chunked via `MORE`.
    FramebufferData = 0x83,
    /// Raw UTF-8 log text.
    Log = 0x84,
    /// CBOR [`DeviceDescriptor`]; reply to `Ping`.
    Pong = 0x85,
}

impl From<MessageType> for u8 {
    fn from(value: MessageType) -> Self {
        value as u8
    }
}

/// A frame header declared a message type byte this crate doesn't
/// recognize. The frame itself was still CRC-valid (this isn't framing
/// corruption) -- it's either a newer protocol version's message this
/// build doesn't parse, or a genuine bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownMessageType(pub u8);

impl TryFrom<u8> for MessageType {
    type Error = UnknownMessageType;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0x01 => MessageType::SyncBegin,
            0x02 => MessageType::SyncChunk,
            0x03 => MessageType::SyncEnd,
            0x04 => MessageType::InputInject,
            0x05 => MessageType::FramebufferRequest,
            0x06 => MessageType::Ping,
            0x81 => MessageType::SyncAck,
            0x82 => MessageType::SyncNack,
            0x83 => MessageType::FramebufferData,
            0x84 => MessageType::Log,
            0x85 => MessageType::Pong,
            other => return Err(UnknownMessageType(other)),
        })
    }
}

/// Errors from CBOR (de)serialization of a structured payload.
#[derive(Debug)]
pub enum CborError {
    Encode(ciborium::ser::Error<std::io::Error>),
    Decode(ciborium::de::Error<std::io::Error>),
}

impl std::fmt::Display for CborError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CborError::Encode(e) => write!(f, "CBOR encode error: {e}"),
            CborError::Decode(e) => write!(f, "CBOR decode error: {e}"),
        }
    }
}

impl std::error::Error for CborError {}

/// Serialize a structured payload to CBOR bytes (the wire form used inside
/// a `Frame::payload` for every message type documented as CBOR above).
///
/// # Errors
///
/// Returns [`CborError::Encode`] if `value` cannot be CBOR-serialized.
pub fn to_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, CborError> {
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf).map_err(CborError::Encode)?;
    Ok(buf)
}

/// Deserialize a structured payload from CBOR bytes.
///
/// # Errors
///
/// Returns [`CborError::Decode`] if `bytes` isn't valid CBOR for `T`.
pub fn from_cbor<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CborError> {
    ciborium::from_reader(bytes).map_err(CborError::Decode)
}

/// Host -> Device, CBOR payload of [`MessageType::SyncBegin`]: announces
/// `total_bytes` of CBOR `SyncRequest` blob, containing `item_count`
/// credentials, arriving next as a `SyncChunk` sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncBegin {
    pub total_bytes: u32,
    pub item_count: u32,
}

/// Host -> Device, CBOR payload of [`MessageType::SyncEnd`]: closes a
/// `SyncChunk` sequence, carrying the CRC32 of the *whole* reassembled
/// blob so the receiver can confirm nothing was dropped/reordered/
/// truncated, independent of each frame's own per-frame CRC32.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncEnd {
    pub crc32_of_whole_blob: u32,
}

/// Host -> Device, CBOR payload of [`MessageType::InputInject`]: a
/// synthetic navigation input event for the agent verify-seam (WS5) to
/// drive the on-device UI without physical rotary-encoder hardware.
///
/// This is the WIRE representation, deliberately NOT a re-export of
/// `bhk_core::NavIntent`. `device-link` must not depend on `bhk-core`: this
/// crate is linked into web-companion's host-side (nested, stable-Rust)
/// workspace once WS4 wires up the host transport, and `bhk-core` pulls in
/// `embedded-graphics`/`embedded-graphics-framebuf`/`u8g2-fonts` for its
/// render-layer code -- a rendering stack a web server has no business
/// linking. `WireIntent` has variant-for-variant parity with
/// `bhk_core::NavIntent` by construction; the firmware (WS2/WS3, the only
/// place that sees both crates) maps `WireIntent <-> bhk_core::NavIntent`
/// at the edge, exactly like the existing `push_protocol::Credential <->
/// bhk_core::VaultItem` wire/domain split this codebase already uses (see
/// `emulator::credentials`'s `From<Credential> for VaultItem`). That's the
/// established precedent this follows, not a new pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireIntent {
    /// Move to next item (encoder CW, arrow down, button down).
    Next,
    /// Move to previous item (encoder CCW, arrow up, button up).
    Prev,
    /// Jump forward N items (fast encoder rotation, held button, Pg Dn).
    NextN(u16),
    /// Select the focused item (encoder short press, space, enter).
    Activate,
    /// Return to parent / dismiss modal (encoder long press, esc, back).
    Back,
}

/// Device -> Host, CBOR payload of [`MessageType::SyncNack`]: sync failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncNack {
    pub code: u16,
    pub message: String,
}

/// Device -> Host, CBOR payload of [`MessageType::Pong`]: identifies the
/// device and its panel in reply to a `Ping`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceDescriptor {
    pub name: String,
    pub fw_version: String,
    pub panel_w: u16,
    pub panel_h: u16,
}

/// Pixel format tag for [`FramebufferHeader::format`]. Only `Rgb565` is
/// defined today (matches the T-Embed's ST7789 panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PixelFormat {
    Rgb565 = 0,
}

impl From<PixelFormat> for u8 {
    fn from(value: PixelFormat) -> Self {
        value as u8
    }
}

impl TryFrom<u8> for PixelFormat {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(PixelFormat::Rgb565),
            other => Err(other),
        }
    }
}

/// Fixed 5-byte binary sub-header at the start of the *first* frame of a
/// `FramebufferData` sequence: width/height (LE u16) and a format tag
/// byte. Continuation frames (later frames of the same `MORE`-linked
/// sequence) carry raw pixel bytes only, with no repeated header -- exactly
/// like a `SyncChunk` continuation. Deliberately NOT CBOR: this precedes a
/// raw big-endian Rgb565 pixel stream, and CBOR-wrapping that stream would
/// add framing overhead per pixel for no benefit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramebufferHeader {
    pub width: u16,
    pub height: u16,
    pub format: PixelFormat,
}

/// Encoded size of [`FramebufferHeader`] in bytes.
pub const FRAMEBUFFER_HEADER_LEN: usize = 5;

/// Errors decoding a [`FramebufferHeader`] from raw bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramebufferHeaderError {
    /// Fewer than [`FRAMEBUFFER_HEADER_LEN`] bytes were available.
    TooShort,
    /// The format tag byte wasn't a recognized [`PixelFormat`].
    UnknownFormat(u8),
}

impl FramebufferHeader {
    /// Encode to the fixed 5-byte wire form.
    #[must_use]
    pub fn encode(&self) -> [u8; FRAMEBUFFER_HEADER_LEN] {
        let mut out = [0u8; FRAMEBUFFER_HEADER_LEN];
        out[0..2].copy_from_slice(&self.width.to_le_bytes());
        out[2..4].copy_from_slice(&self.height.to_le_bytes());
        out[4] = self.format.into();
        out
    }

    /// Decode the fixed 5-byte sub-header from the start of `bytes`,
    /// returning it along with the remaining (pixel data) bytes.
    ///
    /// # Errors
    ///
    /// Returns [`FramebufferHeaderError::TooShort`] if `bytes` is shorter
    /// than [`FRAMEBUFFER_HEADER_LEN`], or
    /// [`FramebufferHeaderError::UnknownFormat`] if the format tag byte
    /// isn't a recognized [`PixelFormat`].
    pub fn decode(bytes: &[u8]) -> Result<(Self, &[u8]), FramebufferHeaderError> {
        if bytes.len() < FRAMEBUFFER_HEADER_LEN {
            return Err(FramebufferHeaderError::TooShort);
        }
        let width = u16::from_le_bytes([bytes[0], bytes[1]]);
        let height = u16::from_le_bytes([bytes[2], bytes[3]]);
        let format =
            PixelFormat::try_from(bytes[4]).map_err(FramebufferHeaderError::UnknownFormat)?;
        Ok((
            FramebufferHeader { width, height, format },
            &bytes[FRAMEBUFFER_HEADER_LEN..],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_type_roundtrips_through_u8_for_all_variants() {
        let all = [
            MessageType::SyncBegin,
            MessageType::SyncChunk,
            MessageType::SyncEnd,
            MessageType::InputInject,
            MessageType::FramebufferRequest,
            MessageType::Ping,
            MessageType::SyncAck,
            MessageType::SyncNack,
            MessageType::FramebufferData,
            MessageType::Log,
            MessageType::Pong,
        ];
        for mt in all {
            let byte: u8 = mt.into();
            assert_eq!(MessageType::try_from(byte), Ok(mt));
        }
    }

    #[test]
    fn unknown_message_type_is_rejected() {
        assert_eq!(MessageType::try_from(0x00), Err(UnknownMessageType(0x00)));
        assert_eq!(MessageType::try_from(0xFF), Err(UnknownMessageType(0xFF)));
    }

    #[test]
    fn cbor_roundtrip_sync_begin() {
        let value = SyncBegin { total_bytes: 4096, item_count: 12 };
        let bytes = to_cbor(&value).unwrap();
        let decoded: SyncBegin = from_cbor(&bytes).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn cbor_roundtrip_wire_intent() {
        for intent in [
            WireIntent::Next,
            WireIntent::Prev,
            WireIntent::NextN(7),
            WireIntent::Activate,
            WireIntent::Back,
        ] {
            let bytes = to_cbor(&intent).unwrap();
            let decoded: WireIntent = from_cbor(&bytes).unwrap();
            assert_eq!(intent, decoded);
        }
    }

    #[test]
    fn framebuffer_header_roundtrip() {
        let header = FramebufferHeader { width: 320, height: 170, format: PixelFormat::Rgb565 };
        let encoded = header.encode();
        let (decoded, rest) = FramebufferHeader::decode(&encoded).unwrap();
        assert_eq!(header, decoded);
        assert!(rest.is_empty());
    }

    #[test]
    fn framebuffer_header_decode_with_trailing_pixel_bytes() {
        let header = FramebufferHeader { width: 4, height: 1, format: PixelFormat::Rgb565 };
        let mut wire = header.encode().to_vec();
        wire.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
        let (decoded, rest) = FramebufferHeader::decode(&wire).unwrap();
        assert_eq!(header, decoded);
        assert_eq!(rest, &[0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn framebuffer_header_too_short_is_rejected() {
        let short = [0u8; FRAMEBUFFER_HEADER_LEN - 1];
        assert_eq!(FramebufferHeader::decode(&short), Err(FramebufferHeaderError::TooShort));
    }

    #[test]
    fn framebuffer_header_unknown_format_is_rejected() {
        let mut wire = [0u8; FRAMEBUFFER_HEADER_LEN];
        wire[4] = 0x7F;
        assert_eq!(FramebufferHeader::decode(&wire), Err(FramebufferHeaderError::UnknownFormat(0x7F)));
    }
}
