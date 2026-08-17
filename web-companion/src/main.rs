//! web-companion: local HTTP server that lets the Bitwarden Web Vault sync
//! credentials to the hardware key over a same-origin, token-gated API.
//!
//! Phase 1 (ai-bitwarden-hw-key-eml.2): axum server skeleton. Binds
//! `127.0.0.1:3000` -- loopback ONLY, never `0.0.0.0` (the desktop emulator
//! owns `:8080` on the same machine; see repo `CLAUDE.md`).
//!
//! Phase 2 (ai-bitwarden-hw-key-eml.3): the `/api/auth/*` login state
//! machine. Phase 3 (eml.4-eml.6): vault sync, device push, and the thin
//! browser UI. See `web_companion::build_app` for the actual router wiring
//! and `crate::` docs on each module for the pieces built on top of it.
//!
//! This binary is now a thin entry point: `web_companion` (this crate's own
//! library target, `src/lib.rs`) owns `build_app`/`emulator_url` and every
//! route module, so a Phase-1 integration test
//! (`tests/emulator_integration.rs`, ai-bitwarden-hw-key-eml.7) can reach
//! `web_companion::transport::HttpEmulatorTransport` directly -- the exact
//! client `POST /api/sync` uses in production -- without a library target
//! that surface would be unreachable from outside `src/main.rs`.

use std::sync::Arc;

use tokio::{net::TcpListener, sync::Mutex};

use web_companion::auth::generate_api_token;
use web_companion::state::{AppState, Session, TransportRegistry, VaultCredentialStore};
use web_companion::{build_app, emulator_url};

#[tokio::main]
async fn main() {
    let state = AppState {
        session: Arc::new(Mutex::new(Session::LoggedOut)),
        transports: TransportRegistry::with_emulator(emulator_url()),
        api_token: generate_api_token(),
        vault_credentials: VaultCredentialStore::default(),
    };

    let app = build_app(state);

    // Loopback ONLY -- never 0.0.0.0. See module docs.
    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind 127.0.0.1:3000");

    // Guaranteed-visible startup banner: println! (not tracing/log) so it
    // shows even with no subscriber configured. Without this the server was
    // silent after cargo run's "Running ..." line and looked dead even
    // though it was up and serving (ai-bitwarden-hw-key-eml.10).
    let addr = listener
        .local_addr()
        .expect("bound listener has a local address");
    println!("Web companion listening on http://{addr}");
    println!("Open that URL in your browser to log in and sync.");

    axum::serve(listener, app)
        .await
        .expect("web-companion server exited unexpectedly");

    println!("Web companion server shut down.");
}
