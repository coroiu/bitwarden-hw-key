//! Wire-format types for the companion-app -> device HTTP+CBOR push
//! protocol (`POST /api/sync`; see
//! `.planning/decisions/2026-08-11-sync-direction-companion-push.md`).
//!
//! This crate is deliberately pure: serde + ciborium + uuid only, no
//! `bhk-core`, no `firmware`, no `emulator`. Both the `emulator` crate (the
//! device side today) and the future companion app depend on it so the
//! contract is defined exactly once and can't drift between the two ends of
//! the wire. Anything that needs to convert these types into
//! `bhk_core::VaultItem` — the render-layer view model — lives in
//! `emulator` instead, since that's the only crate that sees both
//! `push-protocol` and `bhk-core`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Wire/storage format for the HTTP+CBOR push protocol (dev-aid + fallback
/// per the sync-source ADR) and the on-disk JSON credential store.
/// Deliberately a distinct type from `bhk_core::VaultItem`: this is what
/// crosses the network/disk boundary, `VaultItem` is what the render layer
/// consumes. The `From<Credential> for VaultItem` conversion lives in
/// `emulator::credentials`, the boundary between the two.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: Uuid,
    pub name: String,           // "GitHub"
    pub username: String,       // "user@example.com"
    pub password: String,       // Plaintext for now
    pub uri: Option<String>,    // "https://github.com"
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncRequest {
    pub credentials: Vec<Credential>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncResponse {
    pub status: String,
    pub synced: usize,
    pub total_bytes: usize,
}
