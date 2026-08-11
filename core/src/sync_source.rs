//! `SyncSource` trait seam: abstracts where `VaultItem`s come from, so the
//! app core doesn't need to know whether it's talking to the HTTP push
//! dev-aid, a future companion-push protocol, or (if ever revived) an
//! on-device SDK client.
//!
//! **Placeholder only.** No implementation lives here yet:
//! - `PushSyncSource` (wraps the existing HTTP+CBOR push protocol) is
//!   bead W9, not this bead.
//! - `SdkSyncSource` is DEFERRED (SDK feasibility spike returned NO-GO on
//!   xtensa) and must NOT be added — see the ADR below for the post-spike
//!   outcome.
//!
//! See: .planning/decisions/2026-08-11-sync-source-abstraction.md

use crate::vault_item::VaultItem;

/// Opaque marker returned on successful unlock. Placeholder: no real
/// unlock flow exists yet (the device does no crypto per the post-spike
/// decision), so this carries no data today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnlockToken;

pub trait SyncSource {
    type Error;

    /// Fetch the current vault (credentials and metadata) as view-model
    /// projections.
    ///
    /// # Errors
    ///
    /// Returns `Self::Error` if the vault could not be fetched (e.g. a
    /// network failure talking to the push server, once implemented).
    fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error>;

    /// Unlock/authenticate. Returns a success marker or an auth error.
    ///
    /// # Errors
    ///
    /// Returns `Self::Error` if authentication fails.
    fn unlock(&mut self, master_password: &str) -> Result<UnlockToken, Self::Error>;
}
