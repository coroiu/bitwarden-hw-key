//! Device transport abstraction: pushing a `push_protocol::SyncRequest`
//! (server -> device, CBOR over HTTP for Phase 1) to whatever hardware key
//! is currently reachable.
//!
//! Two traits do the work:
//!
//! - `DeviceTransport` -- a single connected/addressable device that can
//!   accept a credential push (`push`) and describe itself (`descriptor`).
//! - `TransportProvider` -- enumerates devices reachable over one medium
//!   (`list_targets`, no network needed) and opens a `DeviceTransport` for
//!   a chosen one (`connect`, fallible -- the id may be stale/unknown).
//!
//! Phase 1 (this bead, ai-bitwarden-hw-key-eml.5) implements exactly one
//! medium: `HttpEmulatorTransport` / `EmulatorTransportProvider`, talking to
//! the desktop emulator's `/api/sync` (`emulator/src/desktop/http_server.rs`,
//! UNCHANGED by this crate) over plain HTTP + CBOR -- the async port of the
//! retired `companion::push_to_device` (`ureq` -> `reqwest`). BLE/USB
//! providers are Phase 2 (the T-Embed hardware migration); see `DeviceKind`.
//!
//! See `crate::state::TransportRegistry` for how multiple providers (future
//! BLE/USB) would union under one `list_all_targets`/`connect` surface, and
//! `crate::transport_routes` for the HTTP surface built on top of this
//! module.
//!
//! ## Security posture
//!
//! `TransportError` is deliberately opaque to HTTP callers -- same posture
//! as `crate::vault::VaultSyncError`: never forwarded verbatim over HTTP,
//! only logged server-side (see `crate::transport_routes::log_transport_error`).
//! Neither `DeviceDescriptor` nor `TransportError` ever carries credential
//! data; the plaintext password only ever appears in the CBOR body built
//! inside `HttpEmulatorTransport::push`.

use std::fmt;
use std::sync::Once;

use push_protocol::{SyncRequest, SyncResponse};
use serde::Serialize;

/// Installs the process-wide rustls `CryptoProvider` exactly once. See the
/// `rustls` dependency comment in `Cargo.toml` for the full "why": reqwest
/// 0.13's resolved feature set here (`rustls-no-provider`) does not
/// auto-install a default provider the way its `rustls` meta-feature would,
/// so `reqwest::Client::new()` panics at construction time until this runs.
/// Idempotent -- `install_default` returning `Err` (already installed,
/// e.g. by some other crate in the process) is not a bug and is ignored.
fn ensure_crypto_provider_installed() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Default base URL for the desktop emulator's HTTP sync server (see
/// `emulator/src/desktop/http_server.rs`). Overridable via the `EMULATOR_URL`
/// env var, resolved once in `main.rs` (see `state::TransportRegistry::with_emulator`
/// callers) -- this constant is only the documented fallback, not something
/// `transport.rs` itself reads from the environment.
pub const DEFAULT_EMULATOR_URL: &str = "http://127.0.0.1:8080";

/// Fixed id for the (currently sole) emulator target. Both
/// `EmulatorTransportProvider::connect` and the descriptor builder key off
/// this constant so the id used to list a device and the id used to
/// address it can't drift apart.
const EMULATOR_DEVICE_ID: &str = "emulator";

/// Distinguishes the physical/logical medium a `DeviceTransport` talks
/// over. Only `Emulator` exists today -- BLE/USB are Phase 2 additions (the
/// T-Embed hardware migration). `#[non_exhaustive]` so adding those variants
/// later isn't a breaking change for any match arm outside this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DeviceKind {
    Emulator,
}

/// Browser-facing device descriptor. No credential data of any kind -- safe
/// to serialize directly into an HTTP response body (see
/// `crate::transport_routes::list_devices`).
#[derive(Debug, Clone, Serialize)]
pub struct DeviceDescriptor {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
}

fn emulator_descriptor() -> DeviceDescriptor {
    DeviceDescriptor {
        id: EMULATOR_DEVICE_ID.to_string(),
        name: "Desktop Emulator".to_string(),
        kind: DeviceKind::Emulator,
    }
}

