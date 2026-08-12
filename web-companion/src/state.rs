//! Shared application state for the web-companion axum server.

use std::sync::Arc;

use bitwarden_core::Client;
use tokio::sync::Mutex;

/// Authentication/session state for the (eventually) single Bitwarden
/// account this companion process manages.
///
/// `Locked(Client)` / `Unlocked(Client)` are never constructed by this
/// crate today -- login is out of scope for this bead
/// (ai-bitwarden-hw-key-eml.2) and is owned by eml.3 (login) / eml.4
/// (sync/unlock). A freshly-started process with no prior login has no
/// `Client` worth attaching to a session yet, so `LoggedOut` carries none;
/// eml.3 will thread a `Client` into `Locked`/`Unlocked` once real login
/// exists. `#[allow(dead_code)]` documents that gap rather than hiding it.
#[allow(dead_code)]
pub enum Session {
    LoggedOut,
    Locked(Client),
    Unlocked(Client),
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
/// `session` and `transports` are not read by any handler yet -- this
/// bead only proves `AppState` has slots for them (per the eml.2 design);
/// eml.3 (login) and the future transport-wiring bead are what will read
/// them. `#[allow(dead_code)]` documents that gap rather than hiding it.
#[allow(dead_code)]
#[derive(Clone)]
pub struct AppState {
    pub session: Arc<Mutex<Session>>,
    pub transports: TransportRegistry,
    pub api_token: String,
}
