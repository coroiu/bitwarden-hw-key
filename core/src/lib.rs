//! Platform-free application core.
//!
//! This crate is the compiler-enforced portability boundary: it must never
//! depend on `esp-idf-*`, `minifb`, `tiny_http`, `ssd1306`, or any other
//! platform-specific crate (see `Cargo.toml`). `firmware` and `emulator`
//! both depend on this crate and implement its trait seams.
//!
//! Current contents are intentionally minimal (W2, this bead):
//! - [`vault_item::VaultItem`]: the credential view-model, migrated out of
//!   the old single-crate `credentials` module.
//! - [`input`], [`platform`], [`sync_source`]: placeholder trait/enum
//!   seams per the accepted ADRs, with no implementations yet.
//!
//! The old GUI rendering engines (`gui`/`simple_gui`) are NOT here — they
//! still live, duplicated, in `firmware` and `emulator` until W3 replaces
//! them with a real render core (built here) and W7 deletes the old
//! engines entirely.
//!
//! See: .planning/decisions/2026-08-11-portability-boundary-and-workspace-split.md

pub mod input;
pub mod platform;
pub mod sync_source;
pub mod vault_item;

pub use input::NavIntent;
pub use sync_source::{SyncSource, UnlockToken};
pub use vault_item::VaultItem;
