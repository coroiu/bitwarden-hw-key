# companion

M1 companion CLI: bridges the official Bitwarden CLI (`bw`) to the device's
`/api/sync` push endpoint. See
`.planning/decisions/2026-08-12-m1-companion-bw-cli-bridge.md` for the
architecture and field-mapping decision this implements.

This file is the **runbook for a real-vault run** (produced by bead
`ai-bitwarden-hw-key-0v8.7`, which verified the pipeline end to end with a
synthetic `bw` stand-in; see the bead for that report). Everything below
was written against what was actually observed running the synthetic
version; only the `bw login`/`bw unlock` steps are real-vault-specific and
untested by that bead.

## Testing without a real vault

`tests/fixtures/fake-bw-bin/bw` is a fake `bw` executable (used by bead
`ai-bitwarden-hw-key-0v8.7` to prove the full companion -> HTTP/CBOR
`/api/sync` -> device pipeline without a real Bitwarden account). Put it
first on `PATH` and it answers `bw list items` with a canned JSON array
covering a multi-URI login, a null-username/null-notes login, and a
non-login item that must be filtered out:

```bash
export PATH="$(pwd)/companion/tests/fixtures/fake-bw-bin:$PATH"
export BW_SESSION="dummy-synthetic-session-token"
cargo run -p companion --target aarch64-apple-darwin
```

## One-time `bw` setup

1. Install the Bitwarden CLI: https://bitwarden.com/help/cli/ (e.g. `npm
   install -g @bitwarden/cli` or the standalone binary). Confirm it's on
   `PATH`: `bw --version`.
2. Point it at your Bitwarden server if not bitwarden.com (skip for the
   default cloud vault): `bw config server https://your-server.example.com`.
3. Log in once: `bw login` (prompts for email + master password, or
   `--apikey`/`--sso`, see `bw login --help`). This persists a login
   session in `bw`'s local config; you normally only do this once per
   machine.

## Every run (session unlock)

`bw list items` requires an **unlocked** session, which is separate from
being logged in and expires:

```bash
bw unlock
# prints: export BW_SESSION="...long-base64-token..."
export BW_SESSION="...long-base64-token..."   # paste the printed line
```

Without a valid `BW_SESSION`, the companion's `bw list items` subprocess
call fails and the companion surfaces one of two messages verbatim from
`companion/src/main.rs::fetch_bw_list_items`:

- **Not logged in at all**: *"The Bitwarden CLI is not logged in. Run `bw
  login` and then `bw unlock`..."*
- **Logged in but locked/expired session**: *"The Bitwarden vault appears
  to be locked. Run `bw unlock` and export the printed BW_SESSION..."*

If you see either, the fix is always the same: re-run `bw unlock` and
re-export the fresh `BW_SESSION` (sessions expire, so the old export will
stop working eventually).

## Start the emulator

```bash
cd /path/to/ai-bitwarden-hw-key
cargo run --bin desktop --target aarch64-apple-darwin
```

This opens the windowed minifb emulator (320x170, 3x scale) and starts the
HTTP server on `127.0.0.1:8080`. Leave it running.

(An agent verifying headlessly instead uses `--headless` and drives it via
`POST /api/input` plus `GET /api/screenshot`; see
`.planning/decisions/2026-08-11-three-mode-testability.md`. For a human
manual run, windowed is simplest.)

## Run the companion

In a second terminal, with `BW_SESSION` still exported from above:

```bash
cd /path/to/ai-bitwarden-hw-key
cargo run -p companion --target aarch64-apple-darwin
```

Defaults to `http://127.0.0.1:8080`; pass `--device-url <base-url>` to
target a different host/port (e.g. a real T-Embed on the LAN once M1's
hardware bring-up lands).

Expected success output:

```
Pushed N credential(s) to http://127.0.0.1:8080/api/sync
```

`N` is the count of **login-type** items in your vault (`type == 1`); notes,
cards, and identities are silently excluded by design (see the ADR's
"Conscious Omissions"). If `N` looks low, check whether most of your vault
is non-login items rather than assuming a bug; that filter is intentional
for M1.

## What to expect on-device

- The list view repopulates with your real logins: name and username on the
  left, an initial-letter color chip, position readout (e.g. "1/12") and a
  green sync-status dot in the title bar top-right.
- Rotate/arrow to move selection, press/Enter to open a credential's detail
  view: USERNAME, PASSWORD (masked, a fixed dot count rather than your real
  password length), WEBSITE (only shown if that login has a URI; only the
  **first** URI survives if you saved multiple), NOTES (only shown if you
  have notes on that item).
- Press/Enter on the PASSWORD field toggles reveal: amber cleartext plus an
  open-lock icon; press again (or navigate away) to re-mask.
- Hold/Esc/Backspace returns to the list with your selection preserved.

## Known M1 scope gaps (not bugs)

Per the ADR's "Conscious Omissions": TOTP codes, master-password reprompt,
folders/collections/favorites, and any URI beyond the first are not synced
or shown in M1. If a login you expect to see is missing entirely, check its
`type` in `bw list items`: only `type == 1` (login) items sync; secure
notes/cards/identities are out of scope for M1 by design.

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| `The Bitwarden CLI ('bw') was not found on PATH` | Install the CLI or fix `PATH` |
| `The Bitwarden CLI is not logged in` | `bw login` |
| `The Bitwarden vault appears to be locked` | `bw unlock` and re-export `BW_SESSION` |
| `Failed to reach the device at http://127.0.0.1:8080/api/sync` | Emulator isn't running, or is on a different port; start it first |
| Companion prints `Pushed 0 credential(s)` | Vault has zero login-type items, or all items failed UUID parsing (shouldn't happen with real `bw` output; see `companion/src/lib.rs` warnings on stderr if any items were skipped) |
