//! Phase-1 server-transport -> device integration proof
//! (ai-bitwarden-hw-key-eml.7).
//!
//! What this DOES prove, end to end, with no shortcuts: a set of
//! constructed `push_protocol::Credential`s (with passwords) pushed through
//! the REAL `web_companion::transport::HttpEmulatorTransport` -- the exact
//! CBOR-over-HTTP client `POST /api/sync` uses in production, not a
//! reimplementation -- against a REAL headless `desktop` emulator binary
//! (`emulator/src/desktop/http_server.rs`'s actual `SyncServer::handle_sync`,
//! unmodified), followed by driving the on-device UI over the same headless
//! HTTP protocol (`POST /api/input`, `GET /api/screenshot`) that any agent
//! uses per `.planning/decisions/2026-08-11-three-mode-testability.md`.
//!
//! What this deliberately does NOT prove: a real Bitwarden vault login
//! (`POST /api/auth/login` -> SDK sync/decrypt -> `POST /api/vault/sync`).
//! That needs Andreas's actual Bitwarden credentials, which this bead does
//! not have and must not fake -- see `web-companion/README.md`'s runbook
//! for that Andreas-only half of the Phase-1 demo.
//!
//! # What you need to run this locally
//!
//! - The `aarch64-apple-darwin` (or your host triple) target buildable for
//!   `-p emulator --bin desktop` from the repo root (this test invokes that
//!   build itself, see `build_emulator_binary`).
//! - Port 8080 free (the emulator's fixed HTTP port,
//!   `emulator/src/main.rs`'s `SyncServer::new("127.0.0.1:8080", ...)` --
//!   not configurable). If something is already listening there (e.g. a
//!   manually-started emulator), this test refuses to touch it and fails
//!   with a clear message -- per repo `CLAUDE.md`, this test suite must
//!   never blanket-`pkill` a process it didn't spawn itself.
//!
//! Run with `cargo test --test emulator_integration -- --nocapture` to see
//! the screenshot output paths.

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use push_protocol::{Credential, SyncRequest};
use uuid::Uuid;
use web_companion::transport::{DeviceTransport, HttpEmulatorTransport};

const EMULATOR_BASE_URL: &str = "http://127.0.0.1:8080";

/// `web-companion/` sits one level under the repo root (this crate's own
/// nested `[workspace]`, see `web-companion/Cargo.toml`'s doc comment) --
/// `CARGO_MANIFEST_DIR` for THIS crate is `<repo_root>/web-companion`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("web-companion sits one level under the repo root")
        .to_path_buf()
}

/// Builds the real `desktop` binary (root workspace, host target) and
/// returns its path. Uses the `CARGO` env var cargo itself sets for child
/// processes (falling back to `"cargo"` on `PATH`) rather than hardcoding a
/// toolchain-specific path.
fn build_emulator_binary() -> PathBuf {
    let root = repo_root();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(&cargo)
        .args([
            "build",
            "-p",
            "emulator",
            "--bin",
            "desktop",
            "--target",
            "aarch64-apple-darwin",
        ])
        .current_dir(&root)
        .status()
        .expect("failed to invoke `cargo build` for the emulator binary");
    assert!(
        status.success(),
        "`cargo build -p emulator --bin desktop --target aarch64-apple-darwin` failed \
         (run it manually from the repo root to see the compiler output)"
    );
    root.join("target/aarch64-apple-darwin/debug/desktop")
}

/// Whether something is already listening on the emulator's fixed HTTP
/// port. If so, this test refuses to proceed (and NEVER kills it) -- see
/// module docs and repo `CLAUDE.md`'s "never `pkill -f desktop`" guardrail.
fn port_8080_already_bound() -> bool {
    TcpStream::connect_timeout(&"127.0.0.1:8080".parse().unwrap(), Duration::from_millis(200)).is_ok()
}

fn spawn_headless_emulator(binary: &Path, cwd: &Path) -> Child {
    Command::new(binary)
        .arg("--headless")
        .current_dir(cwd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn the headless emulator binary")
}

/// Owns the spawned emulator `Child` and its scratch working directory
/// (isolating `./data/credentials.json` / `./data/kv_store.json` from the
/// repo's own `./data/`, per `emulator::desktop::storage::DesktopStorage`
/// and `emulator::platform::FileStorage`'s CWD-relative paths). `Drop`
/// kills ONLY this exact child PID -- never a pattern-matched `pkill` -- as
/// a safety net if an assertion panics before the test's own graceful HTTP
/// shutdown runs.
struct EmulatorGuard {
    child: Child,
    cwd: PathBuf,
}

impl Drop for EmulatorGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.cwd);
    }
}

