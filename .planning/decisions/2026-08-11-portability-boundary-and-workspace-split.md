# Portability Boundary and Workspace Split

**Date**: 2026-08-11
**Status**: Accepted

## Context

The hardware-portable core (application logic, GUI framework, data model) must be isolated from platform-specific dependencies (ESP-IDF, minifb, target CPU). Without a clear boundary, it's easy for embedded code to creep into the "portable" layer, and drift becomes undetectable until the codebase is built for multiple targets.

Currently, work is happening in a single Rust crate (`bitwarden-hw-key`) with `cfg(target_arch=...)` to conditionally compile different dependencies. This convention-based approach fails silently: a developer can add `use esp_idf_svc` in the core logic, and the build passes on firmware but fails on host only after CI runs (or never, if CI is incomplete).

## Decision

Enforce the portability boundary with a **three-layer Cargo workspace**:

```
workspace/
├── core/                 # Platform-free app logic, GUI framework, data model
│   ├── src/
│   │   ├── gui/         # Rewritten GUI framework (embedded-graphics based)
│   │   ├── credentials/
│   │   ├── storage/     # Traits only (platform-trait layer)
│   │   ├── input/       # NavIntent and high-level input abstractions
│   │   └── lib.rs
│   └── Cargo.toml      # NO esp-idf, NO minifb, NO target-specific deps
│
├── firmware/            # ESP32-S3 firmware (uses core)
│   ├── src/
│   │   ├── board/t_embed.rs  # T-Embed pin map and BoardConfig
│   │   ├── platform/         # Trait implementations (ST7789, NVS, RotaryEncoder, Clock)
│   │   ├── main.rs
│   │   └── lib.rs
│   └── Cargo.toml      # Depends on `core`; includes esp-idf-hal, esp-idf-svc
│
└── emulator/            # Desktop emulator (uses core)
    ├── src/
    │   ├── platform/    # Trait implementations (minifb, headless PNG, native FS, Clock)
    │   ├── desktop.rs   # Headless and windowed surface routing
    │   └── main.rs / lib.rs
    └── Cargo.toml      # Depends on `core`; includes minifb, tiny_http, etc.
```

**Boundary enforcement**: The `core` crate has no `esp_idf*`, `minifb`, or any other platform-specific dependency in `Cargo.toml`. If a feature is needed that the current traits don't export, add it to the trait (in `core`), then implement it in `firmware/` and `emulator/`. This makes the gap visible in code review.

**Trait layer**: `core/src/storage/`, `core/src/platform/`, etc., define the four traits (`DisplaySurface`, `InputSource`, `Clock`, `Storage`) and any shared data structures. Implementation modules live in the platform-specific crates (`firmware/src/platform/`, `emulator/src/platform/`).

**Board adapter**: `firmware/src/board/t_embed.rs` houses the T-Embed pin map (`BoardConfig`), GPIO/SPI configuration, and any hardware-specific setup. This is NOT a HAL re-abstraction; it's just initialization and plumbing between esp-idf-hal and the trait implementations.

**No bespoke HAL wrapper**: Do not create custom abstractions over SPI, GPIO, encoder, etc. esp-idf-hal and the Rust ecosystem drivers (e.g., `mipidsi`, `embedded-hal` encoder drivers) are sufficient. Over-engineering a custom HAL layer adds maintenance burden and diverges from upstream.

Fallback (rejected): a single crate with conditional imports detected by CI import-grep. This is convention-based and fails silently.

## Rationale

- **Compiler enforcement** catches portability violations at compile time, not in code review or CI.
- **Clear ownership**: code that references ESP-IDF lives only in `firmware/`, reducing cognitive load.
- **Testability**: the `core` crate can be tested on the host in isolation (`cargo test --package core`).
- **Hardware independence**: swapping T-Embed for a different ESP32 board (or even a different SoC) is now a localized change to `firmware/src/board/`.
- **Dependency clarity**: anyone reading `Cargo.toml` knows immediately what this crate depends on.
- **Parity enforcement**: both `firmware/` and `emulator/` must implement the same traits from `core`, preventing accidental drift.

## Alternatives Considered

- **Single crate + cfg-based imports + CI import-grep.** Existing state.
  - **Pros**: minimal file reorganization.
  - **Cons**: silent failures (invalid imports sneak past until CI); no compiler guarantee; developers can't easily test core in isolation.
  - **Verdict**: Rejected. The workspace approach is worth the reorganization cost.

- **Create a custom HAL abstraction (SpiDriver, GpioPin, etc.).** Add a new layer above esp-idf-hal.
  - **Pros**: unified interface across platforms (hardware and emulator).
  - **Cons**: duplicates esp-idf-hal's work; increases maintenance burden; breaks the rule "ESP-IDF doesn't need to be re-abstracted."
  - **Verdict**: Rejected. Let the platform-trait layer (DisplaySurface, InputSource, etc.) be the HAL boundary.

## Consequences

### Positive
- Compile-time verification that the core doesn't reference platform-specific code.
- Easier to test core logic and GUI framework without hardware.
- Clear organization: new contributors immediately see what each crate owns.
- Simpler to port to a different board or host platform (copy `firmware/` or `emulator/`, update `BoardConfig` or surface impl).

### Negative
- File reorganization required (move files, update imports, split Cargo.toml).
- Three separate `cargo build` invocations instead of one (minor; unlikely to matter in CI).
- Slightly more boilerplate in platform-specific crates to implement the traits.

## Implementation Notes

- Workspace root `Cargo.toml` resolves versions for all three crates (prevents dependency version skew).
- `core` exports public trait definitions via `lib.rs`. Implementations are private.
- Document the expected trait implementations in a `INTEGRATION.md` at the workspace root.

## References

- Owners: Ada (architect), Ruby (rust-embedded-supervisor)
- Related decisions: [2026-08-11-presentation-surface-run-mode-seam.md](2026-08-11-presentation-surface-run-mode-seam.md)
- Roadmap Open Question 3 (answered by this decision)
