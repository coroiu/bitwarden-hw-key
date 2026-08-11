//! `SyncSource` trait seam: abstracts where `VaultItem`s come from, so the
//! app core doesn't need to know whether it's talking to the HTTP push
//! dev-aid, a future companion-push transport (BLE/USB), or (if ever
//! revived) an on-device SDK client.
//!
//! ## Why there is no `unlock()` here
//!
//! The originating ADR (`2026-08-11-sync-source-abstraction.md`) sketched
//! this trait with a second method, `unlock(&mut self, master_password:
//! &str) -> Result<UnlockToken, Self::Error>`, alongside `sync()`. That
//! shape assumed a source that authenticates *and* decrypts on-device
//! (i.e. an on-device SDK client deriving a vault key from a master
//! password).
//!
//! The post-spike ADR (`2026-08-11-sync-direction-companion-push.md`)
//! settled the actual near-term direction: a trusted companion app runs
//! the SDK, authenticates, and decrypts the vault; the device receives
//! already-decrypted `VaultItem`s over the push transport and does no
//! crypto, no TLS, no SDK. There is no master password on the device side
//! for `PushSyncSource` to hand to `unlock()` — the only implementation
//! that exists (or is scheduled) for this trait never has anything
//! meaningful to put in that method. Keeping it here would be a trait
//! method with exactly zero real callers and exactly zero real
//! implementations, which is the kind of speculative shape this codebase
//! is trying to avoid (see `SdkSyncSource`'s deferral, epic
//! `ai-bitwarden-hw-key-1sg`, for the same reasoning applied one level up).
//!
//! So this seam is deliberately narrower than the original ADR sketch:
//! `sync()` only. If/when epic `1sg` (private-fork on-device SDK) is
//! un-deferred, on-device authentication becomes a real concern again and
//! an `unlock`-shaped method (or a separate `AuthSource` trait, depending
//! on what that work actually needs) can be added back then, informed by
//! what the SDK integration actually requires — not guessed at now.
//!
//! **Implementations:**
//! - `PushSyncSource` (wraps the existing HTTP+CBOR push protocol) lives
//!   in the `emulator` crate (bead W9, this bead) — see
//!   `emulator::desktop::PushSyncSource`.
//! - `SdkSyncSource` is DEFERRED (SDK feasibility spike returned NO-GO on
//!   xtensa) and must NOT be added — see the ADR below for the post-spike
//!   outcome.
//!
//! See: .planning/decisions/2026-08-11-sync-source-abstraction.md
//! See: .planning/decisions/2026-08-11-sync-direction-companion-push.md

use crate::vault_item::VaultItem;

/// Platform-agnostic seam for "where do `VaultItem`s come from". The app
/// core depends on this trait, never on a concrete transport
/// (HTTP+CBOR, BLE, USB, ...) or a concrete provider (push dev-aid vs. a
/// future on-device SDK client, if that's ever revived).
pub trait SyncSource {
    type Error;

    /// Fetch the current vault (credentials and metadata) as view-model
    /// projections.
    ///
    /// For the push model, this is not a network round-trip — it's
    /// reading whatever the companion most recently pushed. Call it as
    /// often as needed (e.g. once per frame, or on a `NavIntent::Refresh`)
    /// without worrying about rate-limiting a live sync.
    ///
    /// # Errors
    ///
    /// Returns `Self::Error` if the vault could not be fetched.
    fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error>;
}
