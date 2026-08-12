//! `/api/vault/*` route handlers: trigger a vault sync and read back a
//! METADATA-ONLY credential list.
//!
//! See `crate::vault` for the SDK sync/decrypt edge and the
//! `CipherView` -> `Credential` mapping these handlers sit on top of, and
//! `crate::state::VaultCredentialStore` for where the full (password-
//! carrying) `Credential` set is retained server-side.
//!
//! ## Session precondition
//!
//! All three routes below require `Session::Unlocked` and return `409
//! Conflict` otherwise (`crate::auth_routes` already uses `409` for
//! "session not in the required state" -- e.g. `lock` when already logged
//! out -- so this reuses that convention rather than introducing a second
//! one). `401` is reserved for the bearer-token boundary
//! (`crate::auth::require_bearer_token`), which every `/api/*` route
//! (including these) already sits behind.
//!
//! ## Security invariant
//!
//! `GET /api/vault/list` returns `VaultListItem`, a DTO with NO `password`
//! field -- not blanked out, not omitted-if-empty, structurally absent from
//! the type. See the `list_dto_cannot_carry_a_password` test below for a
//! runtime check that backs up what the type system already guarantees at
//! compile time (there is no `credential.password` reachable through
//! `VaultListItem::from`).

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use push_protocol::Credential;
use serde::Serialize;
use uuid::Uuid;

use crate::state::{AppState, Session};
use crate::vault::{sync_and_decrypt, VaultSyncError};

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

fn error_response(status: StatusCode, message: &'static str) -> Response {
    (status, Json(ErrorBody { error: message })).into_response()
}

/// Server-side diagnostic only -- never forwarded to the client. Mirrors
/// `auth_routes::log_login_error`'s rationale: `VaultSyncError` wraps SDK
/// errors that may embed identity/API-server response text, so the full
/// error stays out of any HTTP response body.
fn log_vault_sync_error(err: &VaultSyncError) {
    eprintln!("web-companion: vault sync failed: {err}");
}

/// `GET /api/vault/list` response DTO -- METADATA ONLY. This struct's field
/// list is the ONLY place stopping a password from crossing to the
/// browser: there is deliberately no `password` field here, not even as
/// `Option<String>` set to `None`. Do not add one; if a future bead needs
/// the device to know a password, that must go through the existing
/// device-push path (`push_protocol::Credential`, server-side sync
/// payload), never through this browser-facing list.
#[derive(Serialize)]
pub struct VaultListItem {
    pub id: Uuid,
    pub name: String,
    pub username: String,
    pub uri: Option<String>,
}

impl From<&Credential> for VaultListItem {
    fn from(credential: &Credential) -> Self {
        Self {
            id: credential.id,
            name: credential.name.clone(),
            username: credential.username.clone(),
            uri: credential.uri.clone(),
        }
    }
}

/// `GET /api/vault/status` response shape.
#[derive(Serialize)]
pub struct VaultStatus {
    /// Whether a sync has produced a retained credential set this process
    /// lifetime. Does NOT distinguish "never synced" from "synced zero
    /// credentials" -- see `count` for that.
    pub synced: bool,
    pub count: usize,
}

/// `POST /api/vault/sync` response shape.
#[derive(Serialize)]
struct VaultSyncResult {
    synced: bool,
    count: usize,
}

/// Returns the cloned `Client` out of an `Unlocked` session, or `None` if
/// the session isn't unlocked. Cloning is intentionally short: the caller
/// drops the session lock immediately afterwards rather than holding it for
/// the full sync network round trip (unlike `auth_routes::login`'s
/// deliberate whole-call lock -- vault sync doesn't mutate `Session` itself,
/// only `AppState::vault_credentials`, and a slow sync shouldn't block an
/// unrelated `/api/auth/status` poll).
///
/// Known accepted race (loopback-only, single-user threat model, same
/// posture as the gaps documented in `auth_routes`' module docs): a
/// `/api/auth/logout` racing a concurrent `/api/vault/sync` can result in
/// vault data landing in `vault_credentials` after the user believes they
/// logged out. `Client` staying alive via the clone is exactly what the
/// eml.1 `Send + Sync + Clone` (wraps `Arc<InternalClient>`) confirmation
/// implies; a stricter fix would need a cancellation token, which is out of
/// scope for this bead.
async fn unlocked_client(state: &AppState) -> Option<bitwarden_core::Client> {
    let session = state.session.lock().await;
    match &*session {
        Session::Unlocked(client) => Some(client.clone()),
        _ => None,
    }
}

