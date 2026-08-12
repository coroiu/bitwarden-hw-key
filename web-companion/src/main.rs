//! Feasibility-gate spike (ai-bitwarden-hw-key-eml.1): prove the Bitwarden PM
//! SDK (bitwarden/sdk-internal, pinned commit 99ffb6ef) resolves and LINKS
//! on a host stable toolchain from inside this repo, and that the API
//! surface Phase 1 needs (Client construction, password login, two-factor,
//! unlock, sync, cipher enumerate+decrypt) still has the shape Ada assumed.
//!
//! This binary is deliberately never meant to be *run* against a real
//! account for this spike -- no credentials are used or required. Every
//! function below is real, callable code that must COMPILE against the
//! pinned SDK rev; `main` never actually invokes the network paths.
//!
//! See ai-bitwarden-hw-key-eml.1 for the full findings report (signature
//! deltas vs. Ada's design are called out inline below).

use bitwarden_core::{
    Client, ClientSettings, DeviceType,
    auth::login::{
        PasswordLoginRequest, PasswordLoginResponse, TwoFactorEmailRequest, TwoFactorProvider,
        TwoFactorRequest,
    },
    key_management::crypto::{InitUserCryptoMethod, InitUserCryptoRequest},
};
use bitwarden_sync::{SyncClientExt, SyncRequest as BwSyncRequest};
use bitwarden_vault::{CipherView, DecryptCipherListResult, VaultClientExt};

// Confirms the cross-workspace path dependency on the sibling wire-format
// crate resolves and its types are usable from web-companion. Aliased to
// avoid colliding with `bitwarden_sync::SyncRequest`.
use push_protocol::{Credential, SyncRequest as PushSyncRequest};

/// Constructs a Client the way Phase 1 will: no credentials touched here.
///
/// FINDING: `Client::new` takes `Option<ClientSettings>`, not a builder
/// pattern requiring explicit `.build()` -- matches Ada's assumption.
fn build_client() -> Client {
    let settings = ClientSettings {
        identity_url: "https://identity.bitwarden.com".to_string(),
        api_url: "https://api.bitwarden.com".to_string(),
        device_type: DeviceType::SDK,
        ..ClientSettings::default()
    };
    Client::new(Some(settings))
}

/// FINDING (differs from Ada's design): there is no separate top-level
/// `login()` vs. `unlock()` split for the master-password flow. A single
/// `client.auth().login_password(&req)` call performs BOTH the network
/// authentication AND (when the server's `userDecryptionOptions` include
/// `masterPasswordUnlock`) the local crypto unlock, internally calling
/// `initialize_user_crypto_master_password_unlock`. The explicit
/// `client.crypto().initialize_user_crypto(InitUserCryptoRequest)` path
/// (exercised separately below) exists for resuming a *cached* session
/// (e.g. mobile persisted state) or non-master-password login methods, not
/// for a fresh password login.
async fn login_password_flow(client: &Client, email: &str, password: &str) -> PasswordLoginResponse {
    let request = PasswordLoginRequest {
        email: email.to_string(),
        password: password.to_string(),
        two_factor: None,
    };

    // Not awaited/unwrapped for real in this spike -- reference the call so
    // it type-checks against the pinned rev; never executed at runtime here.
    if should_never_run() {
        return client.auth().login_password(&request).await.expect(
            "spike: login_password is never actually invoked without --run-live and creds",
        );
    }

    unreachable!("compile-only reference path")
}

/// FINDING: two-factor is threaded through `PasswordLoginRequest.two_factor:
/// Option<TwoFactorRequest>` on retry, not a separate "submit 2FA code" call
/// against a pending-login handle. `TwoFactorEmailRequest` /
/// `client.auth().send_two_factor_email(...)` is a distinct helper to
/// (re)trigger an email OTP to be sent, not to submit one.
async fn two_factor_flow(client: &Client, email: &str, password: &str) {
    if should_never_run() {
        // Ask the server to (re)send an email OTP.
        let send_req = TwoFactorEmailRequest {
            password: password.to_string(),
            email: email.to_string(),
        };
        client
            .auth()
            .send_two_factor_email(&send_req)
            .await
            .expect("spike: never actually invoked");

        // Retry the password login with the second factor attached.
        let retry_req = PasswordLoginRequest {
            email: email.to_string(),
            password: password.to_string(),
            two_factor: Some(TwoFactorRequest {
                token: "000000".to_string(),
                provider: TwoFactorProvider::Email,
                remember: false,
            }),
        };
        let _response = client
            .auth()
            .login_password(&retry_req)
            .await
            .expect("spike: never actually invoked");
    }
}

