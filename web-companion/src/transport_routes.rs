//! `/api/devices` and `/api/sync` route handlers: enumerating device
//! transport targets and pushing (optionally filtered) credentials to one
//! of them. See `crate::transport` for the `DeviceTransport`/
//! `TransportProvider` abstraction and the Phase-1 `HttpEmulatorTransport`
//! this sits on top of.
//!
//! ## Session precondition
//!
//! Both routes require `Session::Unlocked`, same convention as
//! `crate::vault_routes` (`409 CONFLICT` otherwise; `401` is reserved for
//! the bearer-token boundary already wrapping all of `/api/*`, see
//! `crate::auth::require_bearer_token`). Pushing an EMPTY credential list
//! because nothing was synced yet is an acceptable, non-error outcome --
//! there is no separate "must have synced first" gate.
//!
//! ## Security invariant
//!
//! Every response type in this module is credential-free:
//! `DeviceDescriptor` (`GET /api/devices`) carries no credential data at
//! all, and `SyncPushResult`/the error bodies (`POST /api/sync`) never echo
//! back credentials or a password. The plaintext password only ever leaves
//! this process in the OUTBOUND CBOR body built by
//! `crate::transport::HttpEmulatorTransport::push` (server -> device). See
//! `crate::state::VaultCredentialStore` for where the password-carrying set
//! lives server-side, and `log_transport_error` below for why transport
//! errors are never forwarded verbatim over HTTP.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use push_protocol::{Credential, SyncRequest};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::{AppState, Session};
use crate::transport::{DeviceDescriptor, TransportError};

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

fn error_response(status: StatusCode, message: &'static str) -> Response {
    (status, Json(ErrorBody { error: message })).into_response()
}

async fn require_unlocked(state: &AppState) -> Result<(), Response> {
    let session = state.session.lock().await;
    match &*session {
        Session::Unlocked(_) => Ok(()),
        _ => Err(error_response(StatusCode::CONFLICT, "vault is not unlocked")),
    }
}

/// Server-side diagnostic only -- never forwarded to the client. Mirrors
/// `vault_routes::log_vault_sync_error`'s rationale, and never logs a
/// `Credential`/`SyncRequest` (only counts/ids/target ids elsewhere in this
/// module) -- `TransportError` itself is already credential-free by
/// construction (see `crate::transport`).
fn log_transport_error(err: &TransportError) {
    eprintln!("web-companion: device push failed: {err}");
}

/// `GET /api/devices` -- lists every target every registered
/// `TransportProvider` currently sees (Phase 1: just the emulator).
/// Requires `Session::Unlocked`.
pub async fn list_devices(State(state): State<AppState>) -> Response {
    if let Err(response) = require_unlocked(&state).await {
        return response;
    }

    let devices: Vec<DeviceDescriptor> = state.transports.list_all_targets();
    Json(devices).into_response()
}

/// `POST /api/sync` request body.
#[derive(Deserialize)]
pub struct SyncPushRequest {
    pub target_id: String,
    /// If `Some`, push only the credentials whose `id` is in this list. If
    /// `None`, push everything currently retained server-side. See
    /// `filter_credentials` for how unknown ids in this list are handled.
    pub item_ids: Option<Vec<Uuid>>,
}

/// `POST /api/sync` response body on success. `pushed` reflects what the
/// server attempted to send (`filtered.len()`), NOT necessarily the
/// device's own `SyncResponse.synced` count -- those two numbers answering
/// different questions ("how many did we ask to push" vs. "how many did the
/// device report accepting") is deliberate; this bead does not surface the
/// device's own count at all rather than risk conflating the two under one
/// ambiguous field name. `device` is the descriptor of whatever
/// `DeviceTransport` actually received the push (from
/// `DeviceTransport::descriptor`, not merely echoed back from the request's
/// `target_id`) -- credential-free, same as `GET /api/devices`.
#[derive(Serialize)]
struct SyncPushResult {
    pushed: usize,
    device: DeviceDescriptor,
}

