# web-companion

Local HTTP server + thin browser UI that logs into a real Bitwarden account
(via the pinned `bitwarden/sdk-internal` SDK), syncs and decrypts the vault,
and pushes the resulting credentials to a hardware key target (today: the
desktop emulator over HTTP+CBOR; BLE/USB targets are a later phase). Built
across beads `ai-bitwarden-hw-key-eml.1` through `eml.6`; this file is the
runbook produced by `eml.7`.

It replaces the earlier `companion/` crate (a CLI that bridged the `bw` CLI
tool to the device) as the path to a **real-vault** demo -- `companion/`
still exists and still works as documented in `companion/README.md`, but
`web-companion` is the one with an actual login flow (password + 2FA)
instead of shelling out to a separately-authenticated `bw` process.

## Architecture in one paragraph

`web-companion` binds `127.0.0.1:3000` (loopback only) and serves a small
vanilla-JS page (`static/index.html` + `static/app.js`) that talks to its
own `/api/*` surface, which is bearer-token-gated (the token is generated
fresh per process and injected into the page at request time -- see
`src/auth.rs`). The browser never talks to Bitwarden's servers directly or
to the device directly; `web-companion` is the one process that holds both
the unlocked SDK `Client` (in memory only, never persisted) and the
`DeviceTransport` that pushes to the emulator's `POST /api/sync`.

## What eml.7 proved automatically (no real vault involved)

An automated integration test (`tests/emulator_integration.rs`, run via
`cargo test --test emulator_integration`) proves the **server-transport ->
device** half of this pipeline end to end, with no shortcuts:

- Builds and spawns a real headless `desktop` emulator binary.
- Constructs a set of `push_protocol::Credential`s (with plaintext
  passwords) and pushes them through the REAL
  `web_companion::transport::HttpEmulatorTransport` -- the exact CBOR-over-
  HTTP client `POST /api/sync` uses in production, not a stand-in.
- Asserts the emulator's own `GET /api/status` reports the same credential
  count.
