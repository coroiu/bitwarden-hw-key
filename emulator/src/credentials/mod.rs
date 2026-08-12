//! The `bhk_core::VaultItem` conversion boundary for the push-protocol wire
//! types (`push_protocol::Credential`, see the `push-protocol` crate). This
//! impl deliberately lives here rather than in `push-protocol` itself:
//! `push-protocol` is a pure wire crate with no `bhk-core` dependency
//! (shared, unmodified, with the future companion app), while `emulator` is
//! the one crate that sees both `push-protocol` and `bhk-core` and can
//! bridge them.
//!
//! This can't be a `std::convert::From`/`Into` impl: with `Credential` now
//! defined in `push-protocol` and `VaultItem` in `bhk-core`, both types are
//! foreign to `emulator`, so `impl From<Credential> for VaultItem` here
//! would violate Rust's orphan rule (a foreign trait needs at least one
//! locally-defined type in the impl). `ToVaultItem` below is a trait
//! defined *in this crate*, so implementing it for the foreign `Credential`
//! type is legal.

use bhk_core::VaultItem;
use push_protocol::Credential;

pub trait ToVaultItem {
    fn to_vault_item(&self) -> VaultItem;
}

impl ToVaultItem for Credential {
    fn to_vault_item(&self) -> VaultItem {
        VaultItem {
            id: self.id,
            name: self.name.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            uri: self.uri.clone(),
            notes: self.notes.clone(),
        }
    }
}
