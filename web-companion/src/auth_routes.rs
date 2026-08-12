//! `/api/auth/*` route handlers: the login / two-factor / lock / logout /
//! status state machine for the single Bitwarden account this companion
//! process manages.
//!
//! See `crate::auth` for the bearer-token boundary these routes sit behind
//! (unchanged by this module -- these handlers assume they already run
//! inside that boundary) and `crate::state` for the `Session` enum these
//! handlers drive.
//!
//! ## Session state machine
//!
//! ```text
//! LoggedOut
//!   --login (authenticated)-----------------> Unlocked
//!   --login (two_factor required)------------> PendingTwoFactor
//!   --login (bad credentials / error)--------> LoggedOut (unchanged)
//!   --login-apikey (authenticated)-----------> Unlocked
//!   --login-apikey (failed)------------------> LoggedOut (unchanged)
//!
//! PendingTwoFactor
//!   --2fa (correct code)---------------------> Unlocked
//!   --2fa (wrong code / error)---------------> PendingTwoFactor (unchanged; retry allowed)
//!   --logout----------------------------------> LoggedOut
//!
//! Unlocked
//!   --lock------------------------------------> LoggedOut
//!   --logout----------------------------------> LoggedOut
//!
//! (any state) --logout-----------------------> LoggedOut (idempotent, always succeeds)
//! ```
//!
//! `Session::Locked` (see `crate::state`) is never produced by this state
//! machine -- see that module's doc comment for why `lock` goes straight to
//! `LoggedOut` instead.
//!
//! ## Design decisions worth flagging for future beads (eml.4/eml.7)
//!
//! - **2FA retry keeps the pending state on failure** (does not reset to
//!   `LoggedOut`). The brief explicitly allows either choice; this one
//!   avoids forcing the user to re-enter their master password after a
//!   typo'd 2FA code, at the cost of the stashed password living a little
//!   longer in server memory. Given the threat model (loopback-only,
//!   bearer-token gated, single local user), that trade is worth it. There
//!   is **no TTL or max-attempt limit** on a pending 2FA login -- a stuck
//!   pending state lingers until `/api/auth/logout` or process restart.
//!   That's a known gap, out of scope here.
//! - **`login-apikey` cannot report two-factor-required.**
//!   `ApiKeyLoginResponse::two_factor` (bitwarden-core, rev 99ffb6ef) is a
//!   *private* field -- inaccessible outside the `bitwarden-core` crate.
//!   `ApiKeyLoginRequest` also has no `two_factor` field to retry with. So
//!   any non-`authenticated` outcome from `login_api_key` -- whether wrong
//!   credentials or (if it can even happen for this grant type, which
//!   eml.3 did not confirm either way) a 2FA challenge -- surfaces here as
//!   a generic authentication failure. This is a real SDK-surface
//!   limitation, not an oversight; see the eml.3 completion report for the
//!   full signature delta.
//! - **The session `Mutex` is held for the full duration of the SDK login
//!   network call** in `login`, `login_apikey`, and `two_factor`. This is
//!   deliberate: this server is single-user, single-process, and
//!   loopback-only with no expected concurrent auth traffic, so full
//!   serialization is simpler and strictly safer (no lost-update races
//!   between two concurrent logins racing to write `AppState.session`)
//!   than optimistic (check, drop lock, call SDK, re-lock, compare-and-set)
//!   concurrency. The cost -- one `/api/auth/*` request blocks any other
//!   `/api/*` request for the duration of one identity-server round-trip
//!   -- is acceptable here.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use bitwarden_core::auth::login::{
    response::two_factor::TwoFactorProviders, ApiKeyLoginRequest, LoginError,
    PasswordLoginRequest, PasswordLoginResponse, TwoFactorProvider, TwoFactorRequest,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::state::{build_client, AppState, PendingTwoFactorLogin, Session};

