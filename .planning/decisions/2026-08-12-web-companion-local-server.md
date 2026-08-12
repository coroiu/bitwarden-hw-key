# Web Companion: Local Rust Server + Thin Web UI with Native SDK

**Date**: 2026-08-12
**Status**: Accepted

## Context

M1 requires real, decrypted Bitwarden vault credentials synced to the device. The companion-push model (ADR 2026-08-11-sync-direction-companion-push.md) established that a trusted host companion runs the Bitwarden SDK, decrypts, and pushes to the device. However, the specific implementation of the companion remained open.

Initial direction (bead ai-bitwarden-hw-key-eml.1) explored a **bw-CLI bridge** companion: shell out to the official Bitwarden CLI, deserialize JSON, and push to the device. This approach was proven viable and verified credentials could be synced.

However, Andreas (product) clarified that the companion should be **an interactive website** where the user can authenticate, view the vault, and control sync to the device. The question then became: should the web UI run natively in the browser (WASM SDK), or should a server own the SDK integration?

After weighing WASM-SDK-in-browser (friction with build/bindgen, Chromium-only Web BLE/Serial, no emulator path) versus a local Rust server (native SDK, simple emulator dev loop, server-owned transport), Andreas chose the **local Rust server** approach.

## Decision

The companion is a **LOCAL RUST SERVER** (axum + tokio) that natively links the Bitwarden Rust SDK, serves a thin vanilla-JavaScript web UI over `127.0.0.1`, and owns all device transport server-side. The browser is a UI client only; it performs **no Web BLE, no Web Serial, no crypto, no auth**.

### Architecture

**Server-Side (Local Rust Process)**
- Async HTTP server (axum + tokio) on `127.0.0.1` (loopback only)
- Links Bitwarden Rust SDK natively: `bitwarden-core[internal]`, `bitwarden-vault`, `bitwarden-sync`, `bitwarden-api-api`
- SDK git dependency: `bitwarden/sdk-internal` pinned to rev `99ffb6ef` (PM SDK is not on crates.io)
- Manages authentication (login_password, unlock inline, 2FA via web UI input)
- Manages vault sync and decryption
- Owns `DeviceTransport` trait implementation (pluggable, Phase-1 specific)
- Built in a **standalone nested Cargo workspace** (own `Cargo.lock`, stable toolchain) to avoid entangling the Bitwarden SDK/tokio/TLS dependency tree with the firmware's xtensa/esp-idf build
- Workspace isolation proven in bead `ai-bitwarden-hw-key-eml.1`

**Browser-Side (Vanilla JavaScript)**
- Minimal React or vanilla JS (no SDK, no Chromium-only APIs)
- Displays vault list (metadata only: name, username, URI)
- User selects items to sync
- Calls server `/api/vault` and `/api/sync` endpoints
- Bearer token authentication (random per-session, in-memory, constant-time compare, never logged)
- **Passwords never sent to browser**: the decrypted secret flows only server -> device during push

**Device Transport (Pluggable)**
- `DeviceTransport` trait: async fn send_sync(credentials) -> Result
- Phase 1: `HttpDeviceTransport` - connects to emulator over existing `/api/sync` (HTTP POST, CBOR wire format unchanged)
- Phase 2: `BleDeviceTransport` + `UsbDeviceTransport` - native BLE/USB to T-Embed (requires firmware sync handler, overlaps M2)
- Transport is injected at server startup; app doesn't care which is active

### SDK Integration

**Bitwarden Rust SDK Dependency Strategy**
- Crate: `bitwarden` from `bitwarden/sdk-internal` (git dependency)
- Pinned rev: `99ffb6ef` (commit hash, PM SDK not versioned on crates.io)
- Minimal feature set: `bitwarden-core[internal]` (includes login, auth, SDK internals) + `bitwarden-vault` (credential schema) + `bitwarden-sync` (sync operations) + `bitwarden-api-api` (API stubs for client creation)
- Build isolation: nested workspace with its own `Cargo.lock` and stable rustc (not xtensa/esp toolchain) ensures no friction with firmware builds

### Authentication & Secret Handling

**Authentication Flow**
- User provides email + master password + 2FA code (if required) via web form
- Server calls `ClientInitInput::new(user_email, master_password, None)` and `Client::new()` to set up SDK client
- 2FA response collected from browser input, passed to SDK login
- Session token issued to browser (random bearer token, in-memory, expires or locks on explicit logout)