- Injects `NavIntent`s over the emulator's headless HTTP protocol (`POST
  /api/input`) to open a credential's detail view and reveal its password,
  screenshotting each step (`GET /api/screenshot`) and confirming visually
  that the pushed name/username/password actually render on the device.

A separate manual smoke-check (also read-only against the real code, not
scripted into the test suite) confirmed: `web-companion` starts, serves
`index.html` with a real per-process bearer token substituted in, and
`GET /api/devices` is reachable and correctly gated -- **without a real
login it returns `409 {"error":"vault is not unlocked"}`**, not a device
list. That 409 is by design (`src/transport_routes.rs`'s
`require_unlocked`): the device list only becomes visible after a real
`POST /api/auth/login` (or `/login-apikey`) actually unlocks a vault. This
means "confirm `/api/devices` lists the emulator" is NOT something eml.7
could fully automate without real credentials -- see the honest gap below.

**What none of the above touches:** a real Bitwarden login, a real SDK
sync/decrypt of an actual vault, or the `/api/vault/*` routes. That needs
Andreas's own Bitwarden email + master password (+ 2FA), which this bead
does not have and must not fake. That's the runbook below.

## Runbook: Andreas's real-login demo

### 1. Start the emulator

```bash
cd /path/to/ai-bitwarden-hw-key
cargo run -p emulator --bin desktop --target aarch64-apple-darwin
```

This opens the windowed minifb emulator (320x170, 3x scale) and starts its
HTTP server on `127.0.0.1:8080`. Leave it running. (An agent verifying
headlessly instead passes `-- --headless` and drives it via `POST
/api/input` / `GET /api/screenshot` -- see
`.planning/decisions/2026-08-11-three-mode-testability.md`. For your own
demo, windowed is simplest: you'll watch the device UI update live.)

### 2. Start web-companion

In a second terminal:

```bash
cd /path/to/ai-bitwarden-hw-key/web-companion
cargo run
```

This binds `127.0.0.1:3000` (loopback only) and by default targets the
emulator at `http://127.0.0.1:8080` (override with `EMULATOR_URL=<url>` if
your emulator is elsewhere).

### 3. Open the UI and log in

Open <http://127.0.0.1:3000> in a browser. Enter your Bitwarden account
email and master password and submit.

- If your account has two-step login enabled, a second screen asks for the
  verification code for whichever method(s) your account supports (see
  `src/auth_routes.rs`'s `TwoFactorProviders` -- authenticator app, email
  code, Duo, YubiKey, WebAuthn, as applicable to your account).
- On success you land on the authenticated app view: a "Vault" panel and a
  "Send to device" panel.

**Non-interactive alternative (no browser, no 2FA prompt):** if your
account has an API key configured (Settings -> Security -> Keys in the
Bitwarden web vault), you can log in via `curl` instead of the browser
form:

```bash
TOKEN=$(curl -s http://127.0.0.1:3000/ | grep -o '"[0-9a-f-]\{36\}"' | head -1 | tr -d '"')
curl -s -X POST http://127.0.0.1:3000/api/auth/login-apikey \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d "{\"client_id\":\"$BW_CLIENTID\",\"client_secret\":\"$BW_CLIENTSECRET\",\"master_password\":\"$MASTER_PASSWORD\"}"
```

Per `src/auth_routes.rs`'s module docs, this path **cannot report
two-factor-required** (a real SDK-surface limitation of `login_api_key` at
the pinned revision, not an oversight) -- if your account requires 2FA and
the API-key grant doesn't bypass it for your organization's policy, this
call will just come back as a generic authentication failure and you'll
need the browser's password+2FA flow instead.

### 4. Sync your vault, then push to the device

- Click **"Sync from Bitwarden"** in the Vault panel. This calls `POST
  /api/vault/sync`, which triggers a real SDK account sync and decrypts
  every login-type cipher server-side (non-login items -- cards, identities,
  secure notes -- are silently excluded, same scope as the retired
  `companion` CLI's M1 filter). The vault list below populates with
  name/username/URL (never passwords -- see `src/vault_routes.rs`'s
  `VaultListItem`, which structurally cannot carry one).
- Select the items you want on the device (or "Select all"), pick
  **"Desktop Emulator"** in the Target device dropdown, and click **"Sync
  to device"**. This calls `POST /api/sync`, which pushes the selected
  credentials (this time WITH passwords, CBOR-encoded) to the emulator's
  `POST /api/sync` over `HttpEmulatorTransport` -- the same client and wire
  path eml.7's automated test exercised against constructed data.

### 5. Browse the synced real credentials ON THE DEVICE

In the emulator window:

- The list view repopulates with your real logins: name and username, an
  initial-letter color chip, a position readout (e.g. "1/12"), and a sync
  status dot.
- **Rotate** (or arrow keys in the emulator) to move selection, **press**
  (or Enter) to open a credential's detail view: USERNAME, PASSWORD
  (masked), WEBSITE (only shown if that login has a URI), NOTES (only shown
  if present).
- **Press** on the PASSWORD field toggles reveal: amber cleartext plus an
  open-lock icon; press again (or navigate away) to re-mask.
- **Hold** (or Backspace/Esc) returns to the list with your selection
  preserved.

### Troubleshooting

| Symptom | Likely cause |
|---|---|
| Device dropdown is empty / sync fails with "unknown device" | The emulator isn't running, or `EMULATOR_URL` doesn't match where it's actually listening. Start it first (step 1). |
| `POST /api/sync` (device push) fails with a 502 | The emulator was reachable a moment ago but isn't now -- check its terminal for a crash, or that it wasn't closed. |
| Login fails immediately, no 2FA prompt shown | Wrong email/master password -- `POST /api/auth/login` maps any SDK login failure to a generic `401`, on purpose (never leaks *why* a login failed to the browser; see `src/auth_routes.rs`). |
| 2FA code rejected | Re-enter it -- a wrong code does NOT force you to re-enter your master password (the pending login is kept; see `src/auth_routes.rs` module docs), but there's no attempt limit or TTL either, so a stuck pending login only clears via "Log out" or restarting the server. |
| "Sync from Bitwarden" succeeds but the vault list is empty | Your account may have zero login-type items, or they all failed to decrypt/parse (unlikely) -- check the `web-companion` terminal's stderr for `web-companion: vault sync failed: ...` diagnostics (never shown to the browser, by design). |
| `GET /api/devices` (if you curl it directly) returns `409 {"error":"vault is not unlocked"}` | Expected until you've actually logged in -- the device list (like vault sync) requires `Session::Unlocked`. This is not a bug; see `src/transport_routes.rs`. |

## Testing without a real vault (what eml.7 automated)

```bash
cd web-companion
cargo test --test emulator_integration -- --nocapture
```

Builds the real `desktop` emulator binary, spawns it headless, pushes
constructed credentials through the real `HttpEmulatorTransport`, and
screenshots the device UI rendering them (list -> detail -> revealed
password -> back) to `target/eml7-integration-screenshots/`. Requires port
8080 free; refuses to touch anything already listening there (never
`pkill`s by pattern -- see repo `CLAUDE.md`).

```bash
cargo build && cargo test && cargo clippy --all-targets -- -W clippy::pedantic
```

is the standard build/test/lint gate for this crate (build from
`web-companion/`, its own nested Cargo workspace -- see this crate's
`Cargo.toml` doc comment for why).

## What remains only-Andreas-can-do

Everything in the "Runbook" section above needs Andreas's actual Bitwarden
account email, master password, and (if enabled) a live two-factor code --
none of which this bead has or should fabricate. Nobody else can complete
steps 3-5 end-to-end; eml.7's automated test proves everything downstream
of a successful login (sync -> push -> render) already works against
constructed data, so a real login is the only remaining unknown.
