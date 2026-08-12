//! Route handlers for the web-companion axum server.

use std::path::PathBuf;

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use crate::state::AppState;

/// Unauthenticated liveness check.
pub async fn healthz() -> &'static str {
    "ok"
}

/// `web-companion/static/`, resolved relative to the crate manifest so the
/// server finds it regardless of the process's current working directory.
pub fn static_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static")
}

/// Serves `static/index.html` with the `__API_TOKEN__` placeholder
/// replaced by the real per-process bearer token. See `crate::auth` module
/// docs for why this is a dynamic handler rather than a `ServeDir` entry.
pub async fn serve_index(State(state): State<AppState>) -> Response {
    let path = static_dir().join("index.html");
    match tokio::fs::read_to_string(&path).await {
        Ok(template) => {
            let body = template.replace("__API_TOKEN__", &state.api_token);
            Html(body).into_response()
        }
        Err(err) => {
            let display = path.display();
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read {display}: {err}"),
            )
                .into_response()
        }
    }
}

/// Stub `/api/auth/status` -- exists only to exercise the auth middleware
/// (see `crate::auth`) and give eml.3/eml.5 a route pattern to extend.
/// Real login/session-status logic is out of scope for this bead.
pub async fn auth_status_stub() -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}
