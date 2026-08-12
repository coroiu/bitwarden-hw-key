//! Shared application state for the web-companion axum server.

use std::sync::Arc;

use bitwarden_core::{Client, ClientSettings, DeviceType};
use push_protocol::Credential;
use tokio::sync::Mutex;
use zeroize::{Zeroize, Zeroizing};

use crate::transport::{DeviceDescriptor, EmulatorTransportProvider, TransportError, TransportProvider};

/// Email + master password stashed server-side across a `POST
/// /api/auth/login` -> `POST /api/auth/2fa` round trip (see
/// `crate::auth_routes` module docs for the full state machine).
///
/// This is the one place a plaintext master password lives outside the SDK
/// `Client`'s own internal state. `master_password` is wrapped in
/// `Zeroizing<String>` so it is overwritten with zeros the moment this
/// value is dropped -- on a successful 2FA retry (replaced by
/// `Session::Unlocked`), a failed retry that resets to `Session::LoggedOut`
/// (not implemented that way today, see module docs), or an explicit
/// `/api/auth/logout`. It is never `Debug`/`Display`-derived, logged, or
/// serialized.
pub struct PendingTwoFactorLogin {
    pub email: String,
    pub master_password: Zeroizing<String>,
}

/// Authentication/session state for the (eventually) single Bitwarden
/// account this companion process manages. See `crate::auth_routes` for the
/// full state-machine documentation and the handlers that drive these
/// transitions.
///
/// `Locked(Client)` is intentionally never constructed by this crate: at
/// the pinned SDK revision (99ffb6ef) there is no re-unlock-without-password
/// path for a fresh in-memory session (the SDK's persisted-state resume
/// path is for rehydrating state this server does not persist -- see
/// eml.1/eml.3 findings). A session that cannot be unlocked again without
/// the password is functionally logged out, so `POST /api/auth/lock`
/// transitions straight to `LoggedOut` rather than inventing an unusable
/// `Locked` state. The variant is kept (rather than deleted) so a future
/// bead that adds real persisted-state resume has a documented slot to fill
/// in; `#[allow(dead_code)]` on it documents that gap rather than hiding it.
pub enum Session {
    LoggedOut,
    PendingTwoFactor(PendingTwoFactorLogin),
    #[allow(dead_code)]
    Locked(Client),
    // The inner `Client` is constructed and stored here by `auth_routes`
    // once unlocked, but nothing reads it back out yet -- vault access
    // (the reason to ever read an unlocked `Client`) is eml.4's scope, not
    // this bead's. `#[allow(dead_code)]` documents that gap rather than
    // hiding it.
    #[allow(dead_code)]
    Unlocked(Client),
}

/// Constructs a `Client` the way every login attempt does: no credentials
/// touched, no network calls made here. Called fresh for each login/2FA
/// attempt in `crate::auth_routes` (a `Client` used in an attempt that
/// didn't reach `IdentityTokenResponse::Authenticated` never picks up any
/// internal state worth keeping -- see eml.3 report for why re-using the
/// client across a 2FA retry isn't necessary).
#[must_use]
pub fn build_client() -> Client {
    let settings = ClientSettings {
        identity_url: "https://identity.bitwarden.com".to_string(),
        api_url: "https://api.bitwarden.com".to_string(),
        device_type: DeviceType::SDK,
        ..ClientSettings::default()
    };
    Client::new(Some(settings))
}

/// Unions every registered `TransportProvider`'s view of the world into one
/// surface `crate::transport_routes` reads from. Phase 1
/// (ai-bitwarden-hw-key-eml.5) registers exactly one provider (the desktop
/// emulator over HTTP, see `with_emulator`); BLE/USB providers are Phase 2
/// (the T-Embed hardware migration) and would simply be additional entries
/// in `providers`.
///
/// `Arc<dyn TransportProvider>` clones cheaply, so `Vec` of them derives
/// `Clone` for free -- `AppState` clones a `TransportRegistry` on every
/// handler invocation like everything else it holds.
#[derive(Clone)]
pub struct TransportRegistry {
    providers: Vec<Arc<dyn TransportProvider>>,
}

impl TransportRegistry {
    #[must_use]
    pub fn new(providers: Vec<Arc<dyn TransportProvider>>) -> Self {
        Self { providers }
    }

