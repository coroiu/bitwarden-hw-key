//! Streaming decoder: accumulates bytes from the wire (which may arrive in
//! any chunk size -- one byte at a time, or in large bursts) and yields
//! decoded frames as soon as a complete, CRC-valid frame is available.
//!
//! Resync: [`Decoder::poll`] scans for the magic anchor rather than
//! assuming the stream starts on a frame boundary. This is what lets a
//! receiver skip past ESP-IDF boot-log text that lands on the same
//! UART/USB-CDC byte stream before firmware installs the framer, and
//! recover from a corrupted or misparsed header without wedging forever.

use crate::frame::{crc32, Frame, CRC_LEN, HEADER_LEN, MAGIC, MAX_PAYLOAD_LEN};
use crate::message::MessageType;

/// Errors [`Decoder::poll`] can surface. All are *recoverable*: the
/// decoder consumes just enough bytes to make progress, and callers should
/// keep polling in a loop. The decoder never gets stuck waiting for bytes
/// that will never arrive because of a bogus length or corrupt header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Header declared a payload length beyond [`MAX_PAYLOAD_LEN`]. Almost
    /// certainly a false-positive magic-word match inside binary payload
    /// data, or stream corruption. The decoder has already skipped past
    /// the offending magic bytes, so the next `poll` resumes scanning.
    OversizedLen { declared: u32, max: usize },
    /// The frame parsed structurally (length was in range and enough bytes
    /// were available) but its CRC32 didn't match. The decoder has already
    /// skipped past the offending magic bytes.
    CrcMismatch,
    /// A CRC-valid frame with a message type byte this crate doesn't
    /// recognize. The frame's bytes have already been fully consumed (it
    /// wasn't corrupt, just an unrecognized type).
    UnknownMessageType(u8),
}

/// Accumulates raw bytes and extracts frames. A plain buffer, not
/// thread-safe by itself -- callers own one instance per link direction.
#[derive(Debug, Default)]
pub struct Decoder {
    buf: Vec<u8>,
}

impl Decoder {
    #[must_use]
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Append newly-received bytes to the internal buffer. Cheap; does not
    /// itself attempt to decode -- call [`Decoder::poll`] after.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Number of bytes currently buffered and not yet consumed into a
    /// decoded frame or error. Exposed mainly for tests/diagnostics.
    #[must_use]
    pub fn buffered_len(&self) -> usize {
        self.buf.len()
    }

    /// Attempt to extract the next frame (or surface the next decode
    /// error) from the buffered bytes.
    ///
    /// Returns `None` when there isn't enough data yet for another frame
    /// -- callers should `feed` more bytes and `poll` again. Always drain
    /// with a `while let Some(result) = poll()` loop: a single `feed` can
    /// make multiple frames (or a frame plus a resync error) available at
    /// once.
    pub fn poll(&mut self) -> Option<Result<Frame, DecodeError>> {
        // 1. Find the magic anchor.
        let magic_pos = self.buf.windows(MAGIC.len()).position(|w| w == MAGIC);
        let Some(pos) = magic_pos else {
            // No full magic present. Keep a possible partial match (a
            // single trailing byte equal to MAGIC[0]) so it isn't lost
            // across a `feed` boundary; drop everything else -- it's
            // garbage (boot text or otherwise) we're resyncing past.
            if let Some(&last) = self.buf.last().filter(|&&b| b == MAGIC[0]) {
                self.buf.clear();
                self.buf.push(last);
            } else {
                self.buf.clear();
            }
            return None;
        };
        if pos > 0 {
            // Drop garbage before the anchor; buf now starts with MAGIC.
            self.buf.drain(0..pos);
        }

        // 2. Do we have the full fixed header yet?
        if self.buf.len() < HEADER_LEN {
            return None;
        }
        let type_byte = self.buf[2];
        let flags = self.buf[3];
        let len_bytes = [self.buf[4], self.buf[5], self.buf[6], self.buf[7]];
        let len = u32::from_le_bytes(len_bytes);

        if len as usize > MAX_PAYLOAD_LEN {
            // Almost certainly a false-positive magic match inside payload
            // bytes rather than a real header. Skip past just the magic
            // word (not the bogus "header") and let the next poll() keep
            // scanning for a genuine frame start.
            self.buf.drain(0..MAGIC.len());
            return Some(Err(DecodeError::OversizedLen { declared: len, max: MAX_PAYLOAD_LEN }));
        }
        let len = len as usize;

        // 3. Do we have the full frame (header + payload + crc) yet?
        let total_len = HEADER_LEN + len + CRC_LEN;
        if self.buf.len() < total_len {
            return None;
        }

        let payload = self.buf[HEADER_LEN..HEADER_LEN + len].to_vec();
        let crc_bytes = &self.buf[HEADER_LEN + len..total_len];
        let stored_crc = u32::from_le_bytes([crc_bytes[0], crc_bytes[1], crc_bytes[2], crc_bytes[3]]);
        let computed_crc = crc32(type_byte, flags, len_bytes, &payload);

        if computed_crc != stored_crc {
            self.buf.drain(0..MAGIC.len());
            return Some(Err(DecodeError::CrcMismatch));
        }

        // Frame is structurally and CRC valid -- consume it in full
        // regardless of whether the message type is recognized.
        self.buf.drain(0..total_len);

        Some(match MessageType::try_from(type_byte) {
            Ok(msg_type) => Ok(Frame { msg_type, flags, payload }),
            Err(unknown) => Err(DecodeError::UnknownMessageType(unknown.0)),
        })
    }
}

