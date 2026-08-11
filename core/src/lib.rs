//! Platform-free application core.
//!
//! This crate is the compiler-enforced portability boundary: it must never
//! depend on `esp-idf-*`, `minifb`, `tiny_http`, `ssd1306`, or any other
//! platform-specific crate (see `Cargo.toml`). `firmware` and `emulator`
//! both depend on this crate and implement its trait seams.
//!
//! Current contents:
//! - [`vault_item::VaultItem`]: the credential view-model, migrated out of
//!   the old single-crate `credentials` module.
//! - [`input`]: the frozen `NavIntent` semantic input vocabulary (W1).
//! - [`platform`]: the `DisplaySurface`/`InputSource`/`Clock`/`Storage`/
//!   `Platform` trait seams (W1), with no implementations yet.
//! - [`sync_source::SyncSource`]: the `sync() -> Vec<VaultItem>` trait
//!   seam (W9). `PushSyncSource` (the concrete impl wrapping the HTTP+CBOR
//!   push protocol) lives in `emulator::desktop`, not here — this crate
//!   only defines the seam. See the module docs for why the trait has no
//!   `unlock()`, unlike the original ADR sketch.
//! - [`render`]: the render core (W3, this bead) — `FrameBuffer565`,
//!   `Widget`/`Action`/`FocusEvent`, `Screen`/`Navigator`, chrome layout,
//!   and the `VerticalList` widget.
//!
//! The old GUI rendering engines (`gui`/`simple_gui`) are NOT here — they
//! still live, duplicated, in `firmware` and `emulator` until W7 deletes
//! them entirely (this bead builds their replacement but does not wire it
//! up to either binary yet — that's the unified main loop, also W7).
//!
//! See: .planning/decisions/2026-08-11-portability-boundary-and-workspace-split.md

pub mod input;
pub mod platform;
pub mod render;
pub mod sync_source;
pub mod vault_item;

pub use input::NavIntent;
pub use sync_source::SyncSource;
pub use vault_item::VaultItem;
