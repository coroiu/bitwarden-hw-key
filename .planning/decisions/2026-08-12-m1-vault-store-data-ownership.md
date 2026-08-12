# M1 Vault Data Ownership: Shared VaultStore Observed by Domain Widgets

**Date**: 2026-08-12
**Status**: Accepted

## Context

M0 left two deliberate shortcuts in `core/src/app.rs`:

1. **App::step rebuilds the entire Navigator on sync** (lines 92-100): Whenever vault items change, the whole screen tree is discarded and rebuilt from scratch. This is safe when there is a single static screen, but breaks with an M1 pushed detail view: rebuilding would destroy the screen and yank the user out mid-read.

2. **VaultItem-to-ListItem drops the id** (lines 18-20): The mapping does not preserve the credential id, so a detail screen cannot be built from a selected row. Building a detail view requires the id.

Both were explicitly deferred to M1 in the M0 ADRs.

M1 requires:
- A list view where the user can select a credential
- A detail view showing the full credential (name, username, password, uri, notes)
- Sync events that do NOT destroy the detail view mid-read
- Deletion detection: if a credential is deleted upstream during a live detail view, render a "gone" state
- Refresh on Back: when the user navigates back to the list, it is always current

The focus-management system (`2026-01-21-focus-management-system.md`) handles app-level focus routing and works well for a single screen. The problem is at the **data layer**: how does the app keep domain widgets (list, detail) in sync when vault data changes, without destroying the widgets?

## Decision

Introduce a shared, App-owned **VaultStore** holding the authoritative credential state, exposed to domain widgets via `Rc<RefCell<VaultStore>>` (interior mutability, single-threaded, safe in the run loop).

### VaultStore Definition

```rust
pub struct VaultStore {
    pub items: Vec<VaultItem>,
    pub sync_status: SyncStatus,
}

pub enum SyncStatus {
    Synced,
    Error(String),
    Empty,
}
```

### App Ownership and Dirty Tracking

- App owns the `VaultStore` (created at app init)
- App::step calls `sync()` and writes the result into the store on change
- App NEVER rebuilds the Navigator
- App tracks dirty with a simple `items_changed` flag

```rust
// In App::step (pseudo-code)
match self.sync_source.sync() {
    Ok(new_items) => {
        if new_items != self.store.items {
            self.store.items = new_items;
            self.store.sync_status = SyncStatus::Synced;
            self.dirty = true;  // Signal that render should run
        }
    }
    Err(e) => {
        self.store.sync_status = SyncStatus::Error(e.to_string());
    }
}
```

### Domain Widgets Read Live

Domain widgets (CredentialListView, CredentialDetailView) receive `Rc<RefCell<VaultStore>>` and read live at render time:

```rust
// In CredentialListView::render
let store = self.store.borrow();
for item in &store.items {
    // Render each item
}
```

This pattern makes sync-preservation fall out for free:

- **No pop-out**: The Navigator is never rebuilt; widgets stay mounted
- **In-place updates**: If an item's password changes, it is updated in the store; the detail view reads the new value on the next render
- **Deletion detection**: A detail view reads `store.get(id)` and renders a "gone" state if the id vanished
- **Fresh list on Back**: When the user navigates back to the list, it reads the current store.items

### Selection Tracking by Id

Selection is tracked by VaultItem id (not index), so a background sync that reorders or inserts does not jump the cursor:

```rust
pub struct CredentialListView {
    pub store: Rc<RefCell<VaultStore>>,
    pub selected_id: Option<Uuid>,  // Stable across reorders
}

// On sync that reorders items:
// The selected_id stays the same; the list re-renders with a different index
// The focus system re-resolves the id to the new index
```

### SyncStatus for M1 and Beyond

For M1, SyncStatus is simple and derived in App from sync()'s Result plus item count:

```rust
pub enum SyncStatus {
    Synced,        // Last sync succeeded; items populated
    Error(String), // Last sync failed; message included
    Empty,         // No items (synced successfully but vault is empty)
}
```

Richer states (Syncing, Offline, FirstRun) are deferred and require NO change to the SyncSource trait. The `sync() -> Result<Vec, E>` seam is preserved, so the SyncSource abstraction (`2026-08-11-sync-source-abstraction.md`) is unchanged.

### Interior Mutability Justification

`Rc<RefCell>` is single-threaded only (not Send/Sync) and is acceptable because:

