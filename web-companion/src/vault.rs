//! Vault read: sync + decrypt ciphers, mapped to `push_protocol::Credential`.
//!
//! This is deliberately a thin SDK edge (`sync_and_decrypt`) plus a pure,
//! heavily-unit-tested mapping function (`cipher_view_to_credential`). See
//! `crate::vault_routes` for the HTTP surface built on top of this, and
//! `crate::state::VaultCredentialStore` for where the resulting
//! `Credential`s (WITH plaintext passwords) are held server-side.
//!
//! ## SDK signature confirmations (pinned rev 99ffb6ef, sdk-internal)
//!
//! - `Client::sync()` (via `bitwarden_sync::SyncClientExt`, already pinned
//!   as a dependency by eml.1) returns a `SyncClient`. Per the eml.1 delta,
//!   `SyncClient::sync(SyncRequest) -> Result<bool, SyncError>` -- the
//!   `bool` says whether a full sync ran (`true`) or was skipped because the
//!   server's revision date wasn't newer than the last recorded sync
//!   (`false`). It is NOT the sync payload. The payload only reaches this
//!   process via a registered `SyncHandler::on_sync(&SyncResponseModel)`
//!   callback -- there is no "fetch last sync response" getter on
//!   `SyncClient`.
//! - We use `SyncRequest { force: true, .. }` specifically because this
//!   server never persists a `last_sync` timestamp (each login builds a
//!   fresh in-memory `Client` -- see `crate::state::build_client`), so a
//!   revision-date skip would (at best) be a no-op and (at worst) mean the
//!   very first sync of a session returns `Ok(false)` without ever calling
//!   our handler if some other code path had already recorded a timestamp.
//!   `force: true` guarantees the handler always runs when the browser
//!   explicitly asks us to sync.
//! - `bitwarden_vault::CiphersClient::{get_all, list}` read from a
//!   `Repository<Cipher>` (state-backed storage), not from the sync
//!   response directly -- and unlike `bitwarden-vault`'s `FoldersClient`
//!   (which registers its own `SyncHandler` to populate its repository),
//!   there is no built-in cipher `SyncHandler` in this SDK revision that
//!   writes synced ciphers into a `Repository<Cipher>` for us. Wiring one
//!   up would mean adopting `bitwarden_state::repository::Repository`
//!   end-to-end for no benefit here (we don't want on-disk cipher
//!   persistence -- see the security invariant in `crate::state`). So this
//!   module takes the documented alternative: register our OWN
//!   `SyncHandler` that clones `SyncResponseModel.ciphers` directly off the
//!   wire response, then decrypts them ourselves via the `Client`'s own key
//!   store (`client.internal.get_key_store()`, `pub` per
//!   `bitwarden_core::client::client::Client` -- confirmed by reading
//!   `crates/bitwarden-core/src/client/client.rs` at the pinned rev).
//! - Decryption: `Cipher: Decryptable<KeySlotIds, SymmetricKeySlotId,
//!   CipherView>` (confirmed in `bitwarden-vault/src/cipher/cipher.rs`), so
//!   `KeyStore<KeySlotIds>::decrypt::<SymmetricKeySlotId, Cipher,
//!   CipherView>` -- written below with the output type inferred from the
//!   function's return type, matching the pattern `CiphersClient::get`
//!   itself uses internally.

use std::sync::Arc;

use bitwarden_api_api::models::CipherDetailsResponseModel;
use bitwarden_api_api::models::SyncResponseModel;
use bitwarden_core::Client;
use bitwarden_core::key_management::KeySlotIds;
use bitwarden_sync::{SyncClientExt, SyncError, SyncHandler, SyncHandlerError, SyncRequest};
use bitwarden_vault::{Cipher, CipherType, CipherView};
use push_protocol::Credential;
use tokio::sync::Mutex;

/// Registered on the `SyncClient` for the duration of one `sync_and_decrypt`
/// call to capture the raw (still-encrypted) cipher list off the sync
/// response. See module docs for why this is necessary at this SDK rev.
struct CipherCaptureHandler {
    captured: Arc<Mutex<Vec<CipherDetailsResponseModel>>>,
}

#[async_trait::async_trait]
impl SyncHandler for CipherCaptureHandler {
    async fn on_sync(&self, response: &SyncResponseModel) -> Result<(), SyncHandlerError> {
        let mut guard = self.captured.lock().await;
        *guard = response.ciphers.clone().unwrap_or_default();
        Ok(())
    }
}

/// Errors from the SDK edge. Deliberately opaque to callers (see
/// `crate::vault_routes`) -- never forwarded verbatim over HTTP, only
/// logged server-side, matching `auth_routes::log_login_error`'s rationale.
#[derive(Debug)]
pub enum VaultSyncError {
    Sync(SyncError),
}

