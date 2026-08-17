//! Fixed binary frame header + CRC32 framing.
//!
//! Wire layout (all multi-byte integers little-endian unless noted):
//!
//! ```text
//! magic: [u8; 2]   = 0xB1 0x7C   (resync anchor)
//! type:  u8                      (MessageType)
//! flags: u8                      (bit0 = MORE; rest reserved, must be 0)
//! len:   u32 LE                  (payload length in bytes)
//! payload: [u8; len]
//! crc32: u32 LE                  (over type|flags|len(LE bytes)|payload)
//! ```
//!
//! USB CDC already guarantees reliable, in-order, uncorrupted byte
//! delivery, so this framing exists purely for **message delimiting**
//! (multiple messages sharing one byte stream), **resync** (skipping past
//! ESP-IDF boot-log text that lands on the wire before the framer starts
//! listening), and **desync detection** (catching a misparsed header), not
//! for forward error correction.

use crate::message::MessageType;

/// Two-byte anchor written at the start of every frame.
pub const MAGIC: [u8; 2] = [0xB1, 0x7C];

/// Bit 0 of the flags byte: payload is a chunk fragment, more chunks follow
/// for the same logical blob. Bits 1-7 are reserved and must be zero.
pub const FLAG_MORE: u8 = 0b0000_0001;

/// Hard cap on a single frame's payload length. A message larger than this
/// must be split into chunks (see the `chunk` module) rather than sent as
/// one oversized frame.
pub const MAX_PAYLOAD_LEN: usize = 8 * 1024;

/// Fixed header size in bytes: magic(2) + type(1) + flags(1) + len(4).
pub const HEADER_LEN: usize = 2 + 1 + 1 + 4;

/// Trailer size in bytes: crc32(4).
pub const CRC_LEN: usize = 4;

/// Errors that can occur while encoding a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// Payload exceeds [`MAX_PAYLOAD_LEN`]; split it with the `chunk`
    /// module instead of encoding it as a single frame.
    PayloadTooLarge { len: usize, max: usize },
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodeError::PayloadTooLarge { len, max } => {
                write!(f, "payload too large: {len} bytes exceeds cap of {max} bytes")
            }
        }
    }
}

impl std::error::Error for EncodeError {}

/// A decoded frame: message type, flags, and opaque payload bytes.
/// Structured payloads (CBOR, or the fixed sub-headers used by
/// `FramebufferData`) are decoded separately via `message` module helpers
/// -- this type only deals with the framing layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub msg_type: MessageType,
    pub flags: u8,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Whether this frame's `MORE` bit is set (part of a chunked sequence,
    /// more frames follow for the same logical blob).
    #[must_use]
    pub fn more(&self) -> bool {
        self.flags & FLAG_MORE != 0
    }
}

/// CRC32 (CRC-32/ISO-HDLC, the common zlib/PNG/Ethernet polynomial) over
/// `type|flags|len(LE bytes)|payload`, in that order. Shared by the encoder
/// and decoder so they can never disagree about what's covered.
pub(crate) fn crc32(type_byte: u8, flags: u8, len_bytes: [u8; 4], payload: &[u8]) -> u32 {
    use crc::{Crc, CRC_32_ISO_HDLC};
    const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    let mut digest = CRC.digest();
    digest.update(&[type_byte, flags]);
    digest.update(&len_bytes);
    digest.update(payload);
    digest.finalize()
}

/// Encode a single frame: header + payload + trailing CRC32.
///
/// For payloads larger than [`MAX_PAYLOAD_LEN`], use
/// [`crate::chunk::encode_chunks`] instead, which produces a sequence of
/// frames of the same `msg_type` linked by [`FLAG_MORE`].
///
/// # Errors
///
/// Returns [`EncodeError::PayloadTooLarge`] if `payload.len() >
/// MAX_PAYLOAD_LEN`.
pub fn encode_frame(msg_type: MessageType, flags: u8, payload: &[u8]) -> Result<Vec<u8>, EncodeError> {
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(EncodeError::PayloadTooLarge {
            len: payload.len(),
            max: MAX_PAYLOAD_LEN,
        });
    }
    let type_byte: u8 = msg_type.into();
    // Guarded above: payload.len() <= MAX_PAYLOAD_LEN (8 KiB) always fits in
    // a u32, so this cast never truncates.
    #[allow(clippy::cast_possible_truncation)]
    let len_bytes = (payload.len() as u32).to_le_bytes();
    let crc = crc32(type_byte, flags, len_bytes, payload);

    let mut out = Vec::with_capacity(HEADER_LEN + payload.len() + CRC_LEN);
    out.extend_from_slice(&MAGIC);
    out.push(type_byte);
    out.push(flags);
    out.extend_from_slice(&len_bytes);
    out.extend_from_slice(payload);
    out.extend_from_slice(&crc.to_le_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_too_large_is_rejected() {
        let big = vec![0u8; MAX_PAYLOAD_LEN + 1];
        let err = encode_frame(MessageType::Ping, 0, &big).unwrap_err();
        assert_eq!(
            err,
            EncodeError::PayloadTooLarge {
                len: MAX_PAYLOAD_LEN + 1,
                max: MAX_PAYLOAD_LEN
            }
        );
    }

    #[test]
    fn max_payload_len_is_accepted() {
        let max = vec![0xABu8; MAX_PAYLOAD_LEN];
        assert!(encode_frame(MessageType::Ping, 0, &max).is_ok());
    }

    #[test]
    fn encoded_frame_layout_matches_spec() {
        let payload = [1u8, 2, 3];
        let bytes = encode_frame(MessageType::Ping, FLAG_MORE, &payload).unwrap();
        assert_eq!(&bytes[0..2], &MAGIC);
        assert_eq!(bytes[2], u8::from(MessageType::Ping));
        assert_eq!(bytes[3], FLAG_MORE);
        assert_eq!(&bytes[4..8], &3u32.to_le_bytes());
        assert_eq!(&bytes[8..11], &payload);
        assert_eq!(bytes.len(), HEADER_LEN + 3 + CRC_LEN);
    }
}
