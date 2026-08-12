//! Throwaway manual-verification helper (bead `ai-bitwarden-hw-key-0v8.5`):
//! CBOR-encodes a small fixed `SyncRequest` (mirroring the approved
//! mockup's sample data) and writes it to `seed_credentials.cbor`, so it
//! can be `POST`ed to a running `desktop` emulator's `/api/sync` endpoint
//! with `curl` for a windowed-mode visual check, without needing the real
//! `bw` CLI the `companion` binary depends on.
//!
//! Run with:
//!   `cargo run -p emulator --example seed_credentials --target <host-triple>`
//!   `curl -X POST --data-binary @seed_credentials.cbor -H "Content-Type: application/cbor" http://127.0.0.1:8080/api/sync`

use push_protocol::{Credential, SyncRequest};
use uuid::Uuid;

fn credential(name: &str, username: &str) -> Credential {
    Credential {
        id: Uuid::new_v4(),
        name: name.to_string(),
        username: username.to_string(),
        password: "hunter2".to_string(),
        uri: None,
        notes: None,
    }
}

fn main() {
    let request = SyncRequest {
        credentials: vec![
            credential("GitHub", "octocat@example.com"),
            credential("Amazon Web Services", "andreas@bitwarden.com"),
            credential("Postgres (prod)", "admin"),
            credential("Cloudflare", "acoroiu"),
            credential("Figma", "acoroiu@bitwarden.com"),
        ],
    };

    let mut bytes = Vec::new();
    ciborium::into_writer(&request, &mut bytes).expect("CBOR-encode the seed SyncRequest");

    std::fs::write("seed_credentials.cbor", &bytes).expect("write seed_credentials.cbor");
    println!("wrote seed_credentials.cbor ({} bytes, {} credentials)", bytes.len(), request.credentials.len());
}
