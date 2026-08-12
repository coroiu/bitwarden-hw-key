//! web-companion: local HTTP server that lets the Bitwarden Web Vault sync
//! credentials to the hardware key over a same-origin, token-gated API.
//!
//! Phase 1 (ai-bitwarden-hw-key-eml.2): axum server skeleton. Binds
//! `127.0.0.1:3000` -- loopback ONLY, never `0.0.0.0` (the desktop emulator
//! owns `:8080` on the same machine; see repo `CLAUDE.md`).
//!
//! Phase 2 (ai-bitwarden-hw-key-eml.3, this bead): the `/api/auth/*` login
//! state machine (see `crate::auth_routes`), built on the `Client`
//! construction proved compiling against the pinned SDK rev in eml.1.
//! Vault sync is still out of scope -- see eml.4.
//!
//! See `crate::auth` for the token generation/delivery/enforcement design.

mod auth;
mod auth_routes;
mod routes;
mod state;
mod transport;
mod transport_routes;
mod vault;
mod vault_routes;

use std::env;
use std::sync::Arc;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tokio::{net::TcpListener, sync::Mutex};
use tower::ServiceBuilder;
use tower_http::services::ServeDir;

use auth::{generate_api_token, require_bearer_token};
use routes::{healthz, serve_index, static_dir};
use state::{AppState, Session, TransportRegistry};
use transport::DEFAULT_EMULATOR_URL;

/// Resolves the desktop emulator's base URL for the sole Phase-1
/// `DeviceTransport`: the `EMULATOR_URL` env var if set, else
/// `transport::DEFAULT_EMULATOR_URL`. Resolved once, here, rather than
/// inside `transport.rs` itself -- see that module's doc comment.
fn emulator_url() -> String {
    env::var("EMULATOR_URL").unwrap_or_else(|_| DEFAULT_EMULATOR_URL.to_string())
}