#[cfg(test)]
// Test fixtures cast small, known-in-range literals/lengths to u32; never a
// real truncation risk, and `try_from`+`unwrap` would only add noise here.
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use super::*;
    use crate::frame::{encode_frame, FLAG_MORE};

    #[test]
    fn empty_decoder_yields_nothing() {
        let mut d = Decoder::new();
        assert_eq!(d.poll(), None);
    }

    #[test]
    fn single_frame_roundtrip() {
        let bytes = encode_frame(MessageType::Ping, 0, &[]).unwrap();
        let mut d = Decoder::new();
        d.feed(&bytes);
        let frame = d.poll().unwrap().unwrap();
        assert_eq!(frame.msg_type, MessageType::Ping);
        assert_eq!(frame.flags, 0);
        assert!(frame.payload.is_empty());
        assert_eq!(d.poll(), None);
        assert_eq!(d.buffered_len(), 0);
    }

    #[test]
    fn roundtrip_all_message_types_with_nonempty_payload() {
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
            let payload = format!("payload for {mt:?}").into_bytes();
            let bytes = encode_frame(mt, FLAG_MORE, &payload).unwrap();
            let mut d = Decoder::new();
            d.feed(&bytes);
            let frame = d.poll().unwrap().unwrap();
            assert_eq!(frame.msg_type, mt);
            assert_eq!(frame.flags, FLAG_MORE);
            assert!(frame.more());
            assert_eq!(frame.payload, payload);
            assert_eq!(d.poll(), None);
        }
    }

    #[test]
    fn two_frames_back_to_back() {
        let a = encode_frame(MessageType::Ping, 0, b"a").unwrap();
        let b = encode_frame(MessageType::Pong, 0, b"bb").unwrap();
        let mut d = Decoder::new();
        d.feed(&a);
        d.feed(&b);
        let f1 = d.poll().unwrap().unwrap();
        assert_eq!(f1.msg_type, MessageType::Ping);
        assert_eq!(f1.payload, b"a");
        let f2 = d.poll().unwrap().unwrap();
        assert_eq!(f2.msg_type, MessageType::Pong);
        assert_eq!(f2.payload, b"bb");
        assert_eq!(d.poll(), None);
    }

    #[test]
    fn resyncs_past_leading_boot_text_garbage() {
        let mut stream = b"I (123) boot: ESP-IDF v5.1 starting up...\r\nGarbage\xB1\x00more junk".to_vec();
        let frame_bytes = encode_frame(MessageType::Ping, 0, b"hello").unwrap();
        stream.extend_from_slice(&frame_bytes);

        let mut d = Decoder::new();
        d.feed(&stream);
        // Any decode errors surfaced while resyncing past the garbage
        // (e.g. the lone 0xB1 byte above isn't a real magic match since
        // it's not followed by 0x7C) shouldn't prevent the real frame
        // afterwards from being found -- drain them.
        let mut got_frame = None;
        while let Some(result) = d.poll() {
            if let Ok(frame) = result {
                got_frame = Some(frame);
            }
        }
        let frame = got_frame.expect("expected the real frame to be found after resync");
        assert_eq!(frame.msg_type, MessageType::Ping);
        assert_eq!(frame.payload, b"hello");
    }

    #[test]
    fn resyncs_past_a_real_magic_byte_pair_embedded_in_garbage() {
        // Embed a literal MAGIC sequence in the "garbage" region that is
        // NOT a valid frame (arbitrary trailing bytes with no valid
        // header/crc). The decoder must detect this isn't a real frame
        // (oversized len or crc mismatch) and keep scanning until it finds
        // the actual valid frame later in the stream.
        let mut stream = vec![0x00, 0x01, 0x02];
        stream.extend_from_slice(&crate::frame::MAGIC); // fake anchor
        stream.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF]); // bogus type/flags/len
        stream.extend_from_slice(b"trailing noise that is not a real frame");

        let real = encode_frame(MessageType::Log, 0, b"log line").unwrap();
        stream.extend_from_slice(&real);

        let mut d = Decoder::new();
        d.feed(&stream);
        let mut frames = Vec::new();
        let mut errors = Vec::new();
        while let Some(result) = d.poll() {
            match result {
                Ok(f) => frames.push(f),
                Err(e) => errors.push(e),
            }
        }
        assert!(!errors.is_empty(), "expected at least one resync error from the fake anchor");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].msg_type, MessageType::Log);
        assert_eq!(frames[0].payload, b"log line");
    }

    #[test]
    fn crc_mismatch_is_detected_and_resynced_past() {
        let mut bytes = encode_frame(MessageType::Ping, 0, b"hello").unwrap();
        // Flip a payload byte after CRC computation -> CRC now mismatches.
        let payload_start = HEADER_LEN;
        bytes[payload_start] ^= 0xFF;

        let good = encode_frame(MessageType::Pong, 0, b"world").unwrap();
        let mut stream = bytes.clone();
        stream.extend_from_slice(&good);

        let mut d = Decoder::new();
        d.feed(&stream);

        let first = d.poll().unwrap();
        assert_eq!(first, Err(DecodeError::CrcMismatch));

        // Keep polling: after a CRC mismatch we only skip 2 bytes (the
        // magic), so we may see further errors as we crawl through what's
        // left of the corrupted frame before reaching the good one.
        let mut recovered = None;
        while let Some(result) = d.poll() {
            if let Ok(frame) = result {
                recovered = Some(frame);
                break;
            }
        }
        let frame = recovered.expect("should eventually resync onto the valid frame");
        assert_eq!(frame.msg_type, MessageType::Pong);
        assert_eq!(frame.payload, b"world");
    }

    #[test]
    fn oversized_len_is_rejected() {
        // Hand-craft a header claiming a payload beyond MAX_PAYLOAD_LEN.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MAGIC);
        bytes.push(u8::from(MessageType::Ping));
        bytes.push(0);
        bytes.extend_from_slice(&((MAX_PAYLOAD_LEN as u32) + 1).to_le_bytes());
        // No payload/crc bytes follow -- the decoder must reject based on
        // the declared length alone, without waiting for more data.

        let mut d = Decoder::new();
        d.feed(&bytes);
        let result = d.poll().unwrap();
        assert_eq!(
            result,
            Err(DecodeError::OversizedLen { declared: (MAX_PAYLOAD_LEN as u32) + 1, max: MAX_PAYLOAD_LEN })
        );
        // Decoder should have skipped past the magic and not be stuck.
        assert_eq!(d.buffered_len(), bytes.len() - MAGIC.len());
    }

    #[test]
    fn unknown_message_type_consumes_frame_and_reports_error() {
        // Message type 0x00 isn't assigned. Hand-craft a CRC-valid frame
        // with it so we can confirm the decoder distinguishes "well-formed
        // but unrecognized" from "corrupt."
        let payload = b"abc";
        let type_byte = 0x00u8;
        let flags = 0u8;
        let len_bytes = (payload.len() as u32).to_le_bytes();
        let crc = crc32(type_byte, flags, len_bytes, payload);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MAGIC);
        bytes.push(type_byte);
        bytes.push(flags);
        bytes.extend_from_slice(&len_bytes);
        bytes.extend_from_slice(payload);
        bytes.extend_from_slice(&crc.to_le_bytes());

        let good = encode_frame(MessageType::Ping, 0, b"ok").unwrap();
        let mut stream = bytes.clone();
        stream.extend_from_slice(&good);

        let mut d = Decoder::new();
        d.feed(&stream);
        assert_eq!(d.poll(), Some(Err(DecodeError::UnknownMessageType(0x00))));
        // The whole (valid) unknown frame was consumed, so the next poll
        // goes straight to the next real frame with no further errors.
        let frame = d.poll().unwrap().unwrap();
        assert_eq!(frame.msg_type, MessageType::Ping);
        assert_eq!(frame.payload, b"ok");
    }

    #[test]
    fn partial_reads_reassemble_byte_by_byte() {
        let payload = b"this payload is split across many tiny feeds";
        let bytes = encode_frame(MessageType::SyncChunk, FLAG_MORE, payload).unwrap();

        let mut d = Decoder::new();
        let mut decoded = None;
        for byte in &bytes {
            d.feed(std::slice::from_ref(byte));
            if let Some(result) = d.poll() {
                decoded = Some(result.unwrap());
            }
        }
        let frame = decoded.expect("frame should complete once the last byte arrives");
        assert_eq!(frame.msg_type, MessageType::SyncChunk);
        assert_eq!(frame.payload, payload);
        assert!(frame.more());
    }

    #[test]
    fn partial_magic_split_across_feeds_is_not_lost() {
        // Feed the magic's first byte alone, then the rest of a valid
        // frame in a second feed -- the decoder must not have discarded
        // the partial magic byte between the two feeds.
        let bytes = encode_frame(MessageType::Ping, 0, b"z").unwrap();
        let mut d = Decoder::new();
        d.feed(&bytes[0..1]);
        assert_eq!(d.poll(), None);
        d.feed(&bytes[1..]);
        let frame = d.poll().unwrap().unwrap();
        assert_eq!(frame.msg_type, MessageType::Ping);
        assert_eq!(frame.payload, b"z");
    }
}
