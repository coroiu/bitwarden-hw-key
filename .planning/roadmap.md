# Project Roadmap

High-level vision and milestones for the Bitwarden Hardware Key proof-of-concept.

**Last Updated**: 2026-08-12

## Project Vision

The device is a **Bitwarden hardware companion**: a pocketable device with its own
color screen that lets you browse your Bitwarden vault and type credentials into any
machine, with no phone or trusted computer required. The framing for this is the
**"portable vault, screen included."**

**Audience & ambition:** a credible, reliable *internal demo* that could plausibly
seed real Bitwarden product thinking (between "show it to colleagues" and "aspiring
real product"). It must feel real and work in front of people. Security model and
standards-compliance are forward-looking constraints we don't design ourselves out
of, not things we harden or audit yet.

**North star, sequenced:**
1. **Portable vault** first: browse credentials and type them over USB HID and/or BLE HID.
2. **FIDO2 / passkeys** second: the same device also acts as a Bitwarden-native CTAP2 authenticator.

**Trust model:** the device is a **secure display + HID peripheral** that syncs credentials
from a **trusted companion app** (desktop, mobile, or Web Vault integration). The companion
runs the full Bitwarden Rust SDK, authenticates, syncs, and decrypts the vault; it then
pushes credentials to the device over the existing push path (HTTP on emulator, BLE/USB on
real hardware). The device performs no TLS, HTTP, SDK, or cryptographic operations (except
future HOTP/TOTP for FIDO2). This pragmatic delegation model unblocks M1–M2 with real vault
data and zero embedded-crypto risk. A future **on-device first-class client** via a private
SDK fork is the long-term vision, conditional on the portable-vault concept validating and
deferred to epic `ai-bitwarden-hw-key-1sg`.

**Hardware:** the Lilygo T-Embed (ESP32-S3, 320x170 color ST7789, rotary encoder,
8MB PSRAM) is the **first concrete target, not the final form factor.** The UI and
architecture are built to be hardware-portable so they can move to other devices later.

## Platform Reset

The prior work (128x32 mono SSD1306, 3-button nav, `simple_gui`/`gui`) grew out of
early experiments into a half-baked solution. The migration to color + rotary encoder
is the opportunity to **rethink the UI framework and wider architecture from scratch.**
Ada (architect) and Fern (fe-architect) own that decision: salvage what's reusable,
rewrite clean, or adopt an existing open-source layer. Treat the existing GUI as
throwaway unless proven worth keeping.

Carries forward regardless of the GUI reset: the credential data model and the concept
of a sync/storage layer. Demoted: the local HTTP + CBOR *push* protocol (Web Vault to
emulator) was a development mechanism; with direct server sync as the direction, it
becomes a fallback/dev aid rather than the product path.

## Milestones

### M0: Platform migration (emulator milestone complete, on-hardware validation pending)

**Status (2026-08-12):** All nine workstreams W1–W9 of epic `ai-bitwarden-hw-key-8d7` completed and merged to main.

**Emulator Milestone (Done):**
- Green-field UI framework redesigned and implemented on embedded-graphics + u8g2-fonts.
- Three run modes formalized: headless (agent-driven, screenshot-inspected), windowed (minifb, human, no hardware), real-target (T-Embed).
- SDK feasibility spike completed: verdict is NO-GO for on-device Bitwarden Rust SDK (ring won't link on xtensa; bitwarden-crypto not modular). Decision: device uses companion-app push model for M0–M2 (see [2026-08-11-sync-direction-companion-push.md](./decisions/2026-08-11-sync-direction-companion-push.md)).
- Empty-but-real 320x170 color shell running, keyboard-drivable in windowed mode, fully agent-drivable + observable in headless mode via HTTP NavIntent injection and screenshot capture.
- Credential-list shell operational with selection navigation verified.

**On-Hardware Validation (In Progress):**
- T-Embed ESP32-S3 board adapter code (W6) is build-only; physical hardware verification pending arrival of the board.
- Tracked in bead `ai-bitwarden-hw-key-dvm` (on-device verification when hardware arrives).
- *Done when:* on-T-Embed hardware, the color shell runs and is drivable by the rotary encoder. M0 epic closes after this validation.

### M1: Vault browse (portable vault, part 1) [DEVICE DISPLAY COMPLETE]

**Device Display & Navigation (Complete, merged 2026-08-12):**
- Credential list view with selection navigation (tested via headless screenshot inspection)
- Credential detail view layout and design
- Color + rotary encoder UI fully implemented and verified
- All rendering, navigation, and input stacks merged to main

**Real Vault Sync (Via Web Companion Milestone, see below):**
- Device will receive real Bitwarden vault credentials via the companion-app push model
- Backed by companion-app push via `PushSyncSource` (real Bitwarden vault credentials)
- Companion app (web companion, desktop, mobile, or Web Vault) runs the SDK, decrypts, and pushes to device
- *Done when (device portion):* color display + detail views merged to main and verified headless (DONE)
- *Done when (full M1):* you can browse your real vault on the color device (pending web-companion sync)

### M1.5: Web Companion (sync credentials to device)

**Purpose:** Deliver real, decrypted Bitwarden vault credentials to the device for M1 display and M2 output.

**Architecture:** Local Rust server (axum + tokio) running the Bitwarden Rust SDK natively, serving a thin vanilla-JavaScript web UI over 127.0.0.1. Browser is UI only; server owns all device transport and secret handling.

**Phase 1 (COMPLETE, 2026-08-17):**
- Interactive web UI: login, view vault metadata, select items, sync to device
- Device transport: HTTP POST to emulator `/api/sync` (reuses existing push-protocol wire types)
- Real Bitwarden vault credentials synced and stored on emulator (headless or windowed)
- End-to-end tested: live Bitwarden login (with 2FA), sync of 24-item vault, device renders list and detail views with masked passwords
- Fixed: Bitwarden API client-version header requirement (eml.11)

**Phase 2 (Next, overlaps M2, now unblocked by hardware arrival):**
- Device transport: native BLE or USB serial to real T-Embed
- Firmware gains a sync handler (new state machine for BLE/USB push protocol)
- Web UI and server logic unchanged

**Security Posture (Accepted PoC):**
- Bind server to 127.0.0.1 only
- Per-request bearer token validation
- Master password never persisted; decrypted vault never sent to browser
- Decrypted secret flows server -> device during sync; plaintext on-device (M4 hardening territory)
- Zeroize on lock/logout

**Done when (Phase 1):** COMPLETE. Web UI + SDK integration + emulator sync verified against real Bitwarden vault.

**Done when (Phase 2):** Device transport in place and tested on T-Embed with M2 work.

**Decision**: [2026-08-12-web-companion-local-server.md](./decisions/2026-08-12-web-companion-local-server.md)

### M2: Type it (portable vault payoff)
- Credential output over **USB HID and/or BLE HID** (USB the likely primary demo path:
  no pairing, reliable; BLE the cable-free story).
- Companion app continues to push fresh vault data as needed.
- Desktop simulation of typing for the emulated run modes.
- *Done when:* unlock, browse, type a credential into a real login form, end to end. This is the demo.

### M3: FIDO2 / passkeys (second capability)
- CTAP2 over the chosen transport; PIN / user-verification UX reimagined for color + encoder.
- *Done when:* register and authenticate with a passkey on a test site.

### M4: Security & product-readiness (forward-looking)
- Encryption at rest, auto-lock, session-key unlock optimization, pairing/bonding.
- Not hardened or audited now; the goal is to not design these out.

## Open Questions & Risks

### Gating (M0)
1. **Bitwarden Rust SDK on ESP32-S3 (feasibility spike).** [RESOLVED = NO-GO]
   - **Verdict**: SDK is infeasible on xtensa. ring's bundled C crypto links wrong-endian (C/LLVM level, no app-side fix); bitwarden-crypto unconditionally pulls full reqwest/rustls/ring/mockall stack via bitwarden-api-key-connector (no 'just KDF' seam).
   - **Outcome**: Device uses `PushSyncSource` (companion-app push) for M0–M2. Companion runs the SDK on a capable platform (desktop/mobile/Web Vault).
   - **Future direction**: On-device first-class SDK client via private fork deferred to epic `ai-bitwarden-hw-key-1sg` (conditional, only revived if portable-vault validates).
   - **Decision**: [2026-08-11-sync-direction-companion-push.md](./decisions/2026-08-11-sync-direction-companion-push.md) (post-spike ADR). Related: [2026-08-11-sync-source-abstraction.md](./decisions/2026-08-11-sync-source-abstraction.md) (deferred the choice; this ADR resolves it).

2. **UI framework: reuse vs rewrite vs OSS.** [RESOLVED]
   - **Decision**: [2026-08-11-ui-framework-reuse-vs-rewrite.md](./decisions/2026-08-11-ui-framework-reuse-vs-rewrite.md)
   - **Verdict**: Retire both existing GUIs (`src/gui/` dead code, `src/simple_gui/` architecturally incompatible with color). Rewrite clean on `embedded-graphics` + `embedded-graphics-framebuf` + `u8g2-fonts` + `mipidsi`. Salvage concepts (navigation stack, ComponentAction, FocusEvent) only; re-implement cleanly. Layout: fixed chrome + linear stacks, no flexbox.

3. **Portability boundary: abstraction without over-engineering.** [RESOLVED]
   - **Decision**: [2026-08-11-portability-boundary-and-workspace-split.md](./decisions/2026-08-11-portability-boundary-and-workspace-split.md)
   - **Verdict**: Three-layer Cargo workspace (`core` / `firmware` / `emulator`) with compiler-enforced boundaries. Platform traits (DisplaySurface, InputSource, Clock, Storage) defined in `core`; implementations in platform-specific crates. No custom HAL re-abstraction; use esp-idf-hal and ecosystem drivers directly.

### For M1–M2
4. **Companion app (sync and credential push).** The companion app (desktop, mobile, or Web Vault integration) is the trust anchor: it runs the Bitwarden SDK, authenticates, syncs the vault, decrypts credentials, and pushes them to the device over the push path. Scope: initial companion implementation (CLI or simple desktop GUI for testing); full Web Vault integration may be deferred post-M2. **This is now the primary sync mechanism** (previously listed as "Later" and conditional; promoted by spike outcome).
5. **FIDO2 UX** on color + encoder: PIN entry, user verification, multi-credential
   disambiguation. The new input model changes the design space.
6. **Large vaults** (500+): browsing/search performance on-device.

## Related Decisions

ADRs in `.planning/decisions/` implementing M0 architecture:

### M0 Foundation (all Accepted as of 2026-08-11)
- **[2026-08-11 Presentation Surface and Run-Mode Seam](./decisions/2026-08-11-presentation-surface-run-mode-seam.md)**: Platform abstraction with four injected traits; Rgb565 canonical format; shared framebuffer across three modes.
- **[2026-08-11 Portability Boundary and Workspace Split](./decisions/2026-08-11-portability-boundary-and-workspace-split.md)**: Three-layer Cargo workspace (core/firmware/emulator); compiler-enforced boundary; no custom HAL re-abstraction.
- **[2026-08-11 Rotary Encoder Input Model and Navigation Intent](./decisions/2026-08-11-rotary-encoder-input-model.md)**: Semantic `NavIntent` abstraction; encoder mapping; headless input injection.
- **[2026-08-11 Sync Source Abstraction and Deferred SDK Decision](./decisions/2026-08-11-sync-source-abstraction.md)**: `SyncSource` trait decouples app from sync provider; M0 uses fallback push; spike (W8) decides SDK viability.
- **[2026-08-11 UI Framework: Retire Both Existing GUIs, Rewrite Clean](./decisions/2026-08-11-ui-framework-reuse-vs-rewrite.md)**: Deprecate `src/gui/` and `src/simple_gui/`; rewrite on embedded-graphics; retain concepts only.
- **[2026-08-11 Three Run Modes for Agent-Testable Development](./decisions/2026-08-11-three-mode-testability.md)** (Accepted): Formalized via the five decisions above.

### Prior Decisions (still relevant)
- **[2026-01-22 Keyboard Emulation First](./decisions/2026-01-22-keyboard-emulation-first.md)** (Accepted): still valid as sequencing (vault before FIDO2); transport now includes USB HID.
- **[2026-01-22 Emulator HTTP Protocol](./decisions/2026-01-22-emulator-http-protocol.md)** (Superseded): demoted to a dev/fallback push mechanism (now `PushSyncSource`), not the product sync path.
- **[2026-01-21 Focus Management System](./decisions/2026-01-21-focus-management-system.md)** (Accepted*): high-level design (FocusEvent, opt-in focusability, auto-scroll) retained; transport layer superseded by NavIntent.
- **[2026-01-21 Desktop Emulation for Rapid Development](./decisions/2026-01-21-desktop-emulation.md)** (Accepted): still valid; emulator is one of three run modes.

## Notes
- Proof-of-concept focused on **validation**, not perfection.
- FIDO2 (M3) is gated on the portable-vault experience (M1-M2) proving the hardware UX.
- Update this roadmap when direction or priority changes; record hard-to-reverse choices as ADRs.
