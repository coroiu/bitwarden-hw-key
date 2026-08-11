# Sync Direction: Companion-App Push (Near-Term) and Private-Fork Deferral (Long-Term)

**Date**: 2026-08-11
**Status**: Accepted

## Context

The Bitwarden Rust SDK feasibility spike (bead `ai-bitwarden-hw-key-8d7.2`, now closed) returned a **definitive NO-GO** for on-device synchronization on the ESP32-S3 (xtensa target).

### Spike Findings

**Decisive Evidence:**
1. **ring's bundled C crypto compiles wrong-endian on xtensa**: The dependency chain `bitwarden-crypto` → `bitwarden-api-key-connector` → `rustls` → `ring` includes a C crypto library that links with incorrect endianness for xtensa. No application-side workaround exists; the fix is at the C/LLVM level and is not portable.
2. **bitwarden-crypto has no 'just KDF' seam**: The library unconditionally pulls the full stack (`reqwest`, `tokio`, `rustls`, `ring`, `mockall`) via `bitwarden-api-key-connector` dependency. No feature flag, no trait, no way to use only the KDF without hauling in HTTP/TLS/crypto infrastructure. Stripping dependencies is not feasible.
3. **Compilation made to work, but link wall is fatal**: Using workarounds (`ring`'s `less-safe-getrandom-espidf` feature, `opt-level=0` for rustls/mockall), the crates compiled. However, the linker error for the C crypto endianness is insurmountable.

**NOT Measured (no physical hardware available during spike):**
- Footprint (RAM + PSRAM usage)
- KDF latency (Argon2id is known to be slow; PBKDF2 tuning on-device is unvalidated)
- Session-key design feasibility (needed to amortize first-unlock cost; unknown if SDK supports this pattern)

**Spike artifact**: throwaway branch `bd-ai-bitwarden-hw-key-8d7.2` pushed to GitHub for reference and historical record.

## Decision

Pivot to a **two-phase sync strategy**:

### Phase 1 (Near-term, M1–M2): Companion-App Push Model

**Validated direction for credential synchronization:**

1. **Companion app runs the SDK**: A trusted companion application (desktop, mobile, or Web Vault integration) runs the full Bitwarden Rust SDK, performs authentication, syncs the vault, decrypts all credentials.
2. **Device receives push**: The companion pushes encrypted/plaintext credentials to the device over the existing `PushSyncSource` path (HTTP POST `/api/sync` on the emulator; BLE or USB transport on the real device).
3. **Trust model shift**: The **companion is the trust anchor** and first-class Bitwarden client; the **device is a secure display + HID peripheral** that trusts the companion. The device does no TLS, no HTTP, no SDK, and no cryptographic operations except HOTP/TOTP if FIDO2-era work brings it in.
4. **Unblocks M1–M2 validation**: Real Bitwarden vault credentials on real hardware, with zero embedded-crypto risk. The portable-vault concept (browse credentials, type them) is proven with real data and user ergonomics tested on the device.

### Phase 2 (Deferred, conditional, Epic `ai-bitwarden-hw-key-1sg`): Private-Fork On-Device SDK

**Only revived if the companion-push model validates the portable-vault concept.**

If M1–M2 succeeds and the business case holds, revisit on-device first-class client via a **private fork** of the Bitwarden crates (not upstream contributions). Three fronts required:

1. **Transport swap**: Fork `bitwarden-api-base`, `bitwarden-api-core`, `bitwarden-api-key-connector`. Replace `reqwest` HTTP stack with `mbedtls` or another minimal TLS provider. Feature-gate `bitwarden-api-key-connector` out of `bitwarden-crypto` so the KDF can be used standalone.
2. **Crypto endianness fix**: Patch `ring` build.rs for xtensa, OR swap `rustls` to a RustCrypto provider (rustls-pemfile + RustCrypto's AES/HMAC). Validate on xtensa LLVM.
3. **On-hardware measurement**: With a working fork, measure footprint (RAM/PSRAM), KDF latency (Argon2id vs PBKDF2), session-key overhead, and feasibility of unlocking within acceptable UX bounds (target: <3s, tunable).

**Rationale for private fork, not upstream**: This embedded impl-swap is feature-flagged, non-standard, and likely never ships in production Bitwarden. Contributing it upstream adds cost and cognitive load to the SDK team for a side project. A private fork stays out of their way while remaining portable and documented for future reference.

**Reference artifact**: throwaway spike branch (kept, linked from epic 1sg) provides baseline measurement and proof-of-concept setup.

## Rationale

- **Pragmatic validation**: Companion push unblocks M1–M2 immediately with real vault data and zero embedded-crypto risk. The portable-vault concept (the actual product value) is proven independently of whether on-device SDK is viable.
- **Deferred fork is not abandonment**: If the concept succeeds, the fork is a natural next step. If it fails (e.g., KDF cost is unacceptable), the deferral was the right call.
- **Trust model is defensible**: A companion as trust anchor is a legitimate delegation model. The device is a secured display + HID interface; it doesn't need to be a first-class SDK client to be useful (and secure).
- **Reduces technical risk**: Removes the unproven assumption (SDK on xtensa) from the critical path. Companion apps can be built in parallel without blocking the device firmware.
- **Upstream relations**: Not submitting embedded quirks/workarounds upstream keeps the SDK team's codebase clean and focused on the production product.

## Alternatives Considered

1. **Proceed with on-device fork immediately (Phase 2 now).**
   - **Pros**: One code path; matches original vision of first-class client.
   - **Cons**: High cost before validating the portable-vault concept itself; xtensa + SDK + fork complexity all in the critical path; if the UX or business case fails, the fork effort was wasted; fork maintenance burden is ongoing.
   - **Verdict**: Rejected. Companion push validates faster and cheaper. Fork is a natural next step only if validated.

2. **Attempt upstream Bitwarden SDK changes (feature flags, embedded transport swaps).**
   - **Pros**: Leverage upstream maintenance; align with Bitwarden roadmap.
   - **Cons**: Feature-flagged embedded support is not a priority for the SDK team; review/merge cycles slow iteration; xtensa quirks (ring endianness) may not be acceptable upstream (out of scope for Bitwarden's platform support). Unlikely to succeed in reasonable timeframe.
   - **Verdict**: Rejected. Private fork is simpler and respects upstream boundaries.

3. **Hardware pivot to RISC-V or ARM Cortex-M (to avoid xtensa).**
   - **Pros**: Solves the ring endianness issue; more conventional platform.
   - **Cons**: T-Embed commitment is made; RISC-V solves xtensa but not the SDK footprint, async/TLS, or KDF latency problems (those are SDK-level); requires new hardware, new toolchain setup, delays M0 by weeks.
   - **Verdict**: Rejected. Stay on T-Embed. Hardware swap doesn't fix the real blocker (SDK feasibility).

4. **Implement a lightweight on-device KDF only (no full SDK).**
   - **Pros**: Reduces footprint; focuses on the hardest part (unlock latency).
   - **Cons**: Doesn't solve the HTTP/TLS/reqwest stack; leaves credentials with no decryption (still need crypto); re-implementing the KDF or stripping Bitwarden's is high-risk.
   - **Verdict**: Rejected. Companion push is simpler and safer.

## Consequences

### Positive
- **M1–M2 unblocked**: Real vault credentials on the device within weeks, not months. Product validation (portable vault UX works) can proceed.
- **Zero embedded-crypto risk**: Device is a display + HID peripheral; the only crypto operations are in hardware peripherals (future FIDO2) or in the companion.
- **Clear trust model**: Companion is the security boundary; device is a trusted-but-constrained client. No confusion about where decryption happens.
- **Parallel work**: Companion app (desktop, mobile, Web Vault) can be developed in parallel; no blocker on device firmware.
- **Deferral is defensible**: If the concept fails, the private fork was never needed. If it succeeds, revisiting the fork is a justified next phase.

### Negative
- **Trust model shift**: The device is no longer a "first-class client" in the immediate term; it's a display peripheral. This is a reduction in autonomy (but not security, if the companion is trusted).
- **Companion dependency**: The device cannot sync independently; it requires a companion app. For M1–M2, this is acceptable (no offline use case yet). For M4 (security hardening), this constraint may need revisiting.
- **Fork maintenance**: If Phase 2 is revived, the private fork must be maintained (rebase on SDK updates, monitor for security fixes). Burden is manageable but non-zero.
- **Two sync paths**: Codebase carries both `PushSyncSource` (Phase 1) and deferred `SdkSyncSource` (Phase 2) logic. Once Phase 2 is decided, cleanup is needed.

## Transition Plan

1. **Immediate (2026-08-11)**: Close bead `8d7.2`. Create epic `ai-bitwarden-hw-key-1sg` for deferred Phase 2 (private fork). Record this ADR as the outcome.
2. **M0 onward**: `PushSyncSource` is the default and primary sync path. Device UX is validated on companion-pushed credentials.
3. **M1–M2 completion**: Portable-vault concept is validated (or rejected) with real vault data.
4. **Post-M2 decision point**: If portable vault succeeds and the business case holds, epic `1sg` is un-deferred and prioritized. If it fails or the direction shifts, close epic `1sg`.

## Related Decisions

- **[2026-08-11-sync-source-abstraction.md](2026-08-11-sync-source-abstraction.md)**: This ADR is the post-spike decision deferred in sync-source-abstraction. It resolves the final choice: `SdkSyncSource` is deferred (epic 1sg), `PushSyncSource` is the near-term product path.
- **[2026-08-11-presentation-surface-run-mode-seam.md](2026-08-11-presentation-surface-run-mode-seam.md)**: The device's presentation surface (framebuffer, input, clock) is platform-abstracted; the companion syncs over that abstraction (HTTP on emulator, BLE/USB on real hardware).

## References

- **Owners**: Andreas (decision-maker), Ada (architect), Ruby (rust-embedded-supervisor)
- **Spike artifact**: Branch `bd-ai-bitwarden-hw-key-8d7.2` (throwaway, kept for reference)
- **Spike bead**: `ai-bitwarden-hw-key-8d7.2` (closed; findings logged there)
- **Deferred epic**: `ai-bitwarden-hw-key-1sg` (private-fork on-device SDK, conditional revival)
- **Roadmap**: Updated to reflect companion-push as M1–M2 sync mechanism; on-device SDK noted as deferred long-term vision