impl std::fmt::Display for VaultSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultSyncError::Sync(err) => write!(f, "vault sync failed: {err}"),
        }
    }
}

/// Syncs the account's ciphers, decrypts every login item, and maps them to
/// the typed `push_protocol::Credential` shape (the replacement for the
/// retired bw-CLI JSON mapper). Non-login ciphers, and ciphers that fail to
/// parse or decrypt, are silently skipped -- matching the SDK's own
/// `CiphersClient::get_all`/`list` posture of "return what decrypted
/// successfully, don't fail the whole batch over one bad item."
///
/// Requires an already-`Unlocked` `Client` (caller's responsibility --
/// `crate::vault_routes` enforces this via `Session`).
pub async fn sync_and_decrypt(client: &Client) -> Result<Vec<Credential>, VaultSyncError> {
    let captured: Arc<Mutex<Vec<CipherDetailsResponseModel>>> = Arc::new(Mutex::new(Vec::new()));

    let sync_client = client.sync();
    sync_client.register_sync_handler(Arc::new(CipherCaptureHandler {
        captured: captured.clone(),
    }));

    sync_client
        .sync(SyncRequest {
            force: true,
            exclude_subdomains: None,
        })
        .await
        .map_err(VaultSyncError::Sync)?;

    let raw_ciphers = captured.lock().await.clone();
    let key_store = client.internal.get_key_store();

    let credentials = raw_ciphers
        .into_iter()
        .filter_map(|raw| match Cipher::try_from(raw) {
            Ok(cipher) => Some(cipher),
            Err(err) => {
                eprintln!("web-companion: skipping cipher that failed to parse: {err}");
                None
            }
        })
        .filter_map(|cipher| match decrypt_cipher(key_store, &cipher) {
            Ok(view) => Some(view),
            Err(err) => {
                eprintln!("web-companion: skipping cipher that failed to decrypt: {err}");
                None
            }
        })
        .filter_map(cipher_view_to_credential)
        .collect();

    Ok(credentials)
}

/// Named (rather than inlined in a closure) so the `Output = CipherView`
/// type parameter on `KeyStore::decrypt` can be pinned via the return type,
/// the same trick `bitwarden_vault::cipher_client::get::get_cipher` uses
/// internally.
fn decrypt_cipher(
    key_store: &bitwarden_crypto::KeyStore<KeySlotIds>,
    cipher: &Cipher,
) -> Result<CipherView, bitwarden_crypto::CryptoError> {
    key_store.decrypt(cipher)
}

/// Maps a decrypted `CipherView` to the typed `push_protocol::Credential`
/// wire shape. This is the typed replacement for the retired bw-CLI JSON
/// mapper -- same field rules:
///
/// - Non-login cipher types are dropped (`None`).
/// - A cipher with no `id` is dropped (`None`) -- a `Credential` without an
///   id can't be addressed for a future re-sync/diff, and the server always
///   assigns one for real ciphers, so this only guards a defensive/testing
///   edge case.
/// - `login.username` / `login.password`: `None` -> `""` (never `None` on
///   the wire type).
/// - `login.uris`: only `uris[0].uri` is kept; `uris[1..]` are dropped;
///   `None` if there is no login, no uris, or the first uri's `uri` field
///   is itself `None`.
/// - `notes` passes through as-is (`Option<String>`).
/// - `totp`, `reprompt`, `folder_id`, `collection_ids`, `favorite` are
///   consciously NOT mapped -- `Credential` has no fields for them (M1
///   omission, matches the bead brief).
fn cipher_view_to_credential(view: CipherView) -> Option<Credential> {
    if view.r#type != CipherType::Login {
        return None;
    }
    let id = view.id?.into();

    let username = view
        .login
        .as_ref()
        .and_then(|login| login.username.clone())
        .unwrap_or_default();
    let password = view
        .login
        .as_ref()
        .and_then(|login| login.password.clone())
        .unwrap_or_default();
    let uri = view
        .login
        .as_ref()
        .and_then(|login| login.uris.as_ref())
        .and_then(|uris| uris.first())
        .and_then(|first| first.uri.clone());

    Some(Credential {
        id,
        name: view.name,
        username,
        password,
        uri,
        notes: view.notes,
    })
}

#[cfg(test)]
mod tests {
    use bitwarden_vault::{CipherId, CipherRepromptType, LoginUriView, LoginView, UriMatchType};
    use chrono::Utc;
    use uuid::Uuid;

    use super::*;

