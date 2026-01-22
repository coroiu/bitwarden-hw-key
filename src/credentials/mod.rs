use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