// ---------------------------------------------------------------------
// Wire DTOs
//
// None of the request DTOs derive `Debug`/`Display`/`Serialize` -- they
// carry the master password, and the absence of those derives makes an
// accidental `{:?}`/`dbg!`/log of one a compile error rather than a
// runtime leak. Response DTOs never carry secrets (status/provider-list
// only), per the eml.2 security review.
// ---------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub email: String,
    pub master_password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TwoFactorLoginRequest {
    pub provider: TwoFactorProvider,
    pub token: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiKeyLoginBody {
    pub client_id: String,
    pub client_secret: String,
    /// Named `master_password` (not `password`, the SDK's
    /// `ApiKeyLoginRequest` field name) for consistency with
    /// `LoginRequest` -- this is our own wire contract with the browser,
    /// independent of the SDK's internal field naming.
    pub master_password: String,
}

/// `GET /api/auth/status` response shape. Deliberately does not re-serve
/// the two-factor provider list on repeated polls -- see module docs.
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuthStatus {
    LoggedOut,
    TwoFactorRequired,
    Locked,
    Unlocked,
}

/// `POST /api/auth/login` / `POST /api/auth/2fa` success-path response
/// shape. Separate from `AuthStatus` because only the *initiating* login
/// response needs to carry the provider list (a status poll doesn't).
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginResult {
    Unlocked,
    TwoFactorRequired { providers: TwoFactorProviders },
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

fn error_response(status: StatusCode, message: &'static str) -> Response {
    (status, Json(ErrorBody { error: message })).into_response()
}

/// Server-side diagnostic only -- never forwarded to the client (see
/// module docs / eml.2 security review: SDK error internals must not leak
/// to callers). `LoginError` never carries the plaintext master password,
/// but its `IdentityFail` variant can embed identity-server response text,
/// so the full error stays out of any HTTP response body.
fn log_login_error(err: &LoginError) {
    eprintln!("web-companion: login attempt failed: {err}");
}

/// What a successful (`Ok`) SDK login response means for our session.
/// Split out from the handlers so it's unit-testable without a network
/// call or real credentials -- `PasswordLoginResponse` is a plain
/// `pub`-field data struct, so tests can construct one directly to drive
/// this classification (see `tests` module below).
enum LoginOutcome {
    Unlocked,
    TwoFactorRequired(TwoFactorProviders),
    /// Defensive fallback for a hypothetical future SDK response shape
    /// that is neither of the two combinations this SDK revision actually
    /// produces (`authenticated: true, two_factor: None` or
    /// `authenticated: false, two_factor: Some(_)`).
    AuthenticationFailed,
}

fn classify_password_login(response: PasswordLoginResponse) -> LoginOutcome {
    match response.two_factor {
        Some(providers) => LoginOutcome::TwoFactorRequired(providers),
        None if response.authenticated => LoginOutcome::Unlocked,
        None => LoginOutcome::AuthenticationFailed,
    }
}

/// `GET /api/auth/status` -- reflects the current `Session` variant.
/// Status only, no secrets.
pub async fn status(State(state): State<AppState>) -> Json<AuthStatus> {
    let session = state.session.lock().await;
    Json(match &*session {
        Session::LoggedOut => AuthStatus::LoggedOut,
        Session::PendingTwoFactor(_) => AuthStatus::TwoFactorRequired,
        Session::Locked(_) => AuthStatus::Locked,
        Session::Unlocked(_) => AuthStatus::Unlocked,
    })
}

/// `POST /api/auth/login` -- `{ email, master_password }`.
pub async fn login(State(state): State<AppState>, Json(body): Json<LoginRequest>) -> Response {
    if body.email.is_empty() || body.master_password.is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "email and master_password are required",
        );
    }

    // Locked for the whole handler -- see module docs "Design decisions".
    let mut session = state.session.lock().await;
    match &*session {
        Session::Unlocked(_) => return error_response(StatusCode::CONFLICT, "already logged in"),
        Session::PendingTwoFactor(_) => {
            return error_response(
                StatusCode::CONFLICT,
                "two-factor login already in progress",
            )
        }
        Session::LoggedOut | Session::Locked(_) => {}
    }

    let client = build_client();
    let request = PasswordLoginRequest {
        email: body.email,
        password: body.master_password,
        two_factor: None,
    };
    let result = client.auth().login_password(&request).await;

    match result {
        Ok(response) => match classify_password_login(response) {
            LoginOutcome::Unlocked => {
                *session = Session::Unlocked(client);
                Json(LoginResult::Unlocked).into_response()
            }
            LoginOutcome::TwoFactorRequired(providers) => {
                // `request` was only borrowed (`&request`) by the SDK
                // call, so we still own it -- destructure it back apart
                // rather than cloning email/password a second time.
                let PasswordLoginRequest { email, password, .. } = request;
                *session = Session::PendingTwoFactor(PendingTwoFactorLogin {
                    email,
                    master_password: Zeroizing::new(password),
                });
                Json(LoginResult::TwoFactorRequired { providers }).into_response()
            }
            LoginOutcome::AuthenticationFailed => {
                error_response(StatusCode::UNAUTHORIZED, "authentication failed")
            }
        },
        Err(err) => {
            log_login_error(&err);
            error_response(StatusCode::UNAUTHORIZED, "authentication failed")
        }
    }
}