- The app and emulator are single-threaded run loops
- Firmware (esp-idf) is single-core ESP32 (or ESP32-S3), also single-threaded in our use case
- App::step (borrow_mut) and render (borrow) are strictly sequential phases of the loop, never interleaved
- Refcell does not incur async overhead; it is a thin runtime borrow guard

## Rationale

- **Cohesive solution to two M0 shortcuts**: The domain-model now carries the credential id, and sync-preservation falls out for free via live reads
- **Deletion-while-viewing**: A credential deleted upstream is detected at render time (live read); the detail view can render a "gone" state instead of panicking
- **M2 push-fresh-data compatibility**: M2's background push loop can update the store, and the UI will reflect changes on the next render cycle without interrupting the user
- **Focus selection stability**: Selection by id, not index, ensures the cursor does not jump when a sync reorders items
- **Minimal Navigator changes**: The Navigator is built once; no rebuild logic needed

## Alternatives Considered

### Depth-Gated Step + Snapshot Detail Views

The architect proposed a more conservative approach:

- Keep detail screens as value snapshots (clone the Credential at detail-view entry time, already how the code works)
- Reconcile list state only at nav depth 1 (rebuild only the root screen, not the entire tree)
- Add Navigator primitives: `Navigator::replace_root()` and a ListItem selection key
- On sync, replace the root; on Back, re-resolve the list focus

**Pros of this approach**:
- No interior mutability; snapshots are immutable
- Explicit control over when the list state changes

**Cons of this approach**:
- Defers in-place credential updates (if the password changes during a detail view, the snapshot is stale; not reloaded until Back)
- Requires new Navigator primitives (more surface area to test)
- Reconciliation logic is scattered (some in App::step, some in Navigator)
- M2's push-fresh-data loop would need to invalidate snapshots explicitly (more complex)

Both proposals agreed that selection should be by id, not index. The shared-store approach was chosen for cohesion, for M2 compatibility, and because live reads naturally handle deletion-while-viewing.

### Single Mutable Store Without Rc<RefCell>

Pass a mutable reference to the store at render time:

```rust
fn render(&mut self, store: &mut VaultStore) { ... }
```

**Pros**: No Rc<RefCell> overhead; explicit mutability
**Cons**: Changes render signature across all widgets; breaks composability (can't borrow multiple widgets during a render pass); more refactoring

**Verdict**: Rejected. The single-threaded run loop makes Rc<RefCell> safe and clean.

## Consequences

### Positive
- Sync events do not destroy detail views; users are not yanked out mid-read
- Deletion detection falls out naturally (live read, missing id = gone state)
- Selection is stable across reorders (id-based, not index-based)
- M2's background push loop can update the store; UI reflects changes immediately
- Focus-management system is unchanged; works as designed

### Negative
- Introduces `Rc<RefCell>` into the core app/render data path (single-threaded only; not portable to async/WASM without rework, acceptable for embedded + emulator)
- Retires the simple app.rs rebuild pattern (lines 92-100); step becomes more subtle (dirty tracking instead of rebuild)
- A revealed password could in principle change under the user during a live sync (rare edge case: user is viewing a password, vault is pushed an update with a new password for that credential; the detail view would reflect the new password on the next render). Accepted for M1; M3+ may add optimistic locks or read-only snapshots if needed.

## Implementation Notes

- VaultStore is created in App::new() and held as `store: Rc<RefCell<VaultStore>>`
- Domain widgets receive a clone of the Rc (cheap; ref count incremented)
- App::step calls sync_source.sync() and writes to store.items if changed
- render() is unchanged; widgets call `let store = self.store.borrow()` at the start
- No new public API on the Navigator; it just stops rebuilding
- Tests: store mutation is easy to mock; snapshot tests of the store state are straightforward

## References

- Owners: Fern (fe-architect, widget composition), Ada (architect, data ownership), orchestrator (reconciliation + call)
- Related decisions:
  - [2026-01-21-focus-management-system.md](2026-01-21-focus-management-system.md) (focus routing unchanged)
  - [2026-08-11-sync-source-abstraction.md](2026-08-11-sync-source-abstraction.md) (SyncSource unchanged)
  - [2026-08-11-portability-boundary-and-workspace-split.md](2026-08-11-portability-boundary-and-workspace-split.md) (core/src/app.rs structure)
- Roadmap: M1 checkpoint (list + detail), M2 (push-fresh loop), M3+ (optimistic locks or rich SyncStatus)