    /// Full `CipherView` with every field set to an inert default. Tests
    /// override only the fields they care about -- `CipherView` has ~25
    /// fields (attachments, fields, permissions, etc.) that are irrelevant
    /// to the mapping under test (see `cipher_view_to_credential` doc
    /// comment for the fields that ARE consciously dropped).
    fn base_cipher_view() -> CipherView {
        let now = Utc::now();
        CipherView {
            id: Some(CipherId::new(Uuid::new_v4())),
            organization_id: None,
            folder_id: None,
            collection_ids: Vec::new(),
            key: None,
            name: "Example".to_string(),
            notes: None,
            r#type: CipherType::Login,
            login: None,
            identity: None,
            card: None,
            secure_note: None,
            ssh_key: None,
            bank_account: None,
            drivers_license: None,
            passport: None,
            favorite: false,
            reprompt: CipherRepromptType::default(),
            organization_use_totp: false,
            edit: true,
            permissions: None,
            view_password: true,
            local_data: None,
            attachments: None,
            attachment_decryption_failures: None,
            fields: None,
            password_history: None,
            creation_date: now,
            deleted_date: None,
            revision_date: now,
            archived_date: None,
        }
    }

    fn login_view(username: Option<&str>, password: Option<&str>, uris: Vec<&str>) -> LoginView {
        LoginView {
            username: username.map(str::to_string),
            password: password.map(str::to_string),
            password_revision_date: None,
            uris: if uris.is_empty() {
                None
            } else {
                Some(
                    uris.into_iter()
                        .map(|uri| LoginUriView {
                            uri: Some(uri.to_string()),
                            r#match: Some(UriMatchType::Domain),
                            uri_checksum: None,
                        })
                        .collect(),
                )
            },
            totp: None,
            autofill_on_page_load: None,
            fido2_credentials: None,
        }
    }

    #[test]
    fn maps_a_full_login_item() {
        let mut view = base_cipher_view();
        view.name = "GitHub".to_string();
        view.notes = Some("some notes".to_string());
        view.login = Some(login_view(
            Some("octocat"),
            Some("hunter2"),
            vec!["https://github.com", "https://github.com/login"],
        ));

        let expected_id: Uuid = view.id.unwrap().into();
        let credential = cipher_view_to_credential(view).expect("login item should map");

        assert_eq!(credential.id, expected_id);
        assert_eq!(credential.name, "GitHub");
        assert_eq!(credential.username, "octocat");
        assert_eq!(credential.password, "hunter2");
        assert_eq!(credential.notes, Some("some notes".to_string()));
    }

    #[test]
    fn multi_uri_login_collapses_to_the_first_uri() {
        let mut view = base_cipher_view();
        view.login = Some(login_view(
            Some("user"),
            Some("pw"),
            vec!["https://first.example", "https://second.example"],
        ));

        let credential = cipher_view_to_credential(view).expect("login item should map");

        assert_eq!(credential.uri, Some("https://first.example".to_string()));
    }

    #[test]
    fn null_username_and_notes_become_safe_defaults() {
        let mut view = base_cipher_view();
        view.notes = None;
        view.login = Some(login_view(None, None, vec![]));

        let credential = cipher_view_to_credential(view).expect("login item should map");

        assert_eq!(credential.username, "");
        assert_eq!(credential.password, "");
        assert_eq!(credential.uri, None);
        assert_eq!(credential.notes, None);
    }

    #[test]
    fn login_with_no_login_field_gets_safe_defaults() {
        // Defensive case: type == Login but `login` itself is None.
        let mut view = base_cipher_view();
        view.login = None;

        let credential = cipher_view_to_credential(view).expect("login item should map");

        assert_eq!(credential.username, "");
        assert_eq!(credential.password, "");
        assert_eq!(credential.uri, None);
    }

    #[test]
    fn non_login_item_is_filtered_out() {
        let mut view = base_cipher_view();
        view.r#type = CipherType::SecureNote;
        view.login = None;

        assert!(cipher_view_to_credential(view).is_none());
    }

    #[test]
    fn cipher_with_no_id_is_filtered_out() {
        let mut view = base_cipher_view();
        view.id = None;

        assert!(cipher_view_to_credential(view).is_none());
    }

    #[test]
    fn first_uri_with_no_uri_value_is_treated_as_absent() {
        let mut view = base_cipher_view();
        view.login = Some(LoginView {
            uris: Some(vec![LoginUriView {
                uri: None,
                r#match: None,
                uri_checksum: None,
            }]),
            ..login_view(Some("user"), Some("pw"), vec![])
        });

        let credential = cipher_view_to_credential(view).expect("login item should map");

        assert_eq!(credential.uri, None);
    }
}