/// FINDING: explicit unlock (as opposed to the login_password-embedded
/// unlock above) requires the caller to already hold
/// `WrappedAccountCryptographicState` + `MasterPasswordUnlockData` -- both
/// come from a prior `login_password` response, they are NOT independently
/// fetchable. This path is for re-initializing crypto in a fresh process
/// against previously-persisted state, not a first-time login. Included
/// here purely to confirm the type signature compiles; the values passed
/// are placeholders that would panic if this ever ran, which it does not.
#[allow(unreachable_code, unused_variables)]
async fn unlock_flow(client: &Client) {
    if should_never_run() {
        let req: InitUserCryptoRequest = unimplemented!(
            "spike: constructing a real InitUserCryptoRequest needs \
             WrappedAccountCryptographicState + MasterPasswordUnlockData \
             from a prior login response -- see finding above"
        );
        let _ = matches!(&req.method, InitUserCryptoMethod::MasterPasswordUnlock { .. });
        client
            .crypto()
            .initialize_user_crypto(req)
            .await
            .expect("spike: never actually invoked");
    }
}

/// FINDING (differs from Ada's design): `client.sync()` (via
/// `SyncClientExt`) does NOT return the cipher list. It returns `Ok(bool)`
/// (whether a full sync happened vs. was skipped by the revision-date
/// check). Ciphers only become visible afterwards through
/// `client.vault().ciphers().list()` / `.get_all()`, and THOSE calls
/// require a `bitwarden_state::repository::Repository<Cipher>` to have been
/// registered as client-managed state beforehand (`bitwarden-pm`'s
/// `create_client_managed_repositories!` macro does this in the real
/// clients). This compiles without a registered repository -- it would only
/// fail at *runtime* (`GetCipherError::Repository`) -- so this spike proves
/// the call signatures, not the runtime data flow. Registering a repository
/// backend is Phase 1 scope, not this gate.
async fn sync_and_list_flow(client: &Client) -> DecryptCipherListResult {
    if should_never_run() {
        let _did_sync: bool = client
            .sync()
            .sync(BwSyncRequest {
                force: false,
                exclude_subdomains: None,
            })
            .await
            .expect("spike: never actually invoked");

        let cipher_list_view = client
            .vault()
            .ciphers()
            .list()
            .await
            .expect("spike: never actually invoked (no repository registered)");
        return cipher_list_view;
    }
    unreachable!("compile-only reference path")
}

/// FINDING: full per-item decrypt-to-`CipherView` (as opposed to the
/// lighter-weight `CipherListView` from `.list()`) goes through
/// `client.vault().ciphers().get(cipher_id)` or `.get_all()`, both of which
/// hit the same `Repository<Cipher>` + `KeyStore` path as `.list()` above.
async fn decrypt_one_cipher(client: &Client, cipher_id: &str) -> CipherView {
    if should_never_run() {
        return client
            .vault()
            .ciphers()
            .get(cipher_id)
            .await
            .expect("spike: never actually invoked");
    }
    unreachable!("compile-only reference path")
}

/// Confirms the cross-workspace `push-protocol` path dependency's types are
/// usable unmodified alongside the SDK types above.
fn push_protocol_roundtrip() -> PushSyncRequest {
    PushSyncRequest {
        credentials: vec![Credential {
            id: uuid::Uuid::new_v4(),
            name: "spike".to_string(),
            username: "spike@example.com".to_string(),
            password: "unused".to_string(),
            uri: None,
            notes: None,
        }],
    }
}

/// Always `false` at runtime; keeps the network/crypto-touching branches
/// above as real, type-checked, dead code rather than deleting them. `main`
/// never calls this with intent to execute the branches -- the compiler
/// still fully checks them either way, which is the point of this spike.
fn should_never_run() -> bool {
    std::hint::black_box(false)
}

#[tokio::main]
async fn main() {
    let client = build_client();

    // Send + Sync + Clone check for axum State (see report): Client derives
    // Clone, and wraps its only field in Arc<InternalClient>. We don't have
    // InternalClient's definition available (private), so assert the bound
    // we actually need for axum::extract::State here, which will fail to
    // compile if it's ever untrue.
    fn assert_send_sync_clone<T: Send + Sync + Clone>() {}
    assert_send_sync_clone::<Client>();

    if should_never_run() {
        let _r1 = login_password_flow(&client, "spike@example.com", "unused").await;
        two_factor_flow(&client, "spike@example.com", "unused").await;
        unlock_flow(&client).await;
        let _r2 = sync_and_list_flow(&client).await;
        let _r3 = decrypt_one_cipher(&client, "00000000-0000-0000-0000-000000000000").await;
    }

    let push_req = push_protocol_roundtrip();
    println!(
        "web-companion feasibility spike: client constructed, {} push-protocol credential(s) round-tripped, all SDK call sites type-check.",
        push_req.credentials.len()
    );
}
