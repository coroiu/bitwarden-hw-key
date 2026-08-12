//! web-companion: local HTTP server that lets the Bitwarden Web Vault sync
//! credentials to the hardware key over a same-origin, token-gated API.
//!
//! Phase 1 (this bead, ai-bitwarden-hw-key-eml.2): axum server skeleton
//! only. Binds `127.0.0.1:3000` -- loopback ONLY, never `0.0.0.0` (the
//! desktop emulator owns `:8080` on the same machine; see repo
//! `CLAUDE.md`). No SDK login/sync logic yet -- see eml.3 (login) / eml.4
//! (sync), which will build on the `Client` construction proved compiling
//! against the pinned SDK rev in eml.1 and wired into `AppState` here.
//!
//! See `crate::auth` for the token generation/delivery/enforcement design.

mod auth;
mod routes;
mod state;

use std::sync::Arc;

use axum::{middleware, routing::get, Router};
use bitwarden_core::{Client, ClientSettings, DeviceType};
use tokio::{net::TcpListener, sync::Mutex};
use tower::ServiceBuilder;
use tower_http::services::ServeDir;

use auth::{generate_api_token, require_bearer_token};
use routes::{auth_status_stub, healthz, serve_index, static_dir};
use state::{AppState, Session, TransportRegistry};

/// Constructs a `Client` the way Phase 1 will (carried over from the
/// eml.1 feasibility spike): no credentials touched, no network calls made
/// here.
fn build_client() -> Client {
    let settings = ClientSettings {
        identity_url: "https://identity.bitwarden.com".to_string(),
        api_url: "https://api.bitwarden.com".to_string(),
        device_type: DeviceType::SDK,
        ..ClientSettings::default()
    };
    Client::new(Some(settings))
}

/// Builds the axum `Router`. Split out from `main` so tests (see
/// `src/tests.rs`) can construct the same app with a caller-supplied
/// `AppState` (e.g. a known test token) and drive it in-process via
/// `tower::ServiceExt::oneshot`, without binding a real socket.
fn build_app(state: AppState) -> Router {
    let api_routes = Router::new()
        .route("/auth/status", get(auth_status_stub))
        .layer(ServiceBuilder::new().layer(middleware::from_fn_with_state(
            state.clone(),
            require_bearer_token,
        )));

    Router::new()
        .route("/healthz", get(healthz))
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new(static_dir()))
        .with_state(state)
}

#[tokio::main]
async fn main() {
    // Proves `Client::new` runs at startup (this bead's core claim) without
    // touching credentials or the network. The result is intentionally
    // discarded: a fresh process with no prior login has nowhere to put a
    // `Client` yet -- `Session::LoggedOut` carries none by design (see
    // `state::Session` docs). eml.3 will thread a real `Client` into
    // `Session::Locked`/`Unlocked` once login exists.
    let _client = build_client();

    let state = AppState {
        session: Arc::new(Mutex::new(Session::LoggedOut)),
        transports: TransportRegistry,
        api_token: generate_api_token(),
    };

    let app = build_app(state);

    // Loopback ONLY -- never 0.0.0.0. See module docs.
    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind 127.0.0.1:3000");

    axum::serve(listener, app)
        .await
        .expect("web-companion server exited unexpectedly");
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    const TEST_TOKEN: &str = "test-token-do-not-use-in-prod";

    fn test_state() -> AppState {
        AppState {
            session: Arc::new(Mutex::new(Session::LoggedOut)),
            transports: TransportRegistry,
            api_token: TEST_TOKEN.to_string(),
        }
    }

    #[tokio::test]
    async fn healthz_is_unauthenticated() {
        let app = build_app(test_state());
        let response = app
            .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_route_without_token_is_rejected() {
        let app = build_app(test_state());
        let response = app
            .oneshot(
                Request::get("/api/auth/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_route_with_wrong_token_is_rejected() {
        let app = build_app(test_state());
        let response = app
            .oneshot(
                Request::get("/api/auth/status")
                    .header("Authorization", "Bearer wrong-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_route_with_correct_token_reaches_handler() {
        let app = build_app(test_state());
        let response = app
            .oneshot(
                Request::get("/api/auth/status")
                    .header("Authorization", format!("Bearer {TEST_TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // 501, not 401/403 -- proves the middleware let it through to the
        // (deliberately unimplemented) stub handler.
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[tokio::test]
    async fn index_html_has_token_substituted_and_no_placeholder_left() {
        let app = build_app(test_state());
        let response = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains(TEST_TOKEN));
        assert!(!body.contains("__API_TOKEN__"));
    }
}
