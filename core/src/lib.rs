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
//! - [`render`]: the render core (W3) — `FrameBuffer565`,
//!   `Widget`/`Action`/`FocusEvent`, `Screen`/`Navigator`, chrome layout,
//!   and the `VerticalList` widget.
//! - [`vault_store::VaultStore`] (M1): the App-owned, authoritative
//!   credential state (items + derived `SyncStatus`), shared with domain
//!   widgets via `Rc<RefCell<VaultStore>>` so a sync never has to rebuild
//!   the `Navigator` to be reflected on screen. See
//!   `.planning/decisions/2026-08-12-m1-vault-store-data-ownership.md`.
//! - [`credential_list_view::CredentialListView`] (M1, bead
//!   `ai-bitwarden-hw-key-0v8.4`): the real, `VaultStore`-backed root
//!   credential list widget — id-based selection (survives
//!   reorder/insert/delete), empty/waiting/error content states, a
//!   right-edge scrollbar, and a focused-row accent block. Replaces the
//!   M0/0v8.3 placeholder `StoreBackedCredentialList` that used to live in
//!   `app.rs`.
//! - [`app::App`]: the platform-free application state — a `Navigator`,
//!   built once, over a `VaultStore`-backed root credential list that
//!   `App::step` keeps live-updated in place.
//! - [`run::run`]: the unified, `Platform`-generic main loop (W7, this
//!   bead) that drives an `App` — the one loop shared by all three run
//!   modes (headless, windowed, real-target).
//!
//! The old GUI rendering engines (`gui`/`simple_gui`) that used to live,
//! duplicated, in `firmware` and `emulator` are retired as of this bead —
//! [`app`]/[`run`] plus the render core are their replacement, wired into
//! both binaries.
//!
//! See: .planning/decisions/2026-08-11-portability-boundary-and-workspace-split.md

pub mod app;
pub mod credential_list_view;
pub mod input;
pub mod platform;
pub mod render;
pub mod run;
pub mod sync_source;
pub mod vault_item;
pub mod vault_store;

pub use app::App;
pub use credential_list_view::CredentialListView;
pub use input::NavIntent;
pub use run::run;
pub use sync_source::SyncSource;
pub use vault_item::VaultItem;
pub use vault_store::{SyncStatus, VaultStore};