async fn require_unlocked(state: &AppState) -> Result<(), Response> {
    let session = state.session.lock().await;
    match &*session {
        Session::Unlocked(_) => Ok(()),
        _ => Err(error_response(StatusCode::CONFLICT, "vault is not unlocked")),
    }
}

/// `POST /api/vault/sync` -- syncs + decrypts the vault and replaces the
/// server-side retained credential set. Requires `Session::Unlocked`.
pub async fn sync(State(state): State<AppState>) -> Response {
    let Some(client) = unlocked_client(&state).await else {
        return error_response(StatusCode::CONFLICT, "vault is not unlocked");
    };

    match sync_and_decrypt(&client).await {
        Ok(credentials) => {
            let count = credentials.len();
            state.vault_credentials.replace(credentials).await;
            Json(VaultSyncResult {
                synced: true,
                count,
            })
            .into_response()
        }
        Err(err) => {
            log_vault_sync_error(&err);
            error_response(StatusCode::BAD_GATEWAY, "vault sync failed")
        }
    }
}

/// `GET /api/vault/list` -- METADATA ONLY (see module docs). Reads whatever
/// the most recent `POST /api/vault/sync` retained; does not itself trigger
/// a sync. Requires `Session::Unlocked`.
pub async fn list(State(state): State<AppState>) -> Response {
    if let Err(response) = require_unlocked(&state).await {
        return response;
    }

    let credentials = state.vault_credentials.get_all().await;
    let items: Vec<VaultListItem> = credentials.iter().map(VaultListItem::from).collect();
    Json(items).into_response()
}

/// `GET /api/vault/status` -- reflects whether a sync has populated the
/// retained credential set. Requires `Session::Unlocked` (same as `list`;
/// there is nothing meaningful to report about a vault whose session is
/// gone -- `vault_credentials` is also cleared on lock/logout, see
/// `crate::auth_routes`).
pub async fn status(State(state): State<AppState>) -> Response {
    if let Err(response) = require_unlocked(&state).await {
        return response;
    }

    let count = state.vault_credentials.get_all().await.len();
    Json(VaultStatus {
        synced: count > 0,
        count,
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn sample_credential() -> Credential {
        Credential {
            id: Uuid::new_v4(),
            name: "GitHub".to_string(),
            username: "octocat".to_string(),
            password: "hunter2".to_string(),
            uri: Some("https://github.com".to_string()),
            notes: Some("some notes".to_string()),
        }
    }

    #[test]
    fn vault_list_item_carries_metadata() {
        let credential = sample_credential();
        let item = VaultListItem::from(&credential);

        assert_eq!(item.id, credential.id);
        assert_eq!(item.name, "GitHub");
        assert_eq!(item.username, "octocat");
        assert_eq!(item.uri, Some("https://github.com".to_string()));
    }

    /// Runtime proof to back up the compile-time guarantee: serializing a
    /// `VaultListItem` built from a `Credential` that DOES carry a password
    /// produces JSON with no `password` key anywhere. If a future edit ever
    /// added a `password` field to `VaultListItem`, this test would start
    /// failing (and so would the `From` conversion above, which has no
    /// `credential.password` reference to copy from).
    #[test]
    fn list_dto_cannot_carry_a_password() {
        let credential = sample_credential();
        let item = VaultListItem::from(&credential);

        let json = serde_json::to_value(&item).unwrap();
        assert!(
            json.get("password").is_none(),
            "VaultListItem JSON must never contain a password field"
        );
        let json_string = serde_json::to_string(&item).unwrap();
        assert!(!json_string.contains("hunter2"));
        assert!(!json_string.to_lowercase().contains("password"));
    }

    #[test]
    fn vault_status_serializes_count_and_synced_flag() {
        let status = VaultStatus {
            synced: true,
            count: 3,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["synced"], true);
        assert_eq!(json["count"], 3);
    }
}