**Secret Safety (Accepted PoC Posture)**
- Bind server to `127.0.0.1` only (no remote access)
- Per-request bearer token validation (in-memory, constant-time compare)
- **Master password never persisted**: only held during login; immediately discarded after auth
- **Decrypted vault never returned to browser**: vault list is metadata only (name, username, URI)
- **Decrypted secret flows only server -> device**: during sync push, plaintext flows over HTTP to emulator (PoC acceptable, M4 hardening territory)
- **Zeroize on lock/logout**: explicit lock command or session expiry triggers zeroize (future refinement)
- **NOT hardened or audited**: this is a PoC security posture consciously accepted by Andreas. Production hardening (TLS, key derivation, in-process isolation) is M4 work.

### Workspace Structure

```
ai-bitwarden-hw-key/
  core/                  (platform-agnostic app + core)
  firmware/              (esp-idf + T-Embed specific)
  emulator/              (minifb + desktop; depends on core)
  web-companion/         (NEW: local server + web UI)
    src/
      main.rs            (axum server, DeviceTransport injection)
      auth.rs            (SDK login/unlock)
      vault.rs           (vault sync/list)
      transport.rs       (DeviceTransport trait + HTTP/BLE/USB impls)
    web/
      index.html         (vanilla JS or minimal framework)
      app.js             (fetch /api/vault, /api/sync)
    Cargo.toml
    Cargo.lock           (isolated)
  push-protocol/         (shared wire types: Credential, SyncRequest)
  Cargo.toml             (workspace root, excludes web-companion)
```

### Invocation

```bash
# Start the companion server (dev mode, interactive login)
cargo run -p web-companion -- --device-url http://127.0.0.1:8080

# Open browser to http://127.0.0.1:8000
# Login with Bitwarden account -> view vault -> select items -> sync to device
```

## Rationale

- **Matches roadmap trust model literally**: the companion runs the full Bitwarden Rust SDK; device is a display + HID peripheral. This ADR makes that concrete.
- **Avoids WASM friction**: No WebAssembly build, bindgen, or chromium-only browser transport quirks. Native Rust SDK compiles and runs on the host with no impedance mismatch.
- **Trivial emulator dev loop**: Server -> emulator uses the existing `/api/sync` HTTP+CBOR contract (no changes to device). Developers can test sync without hardware, using the headless or windowed emulator modes.
- **Phase 2 hardware is a drop-in**: Swap `HttpDeviceTransport` to `BleDeviceTransport` at startup. Device firmware gains a BLE/USB sync handler (overlaps M2), but the server logic and web UI are unchanged. No re-architecture.
- **Interactive UX**: Users can browse the vault in a browser, select what to sync, and see device state. Aligns with Andreas's "interactive website" framing.
- **Future scalability**: If the portable-vault concept succeeds, the local-server pattern supports a full Web Vault integration (swap the thin browser UI for the production Web Vault component).

## Alternatives Considered

### (a) bw-CLI Bridge (Built, Verified, Now Superseded)
- **Execution**: Companion shells out to `bw list items`, deserializes JSON, maps to wire types, pushes via HTTP
- **Pros**: Minimal scope; leverages official CLI; proven to work
- **Cons**: Non-interactive (shell script, no UX); bw CLI is a process dependency; not the "interactive website" Andreas requested
- **Verdict**: Superseded by local server. The bw-CLI bridge served as proof-of-concept and is kept for reference (bead ai-bitwarden-hw-key-eml.1), but is not the product path forward.

### (b) Headless Native-SDK One-Shot Binary (Designed, Not Built, Now Superseded)
- **Execution**: Single-shot CLI binary that prompts for email/password, syncs vault, pushes to device, exits
- **Pros**: Minimal scope; uses SDK directly; no async/web server complexity
- **Cons**: Non-interactive (no vault browsing); limited UX; doesn't match "interactive website" requirement
- **Verdict**: Superseded. Was considered as a middle ground but doesn't deliver on the interactive UX need.

