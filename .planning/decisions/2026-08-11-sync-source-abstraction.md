# Sync Source Abstraction and Deferred SDK Decision

**Date**: 2026-08-11
**Status**: Accepted

## Context

The roadmap commits to the **Bitwarden Rust SDK** as the trusted on-device client: authenticate, sync, and decrypt vault on the device. However, the SDK's feasibility on ESP32-S3 (footprint, async/TLS, KDF latency) is unproven and gated behind a feasibility spike (roadmap Open Question 1, scheduled as part of M0).

Delaying the final sync-source decision until the spike completes creates risk: if the codebase already assumes direct-SDK sync, discovering feasibility issues late forces painful refactoring. Conversely, fully implementing the fallback (HTTP push from Web Vault) first doubles the work if the SDK turns out viable.

## Decision

Abstract the sync data source behind a trait, allowing **PushSyncSource** (fallback, works now) and **SdkSyncSource** (research-phase, decides viability) to coexist without requiring the core app to know which is in use.

```rust
pub trait SyncSource {
    /// Fetch the current vault (credentials and metadata).
    fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error>;
    
    /// Unlock/authenticate (KDF, derive vault key, etc.).
    /// Returns a success marker or auth error.
    fn unlock(&mut self, master_password: &str) -> Result<UnlockToken, Self::Error>;
    
    type Error;
}
```

**PushSyncSource**: wraps the existing HTTP + CBOR push protocol (demoted in this design to a **dev aid and fallback**). The Web Vault (or emulator) pushes credentials to the device via the HTTP server (`POST /api/sync`). Simple, works without the SDK, used for M0 green-field UI testing.

**SdkSyncSource**: the spike research implementation (bead W8). Links the Bitwarden Rust SDK, drives server sync + decrypt on the device. Hidden behind an experimental feature flag or a separate module until spike outcome is known.

**Runtime wiring**: the app is instantiated with one of the two sources at startup (dependency injection). The choice is external to the core app logic; a config flag or environment variable selects which.

**Decision deferral**: after the spike (bead W8) completes, an ADR records the outcome and makes the final choice:
- If SDK is viable: remove `PushSyncSource`, make `SdkSyncSource` the default, record the decision.
- If SDK is infeasible or too slow: promote `PushSyncSource` to the product path, design a companion/Web Vault integration, record the rationale.

## Rationale

- **Unblocks M0 immediately**: the core app can be built and tested with `PushSyncSource` (zero SDK dependency) while the spike runs in parallel.
- **Manages risk**: if the SDK doesn't fit, the app is already working with a fallback; no need to retrofit.
- **Clean separation**: the sync abstraction decouples the app from whichever provider wins, making the choice reversible if circumstances change.
- **Preserves research integrity**: the spike can focus on technical feasibility without being pressured to force a fit.

## Alternatives Considered

- **Assume direct SDK sync from the start.** Build the app with SdkSyncSource only.
  - **Pros**: simplest initially; one code path.
  - **Cons**: if the spike fails, the codebase is unusable for M0, and retrofitting a fallback is high-friction.
  - **Verdict**: Rejected. Too risky given unproven feasibility.

- **Implement PushSyncSource fully for M1, defer SdkSyncSource.** M0 uses HTTP push; after the spike, conditionally switch.
  - **Pros**: spike has more time to mature.
  - **Cons**: tight coupling to HTTP push in M0 code means refactoring is needed if SDK becomes the path. Also extends M0 schedule if spike finishes early.
  - **Verdict**: Rejected in favor of abstraction from the start.

- **No abstraction; have two separate builds (SDK and non-SDK).** Different codebase branches or feature flags throughout.
  - **Pros**: no trait indirection.
  - **Cons**: duplicate logic, drift, hard to test both paths; scales poorly.
  - **Verdict**: Rejected. A single trait is cleaner.

## Consequences

### Positive
- M0 development is not gated on spike completion; `PushSyncSource` ships working immediately.
- If the spike succeeds, switching to `SdkSyncSource` is isolated to one dependency injection point.
- App code is agnostic to the sync provider, simplifying testing and composition.
- Fallback path (push) remains available for development, demos, and offline scenarios.

### Negative
- Introduces a trait abstraction where only one impl is used at a time (minimal overhead, but adds code).
- Spike must verify that `SdkSyncSource` can be integrated within the abstraction (unlikely to be an issue).
- Decision deferral requires discipline: the spike outcome must be recorded as a follow-up ADR, not left ambiguous.

## Transition Plan

1. **M0 (now)**: implement `SyncSource` trait; `PushSyncSource` is the default for emulator and initial hardware. `SdkSyncSource` is a research stub or behind `#[cfg(feature = "sdk-spike")]`.
2. **W8 (spike, runs in parallel)**: implement `SdkSyncSource` fully, test on ESP32-S3, measure unlock latency, document findings.
3. **Post-spike ADR**: record outcome and final choice (keep SDK or promote fallback). Remove the losing implementation.
4. **M1**: build credential browsing with the chosen sync source.

## References

- Owners: Ada (architect), Ruby (rust-embedded-supervisor)
- Related: [2026-01-22-emulator-http-protocol.md](2026-01-22-emulator-http-protocol.md) (HTTP push is now a fallback, not the product path)
- Spike: bead `ai-bitwarden-hw-key-8d7.8` (W8, SDK feasibility)
- Roadmap Open Question 1 (answered by spike; final choice to follow in post-spike ADR)
