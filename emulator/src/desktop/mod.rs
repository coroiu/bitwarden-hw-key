// `input.rs` (the old `KeyCode`/`InputInterface` desktop input engine) was
// retired in W7: `platform::WindowedInput` (a real `bhk_core::platform::
// InputSource`) replaces it. `storage.rs`/`DesktopStorage` is NOT retired
// here even though it looks adjacent — it persists the HTTP push
// protocol's `Vec<Credential>` (see `http_server`), a different concern
// from `platform::FileStorage`'s opaque KV blob store, so nothing in this
// bead's platform work actually replaces it.

pub mod http_server;
pub mod push_sync_source;
pub mod storage;

pub use http_server::SyncServer;
pub use push_sync_source::PushSyncSource;
pub use storage::DesktopStorage;