### (c) WASM SDK in Browser with Web BLE/Serial (Considered, Rejected)
- **Execution**: Compile Bitwarden SDK to WebAssembly, run in browser, use Web BLE API and Web Serial API for device transport
- **Pros**: No server needed; all logic in browser; portable across browsers (theoretically)
- **Cons**: WASM build and bindgen friction (SDK not designed for wasm); Web BLE/Serial are Chromium-only (no Safari/Firefox); no emulator transport path (Web Serial needs real USB); adds complexity to browser (crypto, auth state); SDK credentials and decrypted vault live in browser memory (higher surface)
- **Verdict**: Rejected. Friction and platform limitation outweigh benefits. Local server is simpler and less coupled.

## Consequences

### Positive
- **Interactive companion UX**: Users browse vault, select items, control sync via web UI
- **Real vault data in M1**: Credentials flow from Bitwarden SDK to device with zero on-device crypto risk
- **Unified wire contract**: Push-protocol crate (Credential, SyncRequest) is the single definition; both emulator and companion depend on it
- **Hardware-agnostic transport**: Phase 2 hardware (BLE/USB) is a DeviceTransport impl swap; no app logic changes
- **Emulator dev loop unblocked**: No hardware required to test vault sync; server -> emulator over HTTP works immediately
- **Accepted PoC security posture**: Loopback bind, bearer token, zeroization, no master-password persistence. Not hardened, but deliberately not designed out.

### Negative
- **New Rust server crate and web UI**: Additional surface to maintain (web UI not versioned, may drift from server API)
- **SDK dependency churn**: Pinning `sdk-internal@99ffb6ef` creates a moving target; API changes require updates
- **Standalone workspace complexity**: Nested `Cargo.lock` and stable toolchain add build coordination overhead (vs. single workspace)
- **Phase 2 device firmware work**: T-Embed firmware must gain a BLE/USB sync handler (new state machine, transport layer); overlaps M2 scope. Estimate: 2-3 weeks of M2 effort.
- **PoC security, not production**: Plaintext vault over localhost HTTP, plaintext on-device storage (M4 hardening territory). Not suitable for production without M4 work.
- **No offline sync**: Device must be connected to the companion to sync; no local-only or deferred-sync model yet (deferred to M4)

## Implementation Notes

- **Server main.rs**: Initialize axum router, inject DeviceTransport, create SDK Client. POST /api/login handles email+password+2FA, returns bearer token. GET /api/vault returns metadata. POST /api/sync fetches decrypted items, filters by user selection, calls transport.send_sync().
- **Error handling**: Bitwarden SDK errors (login failed, sync failed, transport timeout) are caught and returned to browser as JSON (user-friendly messages, not panic/500).
- **Testing**: Unit tests for auth and vault fetch; integration test via headless emulator (sync a real vault item to device, inspect device storage). See `2026-08-11-three-mode-testability.md` for emulator harness.
- **Web UI**: Can start as single HTML file with inline JavaScript (no build step), or Vite/SvelteKit if needed later.

## Related Decisions

- **[2026-08-11-sync-direction-companion-push.md](2026-08-11-sync-direction-companion-push.md)**: Companion-push is the M1-M2 sync strategy; this ADR specifies the companion's implementation.
- **[2026-08-12-m1-companion-bw-cli-bridge.md](2026-08-12-m1-companion-bw-cli-bridge.md)**: Superseded by this ADR; bw-CLI bridge kept as reference proof-of-concept.
- **[2026-08-11-portability-boundary-and-workspace-split.md](2026-08-11-portability-boundary-and-workspace-split.md)**: Device workspace structure; web-companion is a fourth peer crate (excluded from firmware/core/emulator).
- **[2026-08-11-presentation-surface-run-mode-seam.md](2026-08-11-presentation-surface-run-mode-seam.md)**: Device `/api/sync` endpoint and push protocol wire format (Credential, SyncRequest, SyncResponse).

## Ownership

- **Architecture**: Andreas (decision), Ada (architecture review)
- **Implementation**: Ruby (rust-embedded-supervisor), Fern (web UI frontend)
- **Testing**: Tess (emulator harness, integration tests)

## References

- **Bead ai-bitwarden-hw-key-eml (epic)**: Web-companion development. Phase 0 (SDK spike, done); Phase 1 (axum skeleton, auth, vault, sync); Phase 2 (Phase 1 device transport from HTTP to BLE/USB, overlaps M2)
- **Roadmap**: M1.5 or Web Companion milestone (before/around M2, overlaps M2 on device transport)
- **Security**: Accepted PoC posture (loopback, bearer token, no master-password persistence, zeroize on lock). Production hardening is M4 work.
