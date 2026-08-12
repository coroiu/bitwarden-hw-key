//! `VaultStore`: the App-owned, authoritative credential state, exposed to
//! domain widgets via `Rc<RefCell<VaultStore>>` (interior mutability, single
//! -threaded, safe in the run loop — see
//! `.planning/decisions/2026-08-12-m1-vault-store-data-ownership.md`).
//!
//! Replaces the M0 shortcut where `App::step` rebuilt the entire `Navigator`
//! whenever the vault snapshot changed (`app.rs`, pre-M1): that approach
//! would destroy a pushed detail screen mid-read once one exists. Instead,
//! `App` writes fresh sync results into this store in place, and domain
//! widgets holding a clone of the `Rc` read it live at render time — no
//! rebuild, no pop-out.
//!
//! `SyncStatus` here is deliberately the small, M1-only subset the ADR
//! calls for (`Synced`/`Error`/`Empty`), derived entirely from
//! `SyncSource::sync()`'s `Result` plus item count. Richer states
//! (`Syncing`, `Offline`, ...) are explicitly deferred and require no
//! `SyncSource` trait change — do not add them here without a design update.

use uuid::Uuid;

use crate::vault_item::VaultItem;

/// The store's derived view of "how is syncing going", computed by
/// [`VaultStore::apply_sync_ok`]/[`VaultStore::apply_sync_err`] from a
/// `SyncSource::sync()` result — never set directly by a caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    /// The last sync succeeded and produced at least one item.
    Synced,
    /// The last sync failed; the message is `Self::Error`'s `Display`
    /// rendering, captured at the point of failure.
    Error(String),
    /// The last sync succeeded but the vault has no items.
    Empty,
}

/// The authoritative in-memory credential state for the running app.
///
/// Owned by `App` as `Rc<RefCell<VaultStore>>`; domain widgets (the root
/// credential list today, a detail view in a later bead) hold clones of the
/// `Rc` and call [`VaultStore::items`]/[`VaultStore::get`]/
/// [`VaultStore::status`] at render time rather than caching a snapshot.
///
/// Only `App::step` mutates this (via `apply_sync_ok`/`apply_sync_err`);
/// widgets only ever read it. This split is what keeps the
/// `RefCell` borrow discipline simple: `App::step` takes the one
/// `borrow_mut` per frame, `render` takes read-only `borrow`s, and the two
/// phases never overlap (see the ADR's "Interior Mutability Justification").
#[derive(Debug, Default)]
pub struct VaultStore {
    items: Vec<VaultItem>,
    status: Option<SyncStatus>,
}

impl VaultStore {
    /// An empty store with no sync attempted yet. `status()` reports
    /// `None` until the first `apply_sync_ok`/`apply_sync_err` call — there
    /// is no "first-run" `SyncStatus` variant in the M1 subset, so
    /// `Option` is the honest way to represent "no sync has run yet"
    /// without inventing one.
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new(), status: None }
    }

    /// The current credential set, in sync order.
    #[must_use]
    pub fn items(&self) -> &[VaultItem] {
        &self.items
    }

    /// Looks up a credential by id. Used by a detail view to detect
    /// deletion-while-viewing: a live `get(id)` that returns `None` means
    /// the credential vanished from a later sync (see the ADR's
    /// "Deletion detection").
    #[must_use]
    pub fn get(&self, id: Uuid) -> Option<&VaultItem> {
        self.items.iter().find(|item| item.id == id)
    }

    /// The derived status of the most recent sync attempt, or `None` if no
    /// sync has run yet.
    #[must_use]
    pub fn status(&self) -> Option<&SyncStatus> {
        self.status.as_ref()
    }

    /// Applies a successful `sync()` result, deriving `Synced`/`Empty` from
    /// whether `items` is non-empty. Returns whether anything actually
    /// changed (items or status), so `App::step` can set its dirty flag
    /// only when a render would actually look different.
    pub(crate) fn apply_sync_ok(&mut self, items: Vec<VaultItem>) -> bool {
        let new_status = if items.is_empty() { SyncStatus::Empty } else { SyncStatus::Synced };
        let status_changed = self.status.as_ref() != Some(&new_status);
        let items_changed = items != self.items;

        if items_changed {
            self.items = items;
        }
        if status_changed {
            self.status = Some(new_status);
        }
        items_changed || status_changed
    }

    /// Applies a failed `sync()` result. Per the ADR, a sync error does not
    /// clear the previously known items — the last-known-good list keeps
    /// rendering, only `status()` reflects the error. Returns whether the
    /// status actually changed.
    pub(crate) fn apply_sync_err(&mut self, message: String) -> bool {
        let new_status = SyncStatus::Error(message);
        let status_changed = self.status.as_ref() != Some(&new_status);
        if status_changed {
            self.status = Some(new_status);
        }
        status_changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> VaultItem {
        VaultItem {
            id: Uuid::new_v4(),
            name: name.to_string(),
            username: format!("{name}-user"),
            password: "hunter2".to_string(),
            uri: None,
            notes: None,
        }
    }

    #[test]
    fn a_fresh_store_is_empty_with_no_status_yet() {
        let store = VaultStore::new();
        assert!(store.items().is_empty());
        assert_eq!(store.status(), None);
    }

    #[test]
    fn apply_sync_ok_with_items_reports_synced_and_signals_change() {
        let mut store = VaultStore::new();
        let changed = store.apply_sync_ok(vec![item("GitHub")]);
        assert!(changed);
        assert_eq!(store.items().len(), 1);
        assert_eq!(store.status(), Some(&SyncStatus::Synced));
    }

    #[test]
    fn apply_sync_ok_with_no_items_reports_empty() {
        let mut store = VaultStore::new();
        let changed = store.apply_sync_ok(vec![]);
        assert!(changed, "transitioning from 'no sync yet' to Empty is a change");
        assert_eq!(store.status(), Some(&SyncStatus::Empty));
    }

    #[test]
    fn apply_sync_ok_with_identical_items_reports_no_change() {
        let mut store = VaultStore::new();
        let items = vec![item("GitHub")];
        assert!(store.apply_sync_ok(items.clone()));

        let changed = store.apply_sync_ok(items);
        assert!(!changed, "re-applying the same snapshot should be a no-op");
    }

    #[test]
    fn apply_sync_err_sets_error_status_and_preserves_existing_items() {
        let mut store = VaultStore::new();
        store.apply_sync_ok(vec![item("GitHub")]);

        let changed = store.apply_sync_err("boom".to_string());
        assert!(changed);
        assert_eq!(store.status(), Some(&SyncStatus::Error("boom".to_string())));
        assert_eq!(store.items().len(), 1, "last-known-good items must survive a sync error");
    }

    #[test]
    fn apply_sync_err_with_the_same_message_reports_no_change() {
        let mut store = VaultStore::new();
        store.apply_sync_err("boom".to_string());
        let changed = store.apply_sync_err("boom".to_string());
        assert!(!changed);
    }

    #[test]
    fn get_finds_an_item_by_id_and_reports_none_once_it_is_gone() {
        let mut store = VaultStore::new();
        let github = item("GitHub");
        let id = github.id;
        store.apply_sync_ok(vec![github]);

        assert!(store.get(id).is_some());

        store.apply_sync_ok(vec![]); // deleted upstream
        assert!(store.get(id).is_none());
    }
}
