# M1 Companion: bw-CLI Bridge + Shared Push-Protocol Crate

**Date**: 2026-08-12
**Status**: Accepted

## Context

M1 needs REAL decrypted vault credentials on the device. Per the accepted companion-push ADR (`2026-08-11-sync-direction-companion-push.md`), a trusted host companion runs the Bitwarden SDK/auth/decrypt and pushes to the device; the device does no crypto/TLS/HTTP/SDK. The on-device SDK is a no-go, deferred to epic ai-bitwarden-hw-key-1sg.

The sync contract (`.api/sync` endpoint) already exists from M0 and carries login items (id, name, username, password, uri, notes). The challenge is how the host supplies decrypted items to the device without building a full native app or SDK integration on the host.

## Decision

### M1 Companion Architecture

The M1 companion is a small **Rust CLI utility** (workspace crate named `companion`) that bridges the official **Bitwarden CLI** (`bw`):

1. Shell out to `bw list items` and deserialize the JSON response
2. Filter to login items (type == 1)
3. Map each login to our wire `Credential` struct
4. CBOR-encode as a `SyncRequest`
5. POST to the device's existing `/api/sync` push endpoint over HTTP (or USB serial if device is headless)

### Wire Contract: Push-Protocol Crate

Extract the wire types (`Credential`, `SyncRequest`, `SyncResponse`) from `emulator/src/credentials.rs` into a new host-only crate `push-protocol`:

- **Dependencies**: serde, ciborium, uuid (CBOR serialization + UUID support)
- **No dependency on bhk-core, firmware, or emulator**: `push-protocol` is pure wire format
- **Shared by**: both emulator and companion depend on it, so the contract cannot drift
- **Workspace peer**: a fourth crate alongside `core`, `firmware`, `emulator`

### Credential Mapping: bw Item to Credential

```
bw list items (JSON login entry)
  name                -> Credential.name
  login.username      -> Credential.username
  login.password      -> Credential.password
  login.uris[0].uri   -> Credential.uri (collapse multiple URIs to first)
  notes               -> Credential.notes
  id                  -> Credential.id
```

### M1 Conscious Omissions (Not Gaps, Not TODO)

The following fields are deliberately omitted in M1 (revisited in M2+):

- `login.totp` (2-step; M2 might add TOTP display without the secret, for UX)
- `reprompt` (master password re-prompt; deferred to M3+ with auth/unlock UX)
- `folders`, `collections`, `favorite` (deferred to M2 organization tier)
- Additional URIs beyond the first (M1 uses one primary URI per credential)

These are recorded as conscious scope decisions, not code TODOs, so future reviewers understand the intent.

### Companion Workspace Placement

```
ai-bitwarden-hw-key/
  core/           (platform-agnostic app + core)
  firmware/       (esp-idf + T-Embed specific)
  emulator/       (minifb + desktop)
  companion/      (NEW: bw CLI bridge, host only)
  push-protocol/  (NEW: shared wire types)
```

The companion depends only on:
- `push-protocol` (wire types)
- `serde_json` (deserialize bw JSON)
- A blocking HTTP client such as `ureq` (POST to device)
- Standard lib

### Invocation

```bash
# Requires a logged-in, unlocked bw CLI with BW_SESSION set
bw login user@example.com  # (if not already)
bw unlock                  # (if locked; stores BW_SESSION)
export BW_SESSION="..."

# The companion pushes to the device
cargo run -p companion -- --device-url http://192.168.1.100:8080 sync
```

## Rationale

- **Fastest path to real data**: The official Bitwarden CLI already decrypts and returns JSON. No need to re-implement auth or crypto on the host.
- **Credibility**: Driving Bitwarden's own official CLI demonstrates honest integration and reduces risk of protocol drift.
- **Zero crypto on device**: The device receives plaintext over HTTP. Plaintext storage is a PoC trade-off (M4 hardening territory).
- **Durable push contract**: The `/api/sync` CBOR contract is the artifact that will be reused by M2 (re-push fresh data loop), future GUI companions, and Web Vault integration. Defining it in a shared crate ensures no slippage.
- **Minimal scope**: The companion is a small, focused tool. It is not a feature-complete Bitwarden client; it is a data bridge.

## Alternatives Considered

### (a) Bitwarden Rust SDK on the Host

Link the official Bitwarden Rust SDK directly in the companion (the SDK builds fine off-xtensa):

- **Pros**: Full SDK features, deeper integration, closer to "real" Bitwarden client behavior
- **Cons**: Over-built for M1; adds significant dependency weight; requires SDK credential management on host
- **Verdict**: Valid future option for M2+, but rejected for M1. `bw` CLI is sufficient and simpler.

### (b) Desktop GUI Companion

Build a native Qt / Tauri desktop app to display vault and select items:

- **Pros**: Polished UX, potential for long-term product
- **Cons**: Premature. Still needs (a) or bw CLI underneath. Adds scope to M1.
- **Verdict**: Deferred to M3+ (roadmap confirms). Focus on the wire bridge first.

### (c) Inline Wire Structs, No Shared Crate

Define `Credential`, `SyncRequest` separately in emulator and companion (copy-paste):

- **Pros**: Reversible PoC shortcut; no crate complexity
- **Cons**: Risk of divergence; the push contract is the **durable artifact** that M2+ depend on
- **Verdict**: Rejected. The shared crate enforces that the contract cannot drift.

## Consequences

### Positive
- M1 demo has real Bitwarden credentials flowing to the device
- Companion is small and easy to reason about
- Push contract is defined once (shared crate), preventing M2+ slippage
- No on-device crypto/SDK to maintain or audit

### Negative
- Requires pre-login and `bw unlock` on the host (not zero-friction; must detect expired/locked and print remediation)
- bw CLI is a shell dependency (unlikely in security-critical production, acceptable for PoC)
- Companion decrypts the entire vault to plaintext over localhost HTTP; DesktopStorage persists plaintext (inherent to companion-push strategy, PoC-honest, hardening tracked in M4)
- Per-sync, the companion clones the credential vec (fine for M1; a scaling smell for 500+ vaults; revisited in M2 with bulk-push or differential updates)
- The companion's bw JSON deserialization is hand-written (not SDK-backed); future bw CLI schema changes could require maintenance (low risk; bw stable for years)

## Implementation Notes

- The companion main.rs is straightforward: login check, fetch, filter, map, CBOR-encode, POST
- Error paths: bw not found, bw login expired, device unreachable, device /api/sync error (all print user-friendly messages, not panic)
- M1 companion is synchronous and single-threaded (no async); blocking HTTP is fine for PoC
- Testing: unit tests for the bw JSON deserializer; integration test via headless emulator (see `2026-08-11-three-mode-testability.md`)

## References

- Owners: Ruby (rust-embedded-supervisor, companion Rust), orchestrator (scope/contract)
- Related decisions:
  - [2026-08-11-sync-direction-companion-push.md](2026-08-11-sync-direction-companion-push.md) (companion strategy)
  - [2026-08-11-portability-boundary-and-workspace-split.md](2026-08-11-portability-boundary-and-workspace-split.md) (crate layout)
  - [2026-08-11-presentation-surface-run-mode-seam.md](2026-08-11-presentation-surface-run-mode-seam.md) (/api/sync endpoint)
- Roadmap: M1 checkpoint (list + detail views), M2 (push-fresh loop), M3+ (GUI companion)
