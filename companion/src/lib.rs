//! Pure bw-CLI JSON -> `push_protocol::Credential` mapping (M1 companion).
//!
//! Deliberately separated from `main.rs`'s I/O (subprocess + HTTP) so the
//! mapping logic can be unit-tested against inline fixture JSON without a
//! real `bw` binary, a real vault, or a running device. See
//! `.planning/decisions/2026-08-12-m1-companion-bw-cli-bridge.md`.

use push_protocol::Credential;
use serde_json::Value;
use uuid::Uuid;

/// bw's `list items` schema marks logins as `type == 1` (2 = secure note,
/// 3 = card, 4 = identity). Only logins carry the `login.*` fields this
/// bridge maps.
const BW_ITEM_TYPE_LOGIN: u64 = 1;

/// Maps the raw JSON text of `bw list items` output to the wire
/// `Credential` type the device's `/api/sync` endpoint expects.
///
/// Pure function: no subprocess, no network, no filesystem. Takes the JSON
/// **text** (not a pre-parsed value) so callers — production `main.rs` and
/// tests alike — have exactly one entry point.
///
/// Non-login items (`type != 1`) are silently skipped (not an error: most
/// vaults contain notes/cards/identities that simply aren't in scope for
/// M1). Items with a malformed/non-UUID `id` are skipped with a stderr
/// warning rather than panicking — defensive, since real `bw` output always
/// has valid UUIDs, but a hand-written JSON parser should never crash on
/// unexpected input.
///
/// Field mapping (see the ADR's "Credential Mapping" section):
/// - `name`                -> `Credential.name` (missing/non-string -> `""`)
/// - `login.username`      -> `Credential.username` (`null`/missing -> `""`;
///   bw allows a login with no username, e.g. a PIN-only or note-like entry,
///   and `Credential.username` is a non-optional `String`, so empty string
///   is the natural "no username" representation on the wire)
/// - `login.password`      -> `Credential.password` (`null`/missing -> `""`,
///   same rationale as username)
/// - `login.uris[0].uri`   -> `Credential.uri` (`None` if `uris` is
///   absent/empty; additional URIs beyond the first are dropped per the
///   ADR's conscious omissions)
/// - `notes`               -> `Credential.notes` (`null`/missing -> `None`)
/// - `id`                  -> `Credential.id` (parsed via
///   `Uuid::parse_str`; unparsable -> item skipped with a warning)
///
/// Deliberately NOT mapped (per the ADR's "Conscious Omissions", not gaps):
/// `login.totp`, `reprompt`, `folderId`/`collectionIds`/`favorite`, and any
/// URI beyond the first.
#[must_use]
pub fn map_bw_items_to_credentials(bw_list_items_json: &str) -> Vec<Credential> {
    let items: Vec<Value> = match serde_json::from_str(bw_list_items_json) {
        Ok(Value::Array(items)) => items,
        Ok(_) => {
            eprintln!("warning: bw list items output was not a JSON array; no credentials mapped");
            return Vec::new();
        }
        Err(e) => {
            eprintln!("warning: failed to parse bw list items JSON: {e}");
            return Vec::new();
        }
    };

    items
        .iter()
        .filter(|item| item_type(item) == Some(BW_ITEM_TYPE_LOGIN))
        .filter_map(map_one_item)
        .collect()
}

fn item_type(item: &Value) -> Option<u64> {
    item.get("type").and_then(Value::as_u64)
}