/// `POST /api/auth/2fa` -- `{ provider, token }`. Re-invokes
/// `login_password` with the `email`/`master_password` stashed by `login`
/// above, plus the supplied two-factor code. `remember` is always sent as
/// `false`: this server doesn't persist a "remembered device" state, so
/// there's nothing for the SDK to key that off of.
pub async fn two_factor(
    State(state): State<AppState>,
    Json(body): Json<TwoFactorLoginRequest>,
) -> Response {
    let mut session = state.session.lock().await;
    let (email, password) = match &*session {
        Session::PendingTwoFactor(pending) => (
            pending.email.clone(),
            pending.master_password.as_str().to_owned(),
        ),
        _ => return error_response(StatusCode::BAD_REQUEST, "no pending two-factor login"),
    };

    let client = build_client();
    let request = PasswordLoginRequest {
        email,
        password,
        two_factor: Some(TwoFactorRequest {
            token: body.token,
            provider: body.provider,
            remember: false,
        }),
    };
    let result = client.auth().login_password(&request).await;

    match result {
        Ok(response) => match classify_password_login(response) {
            LoginOutcome::Unlocked => {
                // Overwriting `*session` drops the old `PendingTwoFactor`,
                // zeroizing the stashed password (see `Zeroizing` in
                // `crate::state`).
                *session = Session::Unlocked(client);
                Json(LoginResult::Unlocked).into_response()
            }
            LoginOutcome::TwoFactorRequired(_) | LoginOutcome::AuthenticationFailed => {
                // Wrong code (or some other soft failure) -- leave
                // `PendingTwoFactor` in place so the caller can retry
                // without re-entering the master password. See module
                // docs "Design decisions".
                error_response(StatusCode::UNAUTHORIZED, "authentication failed")
            }
        },
        Err(err) => {
            log_login_error(&err);
            error_response(StatusCode::UNAUTHORIZED, "authentication failed")
        }
    }
}

/// `POST /api/auth/lock` -- only valid from `Unlocked`. See `crate::state`
/// for why this transitions to `LoggedOut` rather than `Locked`.
pub async fn lock(State(state): State<AppState>) -> Response {
    let mut session = state.session.lock().await;
    match &*session {
        Session::Unlocked(_) => {
            // Dropping the old value here drops the `Client`, releasing
            // our only reference to its internal key material.
            *session = Session::LoggedOut;
            Json(AuthStatus::LoggedOut).into_response()
        }
        Session::LoggedOut => error_response(StatusCode::CONFLICT, "not logged in"),
        Session::PendingTwoFactor(_) => {
            error_response(StatusCode::CONFLICT, "not logged in (two-factor pending)")
        }
        Session::Locked(_) => {
            unreachable!("Session::Locked is never constructed by this server; see crate::state")
        }
    }
}

/// `POST /api/auth/logout` -- always succeeds, from any state. Drops
/// whatever was in `session` (a `Client` and/or a stashed
/// `PendingTwoFactorLogin`), zeroizing any stashed password.
pub async fn logout(State(state): State<AppState>) -> Response {
    let mut session = state.session.lock().await;
    *session = Session::LoggedOut;
    Json(AuthStatus::LoggedOut).into_response()
}

