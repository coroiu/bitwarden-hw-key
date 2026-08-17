//! End-to-end test of the full Host -> Device sync flow: `SyncBegin`,
//! N `SyncChunk` frames (forced small so the blob actually splits), then
//! `SyncEnd` carrying a whole-blob CRC32 -- decoded back into the original
//! `push_protocol::SyncRequest`. This is the scenario WS3's
//! `SerialSyncReceiver` state machine will implement against, so it's
//! covered as its own integration test (in addition to the smaller
//! per-module unit tests) rather than folded into `lib.rs`'s doctest.

// Test fixture sizes are small and known in-range; casting them to u32 for
// the wire-format fields is never a real truncation risk here.
#![allow(clippy::cast_possible_truncation)]

use crc::{Crc, CRC_32_ISO_HDLC};
use device_link::{
    chunk::{encode_chunks, Reassembler},
    decoder::Decoder,
    frame::encode_frame,
    message::{from_cbor, to_cbor, SyncBegin, SyncEnd},
    Credential, MessageType, SyncRequest,
};
use uuid::Uuid;

fn whole_blob_crc32(blob: &[u8]) -> u32 {
    const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    CRC.checksum(blob)
}

#[test]
fn full_sync_begin_chunk_end_round_trip() {
    let original = SyncRequest {
        credentials: (0..25)
            .map(|i| Credential {
                id: Uuid::new_v4(),
                name: format!("Service {i}"),
                username: format!("user{i}@example.com"),
                password: format!("correct-horse-battery-staple-{i}"),
                uri: Some(format!("https://service{i}.example.com")),
                notes: if i % 3 == 0 { Some("some notes".into()) } else { None },
            })
            .collect(),
    };

    let blob = to_cbor(&original).expect("SyncRequest should CBOR-encode");
    let whole_crc = whole_blob_crc32(&blob);

    // --- Host side: build the wire bytes for the whole sequence. ---
    let mut wire = Vec::new();

    let begin = SyncBegin { total_bytes: blob.len() as u32, item_count: original.credentials.len() as u32 };
    wire.extend_from_slice(&encode_frame(MessageType::SyncBegin, 0, &to_cbor(&begin).unwrap()).unwrap());

    // Force a small chunk size so the 25-credential blob actually splits
    // into multiple SyncChunk frames, not just one.
    let chunk_frames = encode_chunks(MessageType::SyncChunk, &blob, 96).unwrap();
    assert!(chunk_frames.len() > 3, "test blob should need several chunks at this chunk size");
    for f in &chunk_frames {
        wire.extend_from_slice(f);
    }

    let end = SyncEnd { crc32_of_whole_blob: whole_crc };
    wire.extend_from_slice(&encode_frame(MessageType::SyncEnd, 0, &to_cbor(&end).unwrap()).unwrap());

    // --- Device side: decode the byte stream and reassemble. ---
    let mut decoder = Decoder::new();
    decoder.feed(&wire);

    let mut seen_begin: Option<SyncBegin> = None;
    let mut reassembler = Reassembler::new();
    let mut seen_end: Option<SyncEnd> = None;

    while let Some(result) = decoder.poll() {
        let frame = result.expect("no decode errors expected on a clean, well-formed stream");
        match frame.msg_type {
            MessageType::SyncBegin => {
                seen_begin = Some(from_cbor(&frame.payload).unwrap());
            }
            MessageType::SyncChunk => {
                reassembler.push(&frame.payload, frame.more());
            }
            MessageType::SyncEnd => {
                seen_end = Some(from_cbor(&frame.payload).unwrap());
            }
            other => panic!("unexpected message type in sync flow: {other:?}"),
        }
    }

    let seen_begin = seen_begin.expect("SyncBegin frame should have been decoded");
    assert_eq!(seen_begin.total_bytes as usize, blob.len());
    assert_eq!(seen_begin.item_count as usize, original.credentials.len());

    assert!(reassembler.is_done(), "reassembler should see the terminal (non-MORE) chunk");
    let reassembled_blob = reassembler.finish().unwrap();
    assert_eq!(reassembled_blob, blob, "reassembled blob must match the original byte-for-byte");

    let seen_end = seen_end.expect("SyncEnd frame should have been decoded");
    assert_eq!(seen_end.crc32_of_whole_blob, whole_blob_crc32(&reassembled_blob));

    let reassembled_request: SyncRequest = from_cbor(&reassembled_blob).expect("blob should CBOR-decode back");
    assert_eq!(reassembled_request.credentials.len(), original.credentials.len());
    for (a, b) in original.credentials.iter().zip(reassembled_request.credentials.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.name, b.name);
        assert_eq!(a.username, b.username);
        assert_eq!(a.password, b.password);
        assert_eq!(a.uri, b.uri);
        assert_eq!(a.notes, b.notes);
    }
}

#[test]
fn sync_flow_survives_being_fed_one_byte_at_a_time() {
    // Same flow as above, but fed through the decoder one byte at a time,
    // to prove the chunked-sync path (not just single frames) tolerates
    // arbitrary read granularity from the underlying transport.
    let original = SyncRequest {
        credentials: vec![Credential {
            id: Uuid::new_v4(),
            name: "Only One".into(),
            username: "solo@example.com".into(),
            password: "hunter2".into(),
            uri: None,
            notes: None,
        }],
    };
    let blob = to_cbor(&original).unwrap();
    let whole_crc = whole_blob_crc32(&blob);

    let mut wire = Vec::new();
    let begin = SyncBegin { total_bytes: blob.len() as u32, item_count: 1 };
    wire.extend_from_slice(&encode_frame(MessageType::SyncBegin, 0, &to_cbor(&begin).unwrap()).unwrap());
    for f in encode_chunks(MessageType::SyncChunk, &blob, 8).unwrap() {
        wire.extend_from_slice(&f);
    }
    let end = SyncEnd { crc32_of_whole_blob: whole_crc };
    wire.extend_from_slice(&encode_frame(MessageType::SyncEnd, 0, &to_cbor(&end).unwrap()).unwrap());

    let mut decoder = Decoder::new();
    let mut reassembler = Reassembler::new();
    let mut end_crc = None;

    for byte in &wire {
        decoder.feed(std::slice::from_ref(byte));
        while let Some(result) = decoder.poll() {
            let frame = result.unwrap();
            match frame.msg_type {
                MessageType::SyncChunk => reassembler.push(&frame.payload, frame.more()),
                MessageType::SyncEnd => {
                    end_crc = Some(from_cbor::<SyncEnd>(&frame.payload).unwrap().crc32_of_whole_blob);
                }
                _ => {}
            }
        }
    }

    let reassembled_blob = reassembler.finish().unwrap();
    assert_eq!(reassembled_blob, blob);
    assert_eq!(end_crc.unwrap(), whole_blob_crc32(&reassembled_blob));
}
