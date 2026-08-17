//! Chunking: splits a payload too large for one frame into a sequence of
//! same-type frames linked by the `MORE` flag, and reassembles that
//! sequence back into the original bytes on the receiving side.
//!
//! Used for the `SyncChunk` sequence (splitting the push-protocol CBOR
//! `SyncRequest` blob) and equally applicable to a multi-frame
//! `FramebufferData` sequence (splitting a raw pixel stream that follows
//! the fixed sub-header).

use crate::frame::{encode_frame, EncodeError, FLAG_MORE, MAX_PAYLOAD_LEN};
use crate::message::MessageType;

/// Split `blob` into a sequence of encoded frames of `msg_type`, each
/// carrying up to `max_chunk_len` bytes of payload, with [`FLAG_MORE`] set
/// on every frame except the last. `max_chunk_len` is clamped to
/// `[1, MAX_PAYLOAD_LEN]`.
///
/// An empty `blob` still produces exactly one frame (empty payload, `MORE`
/// unset), so a [`Reassembler`] on the other end always sees a definite
/// end even for a zero-length blob.
///
/// # Errors
///
/// Returns [`EncodeError::PayloadTooLarge`] if `encode_frame` rejects a
/// chunk (shouldn't happen given the clamp above, but propagated rather
/// than unwrapped in case that invariant ever changes).
pub fn encode_chunks(
    msg_type: MessageType,
    blob: &[u8],
    max_chunk_len: usize,
) -> Result<Vec<Vec<u8>>, EncodeError> {
    let max_chunk_len = max_chunk_len.clamp(1, MAX_PAYLOAD_LEN);
    if blob.is_empty() {
        return Ok(vec![encode_frame(msg_type, 0, &[])?]);
    }
    let mut frames = Vec::new();
    let mut offset = 0;
    while offset < blob.len() {
        let end = (offset + max_chunk_len).min(blob.len());
        let is_last = end == blob.len();
        let flags = if is_last { 0 } else { FLAG_MORE };
        frames.push(encode_frame(msg_type, flags, &blob[offset..end])?);
        offset = end;
    }
    Ok(frames)
}

/// Reassembles a sequence of chunk payloads (as extracted from decoded
/// `Frame`s) back into the original blob. Feed payload+`more` pairs in
/// order; [`Reassembler::is_done`] becomes true once a frame without
/// `MORE` has been pushed.
#[derive(Debug, Default, Clone)]
pub struct Reassembler {
    buf: Vec<u8>,
    done: bool,
}

impl Reassembler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one chunk's payload bytes and whether its frame had `MORE`
    /// set.
    pub fn push(&mut self, payload: &[u8], more: bool) {
        self.buf.extend_from_slice(payload);
        if !more {
            self.done = true;
        }
    }

    /// Whether a terminal (non-`MORE`) chunk has been pushed yet.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Bytes reassembled so far, regardless of whether the sequence is
    /// complete. Mainly useful for progress reporting.
    #[must_use]
    pub fn partial(&self) -> &[u8] {
        &self.buf
    }

    /// Consumes the reassembler, returning the reassembled bytes if a
    /// terminal (non-`MORE`) chunk has been pushed, `None` otherwise.
    #[must_use]
    pub fn finish(self) -> Option<Vec<u8>> {
        self.done.then_some(self.buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::Decoder;

    #[test]
    fn single_chunk_for_small_blob() {
        let blob = b"tiny";
        let frames = encode_chunks(MessageType::SyncChunk, blob, 1024).unwrap();
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn empty_blob_produces_one_terminal_frame() {
        let frames = encode_chunks(MessageType::SyncChunk, &[], 16).unwrap();
        assert_eq!(frames.len(), 1);

        let mut decoder = Decoder::new();
        decoder.feed(&frames[0]);
        let frame = decoder.poll().unwrap().unwrap();
        assert!(!frame.more());
        assert!(frame.payload.is_empty());
    }

    #[test]
    fn split_and_reassemble_exact_multiple() {
        let blob: Vec<u8> = (0u8..=255).cycle().take(1000).collect();
        let chunk_len = 100;
        let frames = encode_chunks(MessageType::SyncChunk, &blob, chunk_len).unwrap();
        assert_eq!(frames.len(), 10);

        let mut decoder = Decoder::new();
        let mut reassembler = Reassembler::new();
        for f in &frames {
            decoder.feed(f);
        }
        while let Some(result) = decoder.poll() {
            let frame = result.unwrap();
            reassembler.push(&frame.payload, frame.more());
        }
        assert!(reassembler.is_done());
        assert_eq!(reassembler.finish().unwrap(), blob);
    }

    #[test]
    fn split_and_reassemble_non_exact_multiple() {
        let blob: Vec<u8> = (0u8..=200).collect(); // 201 bytes
        let chunk_len = 64;
        let frames = encode_chunks(MessageType::SyncChunk, &blob, chunk_len).unwrap();
        // 64*3=192, remainder 9 -> 4 chunks
        assert_eq!(frames.len(), 4);
        for f in &frames[..frames.len() - 1] {
            let mut d = Decoder::new();
            d.feed(f);
            assert!(d.poll().unwrap().unwrap().more());
        }
        {
            let mut d = Decoder::new();
            d.feed(&frames[frames.len() - 1]);
            assert!(!d.poll().unwrap().unwrap().more());
        }

        let mut decoder = Decoder::new();
        let mut reassembler = Reassembler::new();
        for f in &frames {
            decoder.feed(f);
        }
        while let Some(result) = decoder.poll() {
            let frame = result.unwrap();
            reassembler.push(&frame.payload, frame.more());
        }
        assert_eq!(reassembler.finish().unwrap(), blob);
    }

    #[test]
    fn chunk_len_is_clamped_to_max_payload_len() {
        let blob = vec![7u8; MAX_PAYLOAD_LEN * 2 + 10];
        let frames = encode_chunks(MessageType::SyncChunk, &blob, usize::MAX).unwrap();
        // Clamped to MAX_PAYLOAD_LEN per chunk, so at least 3 frames are
        // needed to carry MAX_PAYLOAD_LEN*2+10 bytes.
        assert!(frames.len() >= 3);
    }

    #[test]
    fn reassembler_not_done_until_terminal_chunk() {
        let mut r = Reassembler::new();
        r.push(b"a", true);
        assert!(!r.is_done());
        assert!(r.finish().is_none());
    }
}