async fn wait_until_ready(client: &reqwest::Client) {
    for _ in 0..100 {
        if client
            .get(format!("{EMULATOR_BASE_URL}/api/status"))
            .send()
            .await
            .is_ok()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("emulator did not become reachable on {EMULATOR_BASE_URL} within 10s");
}

/// `POST /api/input` with `intent`'s bare-string JSON form (e.g.
/// `"Activate"`) -- see `SyncServer::handle_input`'s doc comment for why a
/// plain JSON string round-trips as a `bhk_core::input::NavIntent` unit
/// variant.
async fn inject_intent(client: &reqwest::Client, intent: &str) {
    let response = client
        .post(format!("{EMULATOR_BASE_URL}/api/input"))
        .json(&intent)
        .send()
        .await
        .unwrap_or_else(|err| panic!("POST /api/input {intent:?} failed: {err}"));
    assert!(
        response.status().is_success(),
        "POST /api/input {intent:?} returned {}",
        response.status()
    );
    // The headless render loop's frame budget is 33ms
    // (`emulator/src/main.rs::FRAME_BUDGET`); give it a few frames to drain
    // the input queue and re-render before the next screenshot/intent.
    tokio::time::sleep(Duration::from_millis(250)).await;
}

async fn capture_screenshot(client: &reqwest::Client, path: &Path) {
    let response = client
        .get(format!("{EMULATOR_BASE_URL}/api/screenshot"))
        .send()
        .await
        .expect("GET /api/screenshot request failed");
    assert!(
        response.status().is_success(),
        "GET /api/screenshot returned {}",
        response.status()
    );
    let bytes = response.bytes().await.expect("failed to read screenshot body");
    std::fs::write(path, &bytes)
        .unwrap_or_else(|err| panic!("failed to write screenshot to {}: {err}", path.display()));
    println!("wrote screenshot: {}", path.display());
}

fn sample_credentials() -> Vec<Credential> {
    vec![
        Credential {
            id: Uuid::new_v4(),
            name: "GitHub".to_string(),
            username: "octocat".to_string(),
            password: "S3cr3t-Pass!".to_string(),
            uri: Some("https://github.com".to_string()),
            notes: None,
        },
        Credential {
            id: Uuid::new_v4(),
            name: "AWS Console".to_string(),
            username: "root@example.com".to_string(),
            password: "Another$ecret9".to_string(),
            uri: Some("https://console.aws.amazon.com".to_string()),
            notes: Some("break-glass account".to_string()),
        },
        Credential {
            id: Uuid::new_v4(),
            name: "Postgres (prod)".to_string(),
            username: "svc-app".to_string(),
            password: "hunter2-hunter2".to_string(),
            uri: None,
            notes: None,
        },
    ]
}

#[tokio::test]
async fn full_pipeline_pushes_credentials_and_renders_on_the_device() {
    // Must run before ANY `reqwest::Client` is constructed in this process
    // -- see `web_companion::transport`'s `ensure_crypto_provider_installed`
    // doc comment for why (reqwest 0.13's resolved rustls feature set here
    // does not auto-install a default `CryptoProvider`). Idempotent.
    let _ = rustls::crypto::ring::default_provider().install_default();

    assert!(
        !port_8080_already_bound(),
        "port 8080 already has something listening -- this test refuses to touch an \
         emulator it didn't spawn (see repo CLAUDE.md's `pkill -f desktop` guardrail). \
         Stop whatever's there first, e.g.: curl -X POST http://127.0.0.1:8080/api/shutdown"
    );

    let binary = build_emulator_binary();

    let cwd = std::env::temp_dir().join(format!("bhk-eml7-integration-{}", std::process::id()));
    std::fs::create_dir_all(&cwd).expect("create a scratch cwd for the headless emulator");

    let child = spawn_headless_emulator(&binary, &cwd);
    let mut guard = EmulatorGuard { child, cwd: cwd.clone() };

    let http = reqwest::Client::new();
    wait_until_ready(&http).await;

    // ---- 1. Push constructed credentials through the REAL transport ----
    let credentials = sample_credentials();
    let request = SyncRequest {
        credentials: credentials.clone(),
    };

    let transport = HttpEmulatorTransport::new(EMULATOR_BASE_URL.to_string());
    let sync_response = transport
        .push(&request)
        .await
        .expect("HttpEmulatorTransport::push against the running emulator should succeed");
    assert_eq!(sync_response.status, "success");
    assert_eq!(sync_response.synced, credentials.len());

    // ---- 2. Assert the emulator's own /api/status agrees ----
    let status: serde_json::Value = http
        .get(format!("{EMULATOR_BASE_URL}/api/status"))
        .send()
        .await
        .expect("GET /api/status failed")
        .json()
        .await
        .expect("GET /api/status did not return valid JSON");
    assert_eq!(
        status["credential_count"],
        serde_json::json!(credentials.len()),
        "emulator's /api/status credential_count must match what was pushed"
    );

    // Give the headless render loop (33ms frame budget) a moment to pick
    // up the new `PushSyncSource` state before the first screenshot.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let screenshots_dir = repo_root().join("target/eml7-integration-screenshots");
    std::fs::create_dir_all(&screenshots_dir).expect("create screenshot output dir");

    // ---- 3. Screenshot the list, then navigate into a credential's detail
    // and reveal its password, screenshotting each step ----
    capture_screenshot(&http, &screenshots_dir.join("01-list.png")).await;

    inject_intent(&http, "Activate").await; // open the first credential (GitHub)
    capture_screenshot(&http, &screenshots_dir.join("02-detail-username-focused.png")).await;

    inject_intent(&http, "Next").await; // focus: Username -> Password
    inject_intent(&http, "Activate").await; // reveal the password
    capture_screenshot(&http, &screenshots_dir.join("03-detail-password-revealed.png")).await;

    inject_intent(&http, "Back").await; // back to the list
    capture_screenshot(&http, &screenshots_dir.join("04-back-to-list.png")).await;

    // ---- 4. Graceful shutdown ----
    let _ = http
        .post(format!("{EMULATOR_BASE_URL}/api/shutdown"))
        .send()
        .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    // Reap if it already exited on its own; `EmulatorGuard::drop` force-kills
    // as a fallback if it didn't (e.g. this assertion never got here).
    let _ = guard.child.try_wait();
}
