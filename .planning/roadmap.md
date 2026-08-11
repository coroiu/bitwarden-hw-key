# Project Roadmap

High-level vision and milestones for the Bitwarden Hardware Key proof-of-concept.

**Last Updated**: 2026-08-11

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

**Trust model:** the device is a **first-class Bitwarden client** that authenticates,
syncs, and decrypts on-device using the **Bitwarden Rust SDK**. This is the committed
direction. Its near-term feasibility on the ESP32-S3 (SDK footprint, async/TLS stack,
KDF cost) is unproven and gated behind an early feasibility spike. A trusted-client
push path (companion or Web Vault) is the fallback if on-device sync proves impractical.

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

### M0: Platform migration (current focus)
Rebuild the foundation for a color display + rotary encoder, hardware-portable, T-Embed first.
- Architect-led green-field design of the UI framework + app architecture (reuse vs rewrite vs OSS).
- Formalize the **three run modes**: headless (agent-driven, screenshot-inspected),
  windowed (minifb, human, no hardware), real-target (T-Embed).
- **SDK feasibility spike** (see Open Questions): can the Bitwarden Rust SDK sync +
  decrypt on the ESP32-S3, and at what unlock cost?
- *Done when:* an empty-but-real color UI shell runs and is drivable by the encoder in
  the emulator and on the T-Embed. No new product capability yet, by design.

### M1: Vault browse (portable vault, part 1)
- Credential list + detail views designed for color + rotary encoder (Uma-led design).
- Backed by the sync/storage layer (direction: direct server sync via SDK; fallback: pushed credentials).
- *Done when:* you can browse your real vault on the color device.

### M2: Type it (portable vault payoff)
- Credential output over **USB HID and/or BLE HID** (USB the likely primary demo path:
  no pairing, reliable; BLE the cable-free story).
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
1. **Bitwarden Rust SDK on ESP32-S3 (feasibility spike).** [IN PROGRESS]
   - Does the full dependency tree (async runtime, HTTP, TLS) link and fit within RAM + 8MB PSRAM?
   - `reqwest`/`tokio` on ESP-IDF, and `rustls` vs ESP-IDF-native `mbedtls`.
   - KDF cost: Argon2id memory params may exceed practical limits; PBKDF2 iteration
     time. Unlock latency is the UX risk.
   - **Mitigation to design in:** re-encode the vault key under a **session key**
     protected by a faster algorithm, so only the first unlock pays full KDF cost.
     Session-key support likely needs to be newly added (SDK and/or our layer).
   - **Decision**: [2026-08-11-sync-source-abstraction.md](./decisions/2026-08-11-sync-source-abstraction.md) defers the final choice (SDK viable or fallback) to post-spike ADR. M0 proceeds with `PushSyncSource` (fallback) while spike runs in parallel (W8).

2. **UI framework: reuse vs rewrite vs OSS.** [RESOLVED]
   - **Decision**: [2026-08-11-ui-framework-reuse-vs-rewrite.md](./decisions/2026-08-11-ui-framework-reuse-vs-rewrite.md)
   - **Verdict**: Retire both existing GUIs (`src/gui/` dead code, `src/simple_gui/` architecturally incompatible with color). Rewrite clean on `embedded-graphics` + `embedded-graphics-framebuf` + `u8g2-fonts` + `mipidsi`. Salvage concepts (navigation stack, ComponentAction, FocusEvent) only; re-implement cleanly. Layout: fixed chrome + linear stacks, no flexbox.

3. **Portability boundary: abstraction without over-engineering.** [RESOLVED]
   - **Decision**: [2026-08-11-portability-boundary-and-workspace-split.md](./decisions/2026-08-11-portability-boundary-and-workspace-split.md)
   - **Verdict**: Three-layer Cargo workspace (`core` / `firmware` / `emulator`) with compiler-enforced boundaries. Platform traits (DisplaySurface, InputSource, Clock, Storage) defined in `core`; implementations in platform-specific crates. No custom HAL re-abstraction; use esp-idf-hal and ecosystem drivers directly.

### Later
4. **Companion app.** Managing the device from a computer is easier than a rotary
   encoder (bulk edits, setup, debugging). Likely wanted eventually, but *not* the sync
   trust anchor if direct server sync works, so not prioritized immediately. Scope TBD.
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