/// `POST /api/auth/login-apikey` -- `{ client_id, client_secret,
/// master_password }`. Non-interactive login path for CI/Tess (eml.7) that
/// doesn't require a human to answer a 2FA prompt. See module docs for why
/// this endpoint can't report two-factor-required.
pub async fn login_apikey(
    State(state): State<AppState>,
    Json(body): Json<ApiKeyLoginBody>,
) -> Response {
    if body.client_id.is_empty() || body.client_secret.is_empty() || body.master_password.is_empty()
    {
        return error_response(
            StatusCode::BAD_REQUEST,
            "client_id, client_secret, and master_password are required",
        );
    }

    let mut session = state.session.lock().await;
    match &*session {
        Session::Unlocked(_) => return error_response(StatusCode::CONFLICT, "already logged in"),
        Session::PendingTwoFactor(_) => {
            return error_response(
                StatusCode::CONFLICT,
                "two-factor login already in progress",
            )
        }
        Session::LoggedOut | Session::Locked(_) => {}
    }

    let client = build_client();
    let request = ApiKeyLoginRequest {
        client_id: body.client_id,
        client_secret: body.client_secret,
        password: body.master_password,
    };
    let result = client.auth().login_api_key(&request).await;

    match result {
        Ok(response) if response.authenticated => {
            *session = Session::Unlocked(client);
            Json(AuthStatus::Unlocked).into_response()
        }
        Ok(_) => error_response(StatusCode::UNAUTHORIZED, "authentication failed"),
        Err(err) => {
            log_login_error(&err);
            error_response(StatusCode::UNAUTHORIZED, "authentication failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authenticated_response() -> PasswordLoginResponse {
        PasswordLoginResponse {
            authenticated: true,
            reset_master_password: false,
            force_password_reset: false,
            two_factor: None,
        }
    }

    fn two_factor_required_response() -> PasswordLoginResponse {
        PasswordLoginResponse {
            authenticated: false,
            reset_master_password: false,
            force_password_reset: false,
            two_factor: Some(TwoFactorProviders {
                authenticator: None,
                email: None,
                duo: None,
                organization_duo: None,
                yubi_key: None,
                remember: None,
                web_authn: None,
            }),
        }
    }

    #[test]
    fn classifies_authenticated_response_as_unlocked() {
        assert!(matches!(
            classify_password_login(authenticated_response()),
            LoginOutcome::Unlocked
        ));
    }

    #[test]
    fn classifies_two_factor_response_as_two_factor_required() {
        assert!(matches!(
            classify_password_login(two_factor_required_response()),
            LoginOutcome::TwoFactorRequired(_)
        ));
    }

    #[test]
    fn classifies_unauthenticated_no_two_factor_as_authentication_failed() {
        let response = PasswordLoginResponse {
            authenticated: false,
            reset_master_password: false,
            force_password_reset: false,
            two_factor: None,
        };
        assert!(matches!(
            classify_password_login(response),
            LoginOutcome::AuthenticationFailed
        ));
    }

    #[test]
    fn login_request_round_trips_field_names() {
        let json = r#"{"email":"a@example.com","master_password":"hunter2"}"#;
        let parsed: LoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.email, "a@example.com");
        assert_eq!(parsed.master_password, "hunter2");
    }

    #[test]
    fn login_request_rejects_unknown_fields() {
        let json = r#"{"email":"a@example.com","master_password":"x","extra":"nope"}"#;
        assert!(serde_json::from_str::<LoginRequest>(json).is_err());
    }

    #[test]
    fn two_factor_login_request_deserializes_numeric_provider() {
        let json = r#"{"provider":1,"token":"123456"}"#;
        let parsed: TwoFactorLoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.provider, TwoFactorProvider::Email);
        assert_eq!(parsed.token, "123456");
    }

    #[test]
    fn api_key_login_body_round_trips_field_names() {
        let json = r#"{"client_id":"cid","client_secret":"secret","master_password":"hunter2"}"#;
        let parsed: ApiKeyLoginBody = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.client_id, "cid");
        assert_eq!(parsed.client_secret, "secret");
        assert_eq!(parsed.master_password, "hunter2");
    }

    #[test]
    fn auth_status_serializes_as_tagged_status_field() {
        let json = serde_json::to_string(&AuthStatus::LoggedOut).unwrap();
        assert_eq!(json, r#"{"status":"logged_out"}"#);
    }

    #[test]
    fn login_result_serializes_two_factor_required_with_providers() {
        let result = LoginResult::TwoFactorRequired {
            providers: TwoFactorProviders {
                authenticator: None,
                email: None,
                duo: None,
                organization_duo: None,
                yubi_key: None,
                remember: None,
                web_authn: None,
            },
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "two_factor_required");
        assert!(json["providers"].is_object());
    }
}
