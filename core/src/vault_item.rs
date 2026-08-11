use uuid::Uuid;

/// A credential as presented to the UI/render layer.
///
/// `VaultItem` is a view-model: it's the projection any `SyncSource` yields
/// to the app core, deliberately decoupled from:
/// - the push-CBOR wire format (`Credential`/`SyncRequest`/`SyncResponse`,
///   which live in `emulator` as an implementation detail of the dev-aid
///   HTTP push protocol), and
/// - any future Bitwarden SDK type (`Cipher` etc.), should on-device SDK
///   sync ever be revived.
///
/// Field shape mirrors the former `credentials::Credential` type 1:1 for
/// now (this bead is a structural migration, not a data-model redesign);
/// future `SyncSource` work may reshape this independently of the wire
/// format on either side.
///
/// See: .planning/decisions/2026-08-11-sync-source-abstraction.md
#[derive(Debug, Clone, PartialEq)]
pub struct VaultItem {
    pub id: Uuid,
    pub name: String,
    pub username: String,
    pub password: String,
    pub uri: Option<String>,
    pub notes: Option<String>,
}
