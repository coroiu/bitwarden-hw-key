//! Shared application state for the web-companion axum server.

use std::sync::Arc;

use bitwarden_core::{Client, ClientSettings, DeviceType};
use tokio::sync::Mutex;
use zeroize::Zeroizing;

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
pub fn build_client() -> Client {
    let settings = ClientSettings {
        identity_url: "https://identity.bitwarden.com".to_string(),
        api_url: "https://api.bitwarden.com".to_string(),
        device_type: DeviceType::SDK,
        ..ClientSettings::default()
    };
    Client::new(Some(settings))
}

/// Placeholder for the future device-transport registry (BLE/USB/etc.
/// links to the physical hardware key). Real transport wiring is out of
/// scope for this bead -- this type exists only so `AppState` has a slot
/// for it, per the eml.2 design.
#[derive(Debug, Default, Clone)]
pub struct TransportRegistry;

/// Shared state handed to every axum handler via `axum::extract::State`.
///
/// `Client` (inside `Session`) is `Send + Sync + Clone` (confirmed in
/// eml.1: it wraps `Arc<InternalClient>`), so `Arc<Mutex<Session>>` is safe
/// to clone across handler invocations/tasks.
///
/// `transports` is not read by any handler yet -- the future
/// transport-wiring bead is what will read it. `#[allow(dead_code)]`
/// documents that gap rather than hiding it.
#[derive(Clone)]
pub struct AppState {
    pub session: Arc<Mutex<Session>>,
    #[allow(dead_code)]
    pub transports: TransportRegistry,
    pub api_token: String,
}