/// Errors from a `DeviceTransport`/`TransportProvider`. Deliberately opaque
/// to HTTP callers -- see module docs.
#[derive(Debug)]
pub enum TransportError {
    /// The device could not be reached at all (connection refused, DNS
    /// failure, timeout, etc.) -- a network-level failure, not a protocol
    /// one.
    Unreachable(String),
    /// The device was reached but its response could not be parsed as a
    /// valid `SyncResponse`, or it reported a non-success HTTP status, or
    /// the outbound request itself could not be encoded.
    Protocol(String),
    /// `TransportProvider::connect` (or `TransportRegistry::connect`) was
    /// asked for a device id that no registered provider recognizes.
    UnknownDevice(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::Unreachable(msg) => write!(f, "device unreachable: {msg}"),
            TransportError::Protocol(msg) => write!(f, "device protocol error: {msg}"),
            TransportError::UnknownDevice(id) => write!(f, "unknown device: {id}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// A connected/addressable device that can accept a credential push.
/// Obtained via `TransportProvider::connect`.
#[async_trait::async_trait]
pub trait DeviceTransport: Send + Sync {
    /// Pushes `request` to the device and returns its typed reply. Never
    /// logs or otherwise leaks `request` (which carries plaintext
    /// passwords) outside of the wire payload itself.
    async fn push(&self, request: &SyncRequest) -> Result<SyncResponse, TransportError>;

    /// The descriptor for the device this transport is connected to.
    fn descriptor(&self) -> DeviceDescriptor;
}

/// Enumerates and connects to devices reachable over one medium (e.g. "the
/// emulator over HTTP", later "BLE scan results", "USB serial ports").
#[async_trait::async_trait]
pub trait TransportProvider: Send + Sync {
    /// Lists every device this provider currently sees. No network access
    /// required -- for `EmulatorTransportProvider` this is a fixed,
    /// statically-known single entry; a future BLE provider would return
    /// the results of its last scan.
    fn list_targets(&self) -> Vec<DeviceDescriptor>;

    /// Opens a `DeviceTransport` for `id`. Fails with
    /// `TransportError::UnknownDevice` if this provider doesn't recognize
    /// `id` (e.g. it belongs to a different provider, or the device is
    /// gone) -- `crate::state::TransportRegistry::connect` relies on this
    /// specific variant to know when to try the next provider.
    async fn connect(&self, id: &str) -> Result<Box<dyn DeviceTransport>, TransportError>;
}

/// `DeviceTransport` that pushes to the desktop emulator's `/api/sync` over
/// plain HTTP + CBOR. This is the async port of the retired
/// `companion::push_to_device` (`ureq::post` -> `reqwest::Client::post`),
/// including decoding the JSON `SyncResponse` reply (the retired CLI
/// discarded it; this returns it typed).
///
/// Holds a `reqwest::Client` rather than constructing one per call --
/// `reqwest::Client` is designed to be built once and reused/cheaply cloned
/// (it wraps a connection pool internally).
pub struct HttpEmulatorTransport {
    base_url: String,
    client: reqwest::Client,
}

impl HttpEmulatorTransport {
    #[must_use]
    pub fn new(base_url: String) -> Self {
        ensure_crypto_provider_installed();
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl DeviceTransport for HttpEmulatorTransport {
    async fn push(&self, request: &SyncRequest) -> Result<SyncResponse, TransportError> {
        let mut cbor_body = Vec::new();
        ciborium::into_writer(request, &mut cbor_body)
            .map_err(|err| TransportError::Protocol(format!("failed to encode CBOR: {err}")))?;

        let url = format!("{}/api/sync", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/cbor")
            .body(cbor_body)
            .send()
            .await
            .map_err(|err| TransportError::Unreachable(err.to_string()))?;

        if !response.status().is_success() {
            return Err(TransportError::Protocol(format!(
                "device responded with status {}",
                response.status()
            )));
        }

        response
            .json::<SyncResponse>()
            .await
            .map_err(|err| TransportError::Protocol(format!("failed to decode response: {err}")))
    }

    fn descriptor(&self) -> DeviceDescriptor {
        emulator_descriptor()
    }
}

/// `TransportProvider` for the single desktop-emulator target. Phase 1's
/// only provider.
pub struct EmulatorTransportProvider {
    base_url: String,
}

impl EmulatorTransportProvider {
    #[must_use]
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

#[async_trait::async_trait]
impl TransportProvider for EmulatorTransportProvider {
    fn list_targets(&self) -> Vec<DeviceDescriptor> {
        // Built without constructing a `HttpEmulatorTransport` (and thus
        // without allocating a `reqwest::Client`) -- listing targets should
        // not require standing up a connection pool. See `emulator_descriptor`.
        vec![emulator_descriptor()]
    }

    async fn connect(&self, id: &str) -> Result<Box<dyn DeviceTransport>, TransportError> {
        if id == EMULATOR_DEVICE_ID {
            Ok(Box::new(HttpEmulatorTransport::new(self.base_url.clone())))
        } else {
            Err(TransportError::UnknownDevice(id.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use push_protocol::Credential;
    use uuid::Uuid;

    use super::*;

    fn sample_sync_request() -> SyncRequest {
        SyncRequest {
            credentials: vec![Credential {
                id: Uuid::new_v4(),
                name: "GitHub".to_string(),
                username: "octocat".to_string(),
                password: "hunter2".to_string(),
                uri: Some("https://github.com".to_string()),
                notes: None,
            }],
        }
    }

    /// Real wire-format proof, matching the retired `companion` crate's
    /// `sync_request_round_trips_through_cbor` test: encode via
    /// `ciborium::into_writer`, decode via `ciborium::from_reader`, using
    /// the REAL `push_protocol` types -- not a hand-rolled stand-in for the
    /// wire format.
    #[test]
    fn sync_request_round_trips_through_cbor() {
        let request = sample_sync_request();

        let mut bytes = Vec::new();
        ciborium::into_writer(&request, &mut bytes).expect("encode should succeed");

        let decoded: SyncRequest =
            ciborium::from_reader(bytes.as_slice()).expect("decode should succeed");

        assert_eq!(decoded.credentials.len(), 1);
        assert_eq!(decoded.credentials[0].id, request.credentials[0].id);
        assert_eq!(decoded.credentials[0].password, "hunter2");
    }

    #[test]
    fn emulator_provider_lists_exactly_one_emulator_target() {
        let provider = EmulatorTransportProvider::new(DEFAULT_EMULATOR_URL.to_string());
        let targets = provider.list_targets();

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id, "emulator");
        assert_eq!(targets[0].kind, DeviceKind::Emulator);
    }

    #[tokio::test]
    async fn connect_to_unknown_id_is_a_clean_error() {
        let provider = EmulatorTransportProvider::new(DEFAULT_EMULATOR_URL.to_string());
        let result = provider.connect("some-other-device").await;

        assert!(matches!(result, Err(TransportError::UnknownDevice(_))));
    }

    #[tokio::test]
    async fn connect_to_known_id_succeeds() {
        let provider = EmulatorTransportProvider::new(DEFAULT_EMULATOR_URL.to_string());
        let result = provider.connect("emulator").await;

        assert!(result.is_ok());
    }

    /// Proves the "device unreachable" path maps cleanly to
    /// `TransportError::Unreachable` rather than panicking, without needing
    /// the real emulator binary running: binds a `TcpListener` to get a
    /// free loopback port, then immediately drops it so nothing is
    /// listening there anymore before the push is attempted.
    #[tokio::test]
    async fn push_to_unreachable_device_is_a_clean_error() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
        let addr = listener.local_addr().expect("read local addr");
        drop(listener);

        let transport = HttpEmulatorTransport::new(format!("http://{addr}"));
        let result = transport.push(&sample_sync_request()).await;

        assert!(matches!(result, Err(TransportError::Unreachable(_))));
    }
}
