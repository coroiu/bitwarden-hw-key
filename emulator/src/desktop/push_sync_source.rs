//! `PushSyncSource`: the `bhk_core::SyncSource` implementation for the
//! companion-push model (see
//! `.planning/decisions/2026-08-11-sync-direction-companion-push.md`).
//!
//! This wraps the `Arc<Mutex<Vec<Credential>>>` that `http_server::SyncServer`
//! feeds from `POST /api/sync` (CBOR). It does not talk to the network or
//! storage itself — `SyncServer` already owns that job and hands out a
//! shared handle via `get_credentials_ref()`. `PushSyncSource` is a thin
//! adapter that turns "give me whatever the companion most recently
//! pushed" into the `SyncSource::sync() -> Vec<VaultItem>` shape the app
//! core expects, doing the `Credential` -> `VaultItem` conversion
//! (`From<&Credential> for VaultItem`, defined in `crate::credentials`) at
//! the boundary.
//!
//! `sync()` never actually fails today (reading a shared `Vec` behind a
//! `Mutex` has no failure mode other than a poisoned lock, which would
//! indicate a prior panic elsewhere and is not something this type can
//! meaningfully recover from), so `Error = Infallible`.

use crate::credentials::Credential;
use bhk_core::{SyncSource, VaultItem};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

pub struct PushSyncSource {
    credentials: Arc<Mutex<Vec<Credential>>>,
}

impl PushSyncSource {
    /// Wrap the shared credential handle a `SyncServer` hands out via
    /// `get_credentials_ref()`. Cloning the `Arc` here means the
    /// `PushSyncSource` always sees the latest pushed credentials, even
    /// ones that arrive after construction (the HTTP server thread writes
    /// into the same `Mutex` concurrently).
    #[must_use]
    pub fn new(credentials: Arc<Mutex<Vec<Credential>>>) -> Self {
        Self { credentials }
    }
}

impl SyncSource for PushSyncSource {
    type Error = Infallible;

    fn sync(&mut self) -> Result<Vec<VaultItem>, Self::Error> {
        let credentials = self.credentials.lock().unwrap();
        Ok(credentials.iter().map(VaultItem::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credential(name: &str) -> Credential {
        Credential {
            id: uuid::Uuid::new_v4(),
            name: name.to_string(),
            username: "user@example.com".to_string(),
            password: "hunter2".to_string(),
            uri: Some("https://example.com".to_string()),
            notes: None,
        }
    }

    #[test]
    fn sync_returns_empty_vec_when_nothing_has_been_pushed_yet() {
        let mut source = PushSyncSource::new(Arc::new(Mutex::new(Vec::new())));

        let items = source.sync().unwrap();

        assert!(items.is_empty());
    }

    #[test]
    fn sync_returns_the_credentials_currently_held_by_the_shared_handle() {
        let shared = Arc::new(Mutex::new(vec![credential("GitHub"), credential("Gmail")]));
        let mut source = PushSyncSource::new(shared);

        let items = source.sync().unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "GitHub");
        assert_eq!(items[1].name, "Gmail");
    }

    #[test]
    fn sync_reflects_credentials_pushed_after_construction() {
        // This is the whole point of holding an `Arc` rather than a
        // snapshot `Vec`: the HTTP server thread mutates the shared
        // `Mutex` concurrently as new pushes land, and `PushSyncSource`
        // must see those updates on the next `sync()` call without being
        // reconstructed.
        let shared = Arc::new(Mutex::new(Vec::new()));
        let mut source = PushSyncSource::new(shared.clone());

        assert!(source.sync().unwrap().is_empty());

        shared.lock().unwrap().push(credential("Newly Pushed"));

        let items = source.sync().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Newly Pushed");
    }

    #[test]
    fn sync_maps_all_credential_fields_onto_the_vault_item_view_model() {
        let cred = Credential {
            id: uuid::Uuid::new_v4(),
            name: "GitHub".to_string(),
            username: "octocat".to_string(),
            password: "s3cr3t".to_string(),
            uri: Some("https://github.com".to_string()),
            notes: Some("work account".to_string()),
        };
        let expected_id = cred.id;
        let mut source = PushSyncSource::new(Arc::new(Mutex::new(vec![cred])));

        let items = source.sync().unwrap();

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.id, expected_id);
        assert_eq!(item.name, "GitHub");
        assert_eq!(item.username, "octocat");
        assert_eq!(item.password, "s3cr3t");
        assert_eq!(item.uri, Some("https://github.com".to_string()));
        assert_eq!(item.notes, Some("work account".to_string()));
    }
}
