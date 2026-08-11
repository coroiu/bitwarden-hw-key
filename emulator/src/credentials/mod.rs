use bhk_core::VaultItem;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Wire/storage format for the HTTP+CBOR push protocol (dev-aid + fallback
/// per the sync-source ADR) and the on-disk JSON credential store.
/// Deliberately a distinct type from `bhk_core::VaultItem`: this is what
/// crosses the network/disk boundary, `VaultItem` is what the render layer
/// consumes. `From<Credential> for VaultItem` below is the conversion
/// boundary between the two.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: Uuid,
    pub name: String,           // "GitHub"
    pub username: String,       // "user@example.com"
    pub password: String,       // Plaintext for now
    pub uri: Option<String>,    // "https://github.com"
    pub notes: Option<String>,
}

impl From<&Credential> for VaultItem {
    fn from(credential: &Credential) -> Self {
        VaultItem {
            id: credential.id,
            name: credential.name.clone(),
            username: credential.username.clone(),
            password: credential.password.clone(),
            uri: credential.uri.clone(),
            notes: credential.notes.clone(),
        }
    }
}

impl From<Credential> for VaultItem {
    fn from(credential: Credential) -> Self {
        VaultItem::from(&credential)
    }
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
