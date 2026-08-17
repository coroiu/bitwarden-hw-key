//! `device-link`: fixed binary frame header + CRC32 framing, message-type
//! multiplex, and chunking for the USB-CDC (USB Serial/JTAG) link between
//! the T-Embed firmware and the host companion app (M1.5 Phase 2,
//! `ai-bitwarden-hw-key-2ox`).
//!
//! This crate does not talk to any transport (USB, serial, TCP) itself --
//! it only encodes/decodes bytes in memory. WS2-WS5 wire it into the
//! firmware's USB-CDC driver and the host's transport implementation; see
//! `Cargo.toml` for the portability rationale of its (deliberately small)
//! dependency set.
//!
//! # Layers
//!
//! - [`frame`][]: the fixed binary header (`magic|type|flags|len`) +
//!   trailing CRC32, and [`frame::encode_frame`] for building one frame's
//!   bytes.
//! - [`message`][]: [`message::MessageType`] (one enum for both
//!   directions) and the CBOR (de)serialization helpers/payload structs
//!   for each structured message.
//! - [`decoder`][]: [`decoder::Decoder`], the streaming byte-accumulator
//!   that resyncs on the magic word, validates CRC32, bounds the declared
//!   length, and yields frames one at a time as enough bytes arrive.
//! - [`chunk`][]: splits an oversized blob into a `MORE`-linked sequence of
//!   same-type frames ([`chunk::encode_chunks`]) and reassembles one back
//!   ([`chunk::Reassembler`]).
//!
//! # Example: full sync round trip
//!
//! ```
//! use device_link::{chunk, decoder::Decoder, frame::encode_frame, message, MessageType};
//! use push_protocol::{Credential, SyncRequest};
//! use uuid::Uuid;
//!
//! let request = SyncRequest {
//!     credentials: vec![Credential {
//!         id: Uuid::new_v4(),
//!         name: "GitHub".into(),
//!         username: "user@example.com".into(),
//!         password: "hunter2".into(),
//!         uri: Some("https://github.com".into()),
//!         notes: None,
//!     }],
//! };
//! let blob = message::to_cbor(&request).unwrap();
//!
//! // Host side: SyncBegin, then chunked SyncChunk frames, then SyncEnd.
//! let begin = message::SyncBegin { total_bytes: blob.len() as u32, item_count: 1 };
//! let mut wire = encode_frame(MessageType::SyncBegin, 0, &message::to_cbor(&begin).unwrap()).unwrap();
//! for chunk_frame in chunk::encode_chunks(MessageType::SyncChunk, &blob, 32).unwrap() {
//!     wire.extend_from_slice(&chunk_frame);
//! }
//! let end = message::SyncEnd { crc32_of_whole_blob: 0 /* computed for real below */ };
//! let end_bytes = encode_frame(MessageType::SyncEnd, 0, &message::to_cbor(&end).unwrap()).unwrap();
//! wire.extend_from_slice(&end_bytes);
//!
//! // Device side: decode + reassemble.
//! let mut decoder = Decoder::new();
//! decoder.feed(&wire);
//! let mut reassembler = chunk::Reassembler::new();
//! while let Some(Ok(frame)) = decoder.poll() {
//!     if frame.msg_type == MessageType::SyncChunk {
//!         reassembler.push(&frame.payload, frame.more());
//!     }
//! }
//! let reassembled_blob = reassembler.finish().unwrap();
//! let reassembled: SyncRequest = message::from_cbor(&reassembled_blob).unwrap();
//! assert_eq!(reassembled.credentials.len(), 1);
//! ```

pub mod chunk;
pub mod decoder;
pub mod frame;
pub mod message;

pub use chunk::{encode_chunks, Reassembler};
pub use decoder::{DecodeError, Decoder};
pub use frame::{encode_frame, EncodeError, Frame, FLAG_MORE, HEADER_LEN, MAGIC, MAX_PAYLOAD_LEN};
pub use message::{
    from_cbor, to_cbor, CborError, DeviceDescriptor, FramebufferHeader, FramebufferHeaderError,
    MessageType, PixelFormat, SyncBegin, SyncEnd, SyncNack, UnknownMessageType, WireIntent,
    FRAMEBUFFER_HEADER_LEN,
};

// Re-exported so callers building SyncAck payloads don't need a separate
// direct dependency purely to name these types. Deliberately NOT
// re-exporting anything from `bhk-core` -- see `message::WireIntent`'s doc
// comment and `Cargo.toml` for why this crate has no `bhk-core` dependency
// at all.
pub use push_protocol::{Credential, SyncRequest, SyncResponse};
