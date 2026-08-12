//! M1 companion binary: thin I/O wrapper around the pure mapping logic in
//! `lib.rs`. Shells out to the official Bitwarden CLI (`bw`), maps its
//! output to the shared `push-protocol` wire types, and POSTs the
//! CBOR-encoded result to the device's `/api/sync` endpoint.
//!
//! This file is deliberately NOT unit tested (no live `bw`, no live device
//! in CI or on this machine) — see
//! `.planning/decisions/2026-08-12-m1-companion-bw-cli-bridge.md`. The
//! parse/filter/map logic it calls into (`companion::map_bw_items_to_credentials`)
//! is unit tested in `lib.rs` against fixture JSON.

use companion::map_bw_items_to_credentials;
use push_protocol::SyncRequest;
use std::process::Command;

const DEFAULT_DEVICE_BASE_URL: &str = "http://127.0.0.1:8080";

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let device_base_url = parse_device_base_url(&args)?;
    let sync_url = format!("{}/api/sync", device_base_url.trim_end_matches('/'));

    let bw_json = fetch_bw_list_items()?;
    let credentials = map_bw_items_to_credentials(&bw_json);
    let count = credentials.len();

    let request = SyncRequest { credentials };
    let mut cbor_bytes = Vec::new();
    ciborium::into_writer(&request, &mut cbor_bytes)
        .map_err(|e| format!("Failed to CBOR-encode the sync request: {e}"))?;

    push_to_device(&sync_url, &cbor_bytes)?;

    println!("Pushed {count} credential(s) to {sync_url}");
    Ok(())
}

/// Hand-rolled minimal CLI parsing (no clap; this tool has exactly one
/// optional flag). Supports:
///   companion [--device-url <base-url>] [sync]
/// `sync` is accepted as an optional trailing positional for symmetry with
/// the ADR's example invocation, but the companion only ever does one
/// thing (sync), so it's a no-op if present.
/// Precedence: `--device-url` flag > `DEVICE_URL` env var > built-in default.
fn parse_device_base_url(args: &[String]) -> Result<String, String> {
    let mut base_url = std::env::var("DEVICE_URL").unwrap_or_else(|_| DEFAULT_DEVICE_BASE_URL.to_string());

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--device-url" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--device-url requires a value, e.g. --device-url http://192.168.1.100:8080".to_string())?;
                base_url.clone_from(value);
                i += 2;
            }
            "sync" => {
                i += 1; // recognized no-op positional, see doc comment above
            }
            "--help" | "-h" => {
                return Err(format!(
                    "Usage: companion [--device-url <base-url>] [sync]\n\
                     Defaults to {DEFAULT_DEVICE_BASE_URL} (or the DEVICE_URL env var).\n\
                     Requires a logged-in, unlocked `bw` CLI session (BW_SESSION set)."
                ));
            }
            other => {
                return Err(format!("Unrecognized argument: {other}\nRun with --help for usage."));
            }
        }
    }

    Ok(base_url)
}

/// Shells out to `bw list items` and returns its stdout as a JSON string.
/// Distinguishes the three failure modes an operator actually hits:
/// `bw` missing from PATH, `bw` present but not logged in/unlocked, and any
/// other non-zero exit. Never panics/unwraps on subprocess output.
fn fetch_bw_list_items() -> Result<String, String> {
    let output = Command::new("bw").args(["list", "items"]).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "The Bitwarden CLI (`bw`) was not found on PATH.\n\
             Install it from https://bitwarden.com/help/cli/ and ensure `bw` is on your PATH."
                .to_string()
        } else {
            format!("Failed to run `bw list items`: {e}")
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_lower = stderr.to_lowercase();
        if stderr_lower.contains("not logged in") {
            return Err(format!(
                "The Bitwarden CLI is not logged in.\n\
                 Run `bw login` and then `bw unlock` (export the resulting BW_SESSION) before retrying.\n\
                 bw said: {}",
                stderr.trim()
            ));
        }
        if stderr_lower.contains("vault is locked") || stderr_lower.contains("session") {
            return Err(format!(
                "The Bitwarden vault appears to be locked.\n\
                 Run `bw unlock` and export the printed BW_SESSION before retrying.\n\
                 bw said: {}",
                stderr.trim()
            ));
        }
        return Err(format!(
            "`bw list items` failed (exit status {:?}): {}",
            output.status.code(),
            stderr.trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// POSTs the CBOR-encoded `SyncRequest` body to the device's `/api/sync`
/// endpoint (see `emulator::desktop::http_server::handle_sync`, which
/// decodes it with `ciborium::from_reader`).
fn push_to_device(sync_url: &str, cbor_body: &[u8]) -> Result<(), String> {
    let response = ureq::post(sync_url)
        .set("Content-Type", "application/cbor")
        .send_bytes(cbor_body)
        .map_err(|e| format!("Failed to reach the device at {sync_url}: {e}\nIs the emulator/device running and reachable?"))?;

    // The device responds with a JSON SyncResponse (status/synced/total_bytes).
    // We don't strictly need it (the credential count is already known
    // locally), but surfacing a non-2xx-shaped body would be a red flag.
    let _ = response
        .into_string()
        .map_err(|e| format!("Device response was not readable text: {e}"))?;

    Ok(())
}
