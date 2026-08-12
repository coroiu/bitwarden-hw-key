# Decision Log Index

Quick reference for all architectural and technical decisions made in this project.

## How to Use This Index

1. Scan the table below to find relevant decisions
2. Click the filename link to read the full decision document
3. When adding a new decision, add a row to this table in chronological order

## Decisions

| Date | Title | Status | File |
|------|-------|--------|------|
| 2026-08-12 | Web Companion: Local Rust Server + Thin Web UI with Native SDK | Accepted | [2026-08-12-web-companion-local-server.md](2026-08-12-web-companion-local-server.md) |
| 2026-08-12 | M1 Vault Data Ownership: Shared VaultStore Observed by Domain Widgets | Accepted | [2026-08-12-m1-vault-store-data-ownership.md](2026-08-12-m1-vault-store-data-ownership.md) |
| 2026-08-12 | M1 Companion: bw-CLI Bridge + Shared Push-Protocol Crate | Superseded | [2026-08-12-m1-companion-bw-cli-bridge.md](2026-08-12-m1-companion-bw-cli-bridge.md) |
| 2026-08-11 | Sync Direction: Companion-App Push (Near-Term) and Private-Fork Deferral (Long-Term) | Accepted | [2026-08-11-sync-direction-companion-push.md](2026-08-11-sync-direction-companion-push.md) |
| 2026-08-11 | UI Framework: Retire Both Existing GUIs, Rewrite Clean on Embedded-Graphics | Accepted | [2026-08-11-ui-framework-reuse-vs-rewrite.md](2026-08-11-ui-framework-reuse-vs-rewrite.md) |
| 2026-08-11 | Sync Source Abstraction and Deferred SDK Decision | Accepted | [2026-08-11-sync-source-abstraction.md](2026-08-11-sync-source-abstraction.md) |
| 2026-08-11 | Rotary Encoder Input Model and Navigation Intent (amended 2026-08-12) | Accepted | [2026-08-11-rotary-encoder-input-model.md](2026-08-11-rotary-encoder-input-model.md) |
| 2026-08-11 | Portability Boundary and Workspace Split | Accepted | [2026-08-11-portability-boundary-and-workspace-split.md](2026-08-11-portability-boundary-and-workspace-split.md) |
| 2026-08-11 | Presentation Surface and Run-Mode Seam | Accepted | [2026-08-11-presentation-surface-run-mode-seam.md](2026-08-11-presentation-surface-run-mode-seam.md) |
| 2026-08-11 | Three Run Modes for Agent-Testable Development | Accepted | [2026-08-11-three-mode-testability.md](2026-08-11-three-mode-testability.md) |
| 2026-01-22 | Keyboard Emulation Before FIDO2 for PoC | Accepted | [2026-01-22-keyboard-emulation-first.md](2026-01-22-keyboard-emulation-first.md) |
| 2026-01-22 | Desktop Emulator HTTP Protocol for BLE Simulation | Superseded | [2026-01-22-emulator-http-protocol.md](2026-01-22-emulator-http-protocol.md) |
| 2026-01-21 | Focus Management System for Simple GUI | Accepted* | [2026-01-21-focus-management-system.md](2026-01-21-focus-management-system.md) |
| 2026-01-21 | Desktop Emulation for Rapid Development | Accepted | [2026-01-21-desktop-emulation.md](2026-01-21-desktop-emulation.md) |

## Decision Categories

For future organization, consider categorizing decisions by:
- **Architecture**: System design, component structure
- **Technology**: Framework, library, or tool choices
- **Hardware**: Hardware-specific decisions
- **UI/UX**: User interface and interaction design
- **Security**: Security and encryption decisions

## Notes

- Keep this index updated whenever you create a new decision file
- Use the format specified in CLAUDE.md for decision documents
- Archive deprecated decisions by updating their status rather than removing them
- `*` = Status applies with caveats (see decision file for details)

## Status Legend

- **Accepted**: Implemented or decided; applies to current work
- **Proposed**: Under consideration or awaiting stakeholder approval
- **Superseded**: Replaced by a newer decision (see the newer ADR for rationale)
- **Deprecated**: No longer applies; kept for historical context
