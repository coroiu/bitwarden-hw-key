//! Bearer-token authentication for `/api/*` routes.
//!
//! **Token generation:** a fresh random token is generated once per
//! process at startup (`generate_api_token`, backed by
//! `uuid::Uuid::new_v4`, which sources randomness from the OS CSPRNG via
//! `getrandom`). It is held only in memory (`AppState::api_token`) --
//! never written to disk, never logged.
//!
//! **Token delivery:** the token is delivered to the browser by
//! substituting a `__API_TOKEN__` placeholder in `static/index.html` at
//! request time (see `crate::routes::serve_index`), rather than via a
//! `tower_http::services::ServeDir` static-file entry for `index.html`.
//! This lets same-origin JS in the page read the token (e.g. into a JS
//! variable it attaches as an `Authorization` header on `fetch` calls)
//! without writing the token to disk or exposing it at a predictable
//! static URL. All other static assets that don't need the token (CSS/JS
//! bundles) are still served unmodified via `ServeDir`.
//!
//! **Enforcement:** this middleware (`require_bearer_token`) is applied
//! only to the `/api/*` sub-router (see `crate::build_app`) via
//! `axum::middleware::from_fn_with_state`. `/healthz` and the static UI
//! are intentionally left unauthenticated -- the loopback bind
//! (`127.0.0.1:3000`, see `crate::main`) plus this token together are the
//! security boundary for `/api/*`.

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

const BEARER_PREFIX: &str = "Bearer ";

/// Generates a per-process bearer token. See module docs for the
/// randomness source and rationale.
#[must_use]
pub fn generate_api_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// axum middleware requiring `Authorization: Bearer <token>` matching
/// `state.api_token`. See module docs for what this is (and isn't) applied
/// to.
///
/// # Errors
///
/// Returns `StatusCode::UNAUTHORIZED` if the request has no bearer token,
/// or one that doesn't match `state.api_token`.
pub async fn require_bearer_token(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let provided_token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix(BEARER_PREFIX));

    match provided_token {
        Some(token) if constant_time_eq(token.as_bytes(), state.api_token.as_bytes()) => {
            Ok(next.run(req).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Constant-time byte comparison, used instead of a short-circuiting `==`
/// on the bearer token. This is defense-in-depth rather than a
/// load-bearing mitigation (the token is loopback-only and
/// process-lifetime-scoped), but it costs nothing to get right.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn constant_time_eq_matches_equal_slices() {
        assert!(constant_time_eq(b"abc", b"abc"));
    }

    #[test]
    fn constant_time_eq_rejects_different_slices() {
        assert!(!constant_time_eq(b"abc", b"abd"));
    }

    #[test]
    fn constant_time_eq_rejects_different_lengths() {
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