/// Builds the axum `Router`. Split out from `main` so tests (see
/// `src/tests.rs`) can construct the same app with a caller-supplied
/// `AppState` (e.g. a known test token) and drive it in-process via
/// `tower::ServiceExt::oneshot`, without binding a real socket.
fn build_app(state: AppState) -> Router {
    let api_routes = Router::new()
        .route("/auth/status", get(auth_routes::status))
        .route("/auth/login", post(auth_routes::login))
        .route("/auth/login-apikey", post(auth_routes::login_apikey))
        .route("/auth/2fa", post(auth_routes::two_factor))
        .route("/auth/lock", post(auth_routes::lock))
        .route("/auth/logout", post(auth_routes::logout))
        .route("/vault/sync", post(vault_routes::sync))
        .route("/vault/list", get(vault_routes::list))
        .route("/vault/status", get(vault_routes::status))
        .route("/devices", get(transport_routes::list_devices))
        .route("/sync", post(transport_routes::sync))
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
    let state = AppState {
        session: Arc::new(Mutex::new(Session::LoggedOut)),
        transports: TransportRegistry::with_emulator(emulator_url()),
        api_token: generate_api_token(),
        vault_credentials: state::VaultCredentialStore::default(),
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
            transports: TransportRegistry::with_emulator(DEFAULT_EMULATOR_URL.to_string()),
            api_token: TEST_TOKEN.to_string(),
            vault_credentials: state::VaultCredentialStore::default(),
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
        // 200 with a real status body, not 401/403 -- proves the
        // middleware let it through to the real handler (see
        // auth_status_reflects_logged_out_by_default below for the body).
        assert_eq!(response.status(), StatusCode::OK);
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

    async fn authed_request(
        method: &str,
        path: &str,
        json_body: Option<&str>,
    ) -> axum::http::Response<Body> {
        let app = build_app(test_state());
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("Authorization", format!("Bearer {TEST_TOKEN}"));
        let body = if let Some(json) = json_body {
            builder = builder.header("Content-Type", "application/json");
            Body::from(json.to_string())
        } else {
            Body::empty()
        };
        app.oneshot(builder.body(body).unwrap()).await.unwrap()
    }

    async fn body_json(response: axum::http::Response<Body>) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn auth_status_reflects_logged_out_by_default() {
        let response = authed_request("GET", "/api/auth/status", None).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["status"], "logged_out");
    }

    #[tokio::test]
    async fn login_missing_master_password_is_bad_request() {
        let response = authed_request("POST", "/api/auth/login", Some(r#"{"email":"a@b.com"}"#))
            .await;
        assert!(response.status().is_client_error());
    }

    #[tokio::test]
    async fn login_malformed_json_is_client_error_not_panic() {
        let response = authed_request("POST", "/api/auth/login", Some("not json")).await;
        assert!(response.status().is_client_error());
    }

    #[tokio::test]
    async fn login_empty_master_password_is_bad_request() {
        let response = authed_request(
            "POST",
            "/api/auth/login",
            Some(r#"{"email":"a@b.com","master_password":""}"#),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn two_factor_without_pending_login_is_bad_request() {
        let response = authed_request(
            "POST",
            "/api/auth/2fa",
            Some(r#"{"provider":1,"token":"123456"}"#),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn two_factor_malformed_provider_is_client_error_not_panic() {
        // provider must be a small integer (TwoFactorProvider's repr) --
        // a string here must be a clean 4xx, not a panic.
        let response = authed_request(
            "POST",
            "/api/auth/2fa",
            Some(r#"{"provider":"not-a-number","token":"123456"}"#),
        )
        .await;
        assert!(response.status().is_client_error());
    }

    #[tokio::test]
    async fn lock_without_active_session_is_conflict() {
        let response = authed_request("POST", "/api/auth/lock", None).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn logout_without_active_session_is_idempotent_ok() {
        let response = authed_request("POST", "/api/auth/logout", None).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["status"], "logged_out");
    }

    #[tokio::test]
    async fn vault_list_without_unlocked_session_is_conflict() {
        let response = authed_request("GET", "/api/vault/list", None).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn vault_sync_without_unlocked_session_is_conflict() {
        let response = authed_request("POST", "/api/vault/sync", None).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn vault_status_without_unlocked_session_is_conflict() {
        let response = authed_request("GET", "/api/vault/status", None).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn vault_routes_require_bearer_token() {
        let app = build_app(test_state());
        let response = app
            .oneshot(Request::get("/api/vault/list").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn devices_without_unlocked_session_is_conflict() {
        let response = authed_request("GET", "/api/devices", None).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn devices_route_requires_bearer_token() {
        let app = build_app(test_state());
        let response = app
            .oneshot(Request::get("/api/devices").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn sync_without_unlocked_session_is_conflict() {
        let response = authed_request(
            "POST",
            "/api/sync",
            Some(r#"{"target_id":"emulator","item_ids":null}"#),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn sync_route_requires_bearer_token() {
        let app = build_app(test_state());
        let response = app
            .oneshot(
                Request::post("/api/sync")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"target_id":"emulator"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn sync_malformed_json_is_client_error_not_panic() {
        let response = authed_request("POST", "/api/sync", Some("not json")).await;
        assert!(response.status().is_client_error());
    }

    #[tokio::test]
    async fn login_apikey_missing_fields_is_bad_request() {
        let response = authed_request(
            "POST",
            "/api/auth/login-apikey",
            Some(r#"{"client_id":"","client_secret":"","master_password":""}"#),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    /// Exercises the REAL SDK login error path against the real Bitwarden
    /// identity server with bogus credentials. Proves `LoginError` maps to
    /// a clean 401 (not a 500/panic) end-to-end. Makes an outbound network
    /// call -- `#[ignore]`d by default; run manually with:
    /// `cargo test --test-threads=1 -- --ignored bogus_credentials_login_maps_to_401`
    #[tokio::test]
    #[ignore = "makes a real network call to https://identity.bitwarden.com"]
    async fn bogus_credentials_login_maps_to_401() {
        let response = authed_request(
            "POST",
            "/api/auth/login",
            Some(r#"{"email":"definitely-not-a-real-account@example.com","master_password":"definitely-wrong"}"#),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