fn map_one_item(item: &Value) -> Option<Credential> {
    let raw_id = item.get("id").and_then(Value::as_str).unwrap_or("");
    let id = match Uuid::parse_str(raw_id) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("warning: skipping bw item with unparsable id {raw_id:?}: {e}");
            return None;
        }
    };

    let name = item
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let login = item.get("login");

    let username = login
        .and_then(|l| l.get("username"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let password = login
        .and_then(|l| l.get("password"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let uri = login
        .and_then(|l| l.get("uris"))
        .and_then(Value::as_array)
        .and_then(|uris| uris.first())
        .and_then(|first| first.get("uri"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let notes = item
        .get("notes")
        .and_then(Value::as_str)
        .map(str::to_string);

    Some(Credential {
        id,
        name,
        username,
        password,
        uri,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use push_protocol::SyncRequest;

    /// Realistic `bw list items --pretty` fixture covering the three
    /// required cases:
    /// (a) a login with multiple URIs (only the first must survive)
    /// (b) a login with null username and null notes
    /// (c) a non-login item (type 2, secure note) that must be dropped
    ///
    /// Plus two extra edge cases:
    /// (d) a login with `uris: []` (present but empty) -> `uri: None`
    /// (e) a login with a malformed `id` -> skipped with a warning, not a
    ///     panic (this can't happen with real bw output, but the mapper
    ///     must not crash on it)
    const FIXTURE: &str = r#"
    [
        {
            "id": "6b1f0c2e-6b4a-4b8b-9b1a-000000000001",
            "organizationId": null,
            "folderId": null,
            "type": 1,
            "name": "GitHub",
            "notes": "personal account",
            "favorite": false,
            "login": {
                "username": "octocat@example.com",
                "password": "hunter2",
                "totp": "otpauth://totp/GitHub:octocat?secret=ABC",
                "uris": [
                    { "match": null, "uri": "https://github.com" },
                    { "match": null, "uri": "https://github.com/login" }
                ]
            },
            "reprompt": 0
        },
        {
            "id": "6b1f0c2e-6b4a-4b8b-9b1a-000000000002",
            "organizationId": null,
            "folderId": null,
            "type": 1,
            "name": "No Username Login",
            "notes": null,
            "favorite": false,
            "login": {
                "username": null,
                "password": "swordfish",
                "totp": null,
                "uris": []
            },
            "reprompt": 0
        },
        {
            "id": "6b1f0c2e-6b4a-4b8b-9b1a-000000000003",
            "organizationId": null,
            "folderId": null,
            "type": 2,
            "name": "Wifi Password Note",
            "notes": "SSID: home, PW: whatever",
            "favorite": false,
            "reprompt": 0
        },
        {
            "id": "6b1f0c2e-6b4a-4b8b-9b1a-000000000004",
            "type": 1,
            "name": "Empty Uris Array",
            "notes": null,
            "login": {
                "username": "user4",
                "password": "pw4",
                "uris": []
            }
        },
        {
            "id": "not-a-uuid",
            "type": 1,
            "name": "Malformed Id Login",
            "notes": null,
            "login": {
                "username": "user5",
                "password": "pw5",
                "uris": []
            }
        }
    ]
    "#;

    #[test]
    fn filters_out_non_login_items() {
        let creds = map_bw_items_to_credentials(FIXTURE);
        assert!(
            creds.iter().all(|c| c.name != "Wifi Password Note"),
            "secure note (type 2) must not be mapped"
        );
    }

    #[test]
    fn collapses_multiple_uris_to_first() {
        let creds = map_bw_items_to_credentials(FIXTURE);
        let github = creds
            .iter()
            .find(|c| c.name == "GitHub")
            .expect("GitHub login should be mapped");
        assert_eq!(github.uri.as_deref(), Some("https://github.com"));
        assert_eq!(github.username, "octocat@example.com");
        assert_eq!(github.password, "hunter2");
        assert_eq!(github.notes.as_deref(), Some("personal account"));
    }

    #[test]
    fn null_username_and_notes_map_to_sensible_defaults_not_panic() {
        let creds = map_bw_items_to_credentials(FIXTURE);
        let no_username = creds
            .iter()
            .find(|c| c.name == "No Username Login")
            .expect("login with null username should still be mapped");
        assert_eq!(no_username.username, "");
        assert_eq!(no_username.password, "swordfish");
        assert_eq!(no_username.notes, None);
        assert_eq!(no_username.uri, None);
    }

    #[test]
    fn empty_uris_array_maps_to_none() {
        let creds = map_bw_items_to_credentials(FIXTURE);
        let item = creds
            .iter()
            .find(|c| c.name == "Empty Uris Array")
            .expect("login with empty uris array should still be mapped");
        assert_eq!(item.uri, None);
    }

    #[test]
    fn malformed_id_is_skipped_not_panicked() {
        let creds = map_bw_items_to_credentials(FIXTURE);
        assert!(
            creds.iter().all(|c| c.name != "Malformed Id Login"),
            "item with an unparsable id must be skipped, not mapped or panicked on"
        );
    }

    #[test]
    fn maps_expected_count_of_login_items() {
        let creds = map_bw_items_to_credentials(FIXTURE);
        // 5 items in the fixture: 3 valid logins (GitHub, No Username,
        // Empty Uris) + 1 secure note (dropped, wrong type) + 1 malformed-id
        // login (dropped, bad id).
        assert_eq!(creds.len(), 3);
    }

    #[test]
    fn non_array_top_level_json_returns_empty_not_panic() {
        let creds = map_bw_items_to_credentials(r#"{"not": "an array"}"#);
        assert!(creds.is_empty());
    }

    #[test]
    fn invalid_json_returns_empty_not_panic() {
        let creds = map_bw_items_to_credentials("not json at all {{{");
        assert!(creds.is_empty());
    }

    /// Proves wire compatibility with `emulator::desktop::http_server::handle_sync`,
    /// which decodes the POST body via `ciborium::from_reader` into a
    /// `SyncRequest`. Round-trips fixture-mapped credentials through the
    /// same CBOR encode/decode path the real companion binary uses.
    #[test]
    fn sync_request_round_trips_through_cbor() {
        let credentials = map_bw_items_to_credentials(FIXTURE);
        assert_eq!(credentials.len(), 3);

        let request = SyncRequest {
            credentials,
        };

        let mut cbor_bytes = Vec::new();
        ciborium::into_writer(&request, &mut cbor_bytes).expect("CBOR encode should succeed");

        let decoded: SyncRequest =
            ciborium::from_reader(cbor_bytes.as_slice()).expect("CBOR decode should succeed");

        assert_eq!(decoded.credentials.len(), request.credentials.len());
        for (original, round_tripped) in request.credentials.iter().zip(decoded.credentials.iter()) {
            assert_eq!(original.id, round_tripped.id);
            assert_eq!(original.name, round_tripped.name);
            assert_eq!(original.username, round_tripped.username);
            assert_eq!(original.password, round_tripped.password);
            assert_eq!(original.uri, round_tripped.uri);
            assert_eq!(original.notes, round_tripped.notes);
        }
    }
}