/// Filters `all` down to only the credentials whose `id` is present in
/// `item_ids`, preserving `all`'s order. `item_ids` being `None` means "no
/// filter, push everything." Ids in `item_ids` that don't match any
/// credential in `all` are simply absent from the output -- treated as a
/// no-op, not an error (an "optional filter" reasonably tolerates
/// stale/unknown ids without failing the whole push).
fn filter_credentials(all: &[Credential], item_ids: Option<&[Uuid]>) -> Vec<Credential> {
    match item_ids {
        None => all.to_vec(),
        Some(ids) => all.iter().filter(|c| ids.contains(&c.id)).cloned().collect(),
    }
}

/// `POST /api/sync` -- pushes the (optionally filtered) server-side
/// credential set to `target_id` over whatever `DeviceTransport`
/// `state.transports.connect` resolves it to. Requires `Session::Unlocked`.
/// See module docs for the security posture.
pub async fn sync(State(state): State<AppState>, Json(body): Json<SyncPushRequest>) -> Response {
    if let Err(response) = require_unlocked(&state).await {
        return response;
    }

    let all_credentials = state.vault_credentials.get_all().await;
    let filtered = filter_credentials(&all_credentials, body.item_ids.as_deref());
    let pushed = filtered.len();
    let request = SyncRequest {
        credentials: filtered,
    };

    let transport = match state.transports.connect(&body.target_id).await {
        Ok(transport) => transport,
        Err(err) => {
            log_transport_error(&err);
            return error_response(StatusCode::NOT_FOUND, "unknown device");
        }
    };

    let device = transport.descriptor();
    match transport.push(&request).await {
        Ok(_response) => Json(SyncPushResult { pushed, device }).into_response(),
        Err(err) => {
            log_transport_error(&err);
            error_response(StatusCode::BAD_GATEWAY, "device push failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_credential(name: &str) -> Credential {
        Credential {
            id: Uuid::new_v4(),
            name: name.to_string(),
            username: "user".to_string(),
            password: "hunter2".to_string(),
            uri: None,
            notes: None,
        }
    }

    #[test]
    fn no_filter_returns_everything_in_order() {
        let all = vec![sample_credential("a"), sample_credential("b")];
        let filtered = filter_credentials(&all, None);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "a");
        assert_eq!(filtered[1].name, "b");
    }

    #[test]
    fn filter_selects_only_matching_ids_preserving_order() {
        let a = sample_credential("a");
        let b = sample_credential("b");
        let c = sample_credential("c");
        let all = vec![a.clone(), b.clone(), c.clone()];

        let filtered = filter_credentials(&all, Some(&[c.id, a.id]));

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, a.id);
        assert_eq!(filtered[1].id, c.id);
    }

    #[test]
    fn unknown_ids_in_filter_are_silently_absent_not_an_error() {
        let a = sample_credential("a");
        let all = vec![a.clone()];
        let unknown_id = Uuid::new_v4();

        let filtered = filter_credentials(&all, Some(&[unknown_id]));

        assert!(filtered.is_empty());
    }

    #[test]
    fn empty_filter_list_pushes_nothing() {
        let all = vec![sample_credential("a")];
        let filtered = filter_credentials(&all, Some(&[]));

        assert!(filtered.is_empty());
    }

    /// Runtime proof to back up the module's security invariant doc: the
    /// success response type structurally cannot carry a credential or
    /// password field, even though it does carry a (credential-free)
    /// device descriptor.
    #[test]
    fn sync_push_result_cannot_carry_credentials() {
        let result = SyncPushResult {
            pushed: 3,
            device: DeviceDescriptor {
                id: "emulator".to_string(),
                name: "Desktop Emulator".to_string(),
                kind: crate::transport::DeviceKind::Emulator,
            },
        };
        let json = serde_json::to_value(&result).unwrap();

        assert_eq!(json["pushed"], 3);
        assert_eq!(json["device"]["id"], "emulator");
        assert!(json.get("credentials").is_none());
        assert!(json.get("password").is_none());
        let json_string = serde_json::to_string(&result).unwrap();
        assert!(!json_string.to_lowercase().contains("password"));
    }
}