    /// Convenience constructor wiring up the one Phase-1 provider (the
    /// desktop emulator over HTTP) at `base_url`. See `main.rs` for where
    /// `base_url` is resolved (the `EMULATOR_URL` env var, falling back to
    /// `crate::transport::DEFAULT_EMULATOR_URL`).
    #[must_use]
    pub fn with_emulator(base_url: String) -> Self {
        Self::new(vec![Arc::new(EmulatorTransportProvider::new(base_url))])
    }

    /// Union of every registered provider's `list_targets()`.
    #[must_use]
    pub fn list_all_targets(&self) -> Vec<DeviceDescriptor> {
        self.providers
            .iter()
            .flat_map(|provider| provider.list_targets())
            .collect()
    }

    /// Tries each registered provider in turn; the first that recognizes
    /// `id` wins. Returns `TransportError::UnknownDevice` if none do.
    ///
    /// # Errors
    ///
    /// Returns `TransportError::UnknownDevice` if no registered provider
    /// recognizes `id`, or whatever error the matching provider's
    /// `connect` itself returns.
    pub async fn connect(&self, id: &str) -> Result<Box<dyn crate::transport::DeviceTransport>, TransportError> {
        for provider in &self.providers {
            match provider.connect(id).await {
                Ok(transport) => return Ok(transport),
                Err(TransportError::UnknownDevice(_)) => {}
                Err(other) => return Err(other),
            }
        }
        Err(TransportError::UnknownDevice(id.to_string()))
    }
}

/// The ONLY place plaintext vault passwords live server-side, held in
/// memory only (never persisted to disk -- see `crate::vault` for how it's
/// populated via SDK sync + decrypt, and `crate::vault_routes` for why the
/// `/api/vault/list` surface built on top of this never serializes a
/// password back out).
///
/// `replace` zeroizes the outgoing generation's passwords before dropping
/// them (best-effort: `String::zeroize` overwrites the heap buffer via a
/// volatile write, matching the level of rigor `Zeroizing<String>` gets
/// elsewhere in this crate for the stashed master password -- see
/// `PendingTwoFactorLogin`). `push_protocol::Credential` is a shared wire
/// type (also used by the emulator/device side), so it does not itself
/// derive `Zeroize`; this store does the zeroizing at the boundary instead.
#[derive(Clone, Default)]
pub struct VaultCredentialStore {
    credentials: Arc<Mutex<Vec<Credential>>>,
}

impl VaultCredentialStore {
    /// Replaces the retained credential set (e.g. after a vault sync),
    /// zeroizing the passwords of whatever generation this replaces.
    pub async fn replace(&self, new_credentials: Vec<Credential>) {
        let mut guard = self.credentials.lock().await;
        for credential in guard.iter_mut() {
            credential.password.zeroize();
        }
        *guard = new_credentials;
    }

    /// Returns a clone of the full credential set, WITH passwords. Callers
    /// outside this module must never forward this verbatim over HTTP --
    /// see `crate::vault_routes::VaultListItem` for the metadata-only
    /// projection that is safe to serve to the browser.
    pub async fn get_all(&self) -> Vec<Credential> {
        self.credentials.lock().await.clone()
    }

    /// Drops the retained credential set, zeroizing passwords first. Called
    /// on `/api/auth/lock` and `/api/auth/logout` (see `crate::auth_routes`)
    /// -- there is no reason for a server-side password to outlive the
    /// session that decrypted it.
    pub async fn clear(&self) {
        self.replace(Vec::new()).await;
    }
}

/// Shared state handed to every axum handler via `axum::extract::State`.
///
/// `Client` (inside `Session`) is `Send + Sync + Clone` (confirmed in
/// eml.1: it wraps `Arc<InternalClient>`), so `Arc<Mutex<Session>>` is safe
/// to clone across handler invocations/tasks.
///
/// `transports` is read by `crate::transport_routes` (`GET /api/devices`,
/// `POST /api/sync`) -- see `TransportRegistry` above.
#[derive(Clone)]
pub struct AppState {
    pub session: Arc<Mutex<Session>>,
    pub transports: TransportRegistry,
    pub api_token: String,
    /// See `VaultCredentialStore` docs -- the server-side-only decrypted
    /// vault, populated by `crate::vault_routes::sync`.
    pub vault_credentials: VaultCredentialStore,
}
