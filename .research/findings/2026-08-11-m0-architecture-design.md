# M0 Architecture Design: Platform Abstraction, Render Core, and Workspace Boundary

**Date**: 2026-08-11  
**Researcher**: Ada (architect)  
**Status**: Complete — informed ADRs `2026-08-11-presentation-surface-run-mode-seam.md` and `2026-08-11-portability-boundary-and-workspace-split.md`

## Question/Goal

Design the portability boundary for the M0 release to enable three-mode testability (headless, windowed, real-target) while ensuring:
1. The app core is platform-agnostic (portable to new hardware without logic changes).
2. Compile-time enforcement of the boundary (not convention-based).
3. Pixel-fidelity parity across all three modes (headless screenshots == windowed display == hardware output).
4. Clear ownership and minimal boilerplate in platform adapters.

## Key Findings

### Finding 1: Platform Abstraction via Trait Injection

**A four-trait capability bundle abstracts the platform.**

The app core depends only on these traits; implementations live in platform-specific crates:

```rust
// core/src/platform/display.rs
pub trait DisplaySurface {
    fn flush(&mut self, framebuffer: &FrameBuffer) -> Result<(), Self::Error>;
    type Error: std::error::Error;
}

// core/src/platform/input.rs
pub trait InputSource {
    fn poll(&mut self) -> Vec<InputEvent>;
}

// core/src/platform/clock.rs
pub trait Clock {
    fn now(&self) -> Instant;
}

// core/src/platform/storage.rs
pub trait Storage {
    fn get(&self, key: &str) -> Option<Vec<u8>>;
    fn set(&mut self, key: &str, value: Vec<u8>) -> Result<(), Self::Error>;
    type Error: std::error::Error;
}
```

**Why traits, not a large `Platform` struct**:
- Each capability can be independently mocked for testing.
- App core can be unit-tested in isolation (inject test doubles).
- Implementations are focused (DisplaySurface is just I/O, not business logic).

**Trait layer ownership**: `core/src/platform/` defines the traits and shared data structures (`FrameBuffer`, `InputEvent`, `Instant`, etc.). Implementation modules live in platform-specific crates.

### Finding 2: Single Framebuffer, Rgb565 Canonical Throughout

**The app core owns one in-RAM shared framebuffer in Rgb565 format.**

This is the *sole* render output of the core. All three run modes differ only in their `DisplaySurface::flush()` implementation:

**Framebuffer definition**:
```rust
pub struct FrameBuffer {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<Rgb565>,  // or &mut [Rgb565] for PSRAM
}

impl DrawTarget for FrameBuffer {
    type Color = Rgb565;
    type Error = Infallible;
    // ... DrawTarget implementation
}
```

**Why Rgb565 canonical** (not RGB888 core + quantize-at-device):
- The ST7789 panel natively expects Rgb565. Rendering at higher bit-depth in the core would require quantization at the device boundary, introducing color fidelity mismatch.
- Headless PNG captures would be more colorful than hardware output, violating three-mode honesty.
- Memory budget: Rgb565 is 106 KB/frame (320×170×2 bytes) vs. RGB888 at 163 KB/frame. ESP32-S3 has 512 KB SRAM; Rgb565 leaves headroom for the app logic, input handling, and crypto.
- The ST7789 can't display RGB888 anyway; quantization would be lossy and adds complexity.

**Rendering guarantee**: Widgets and components draw directly into the framebuffer using `embedded-graphics` DrawTarget API. No intermediate RGBA buffer. No thresholding. No re-coloring hacks. The framebuffer is the source of truth.

### Finding 3: Three Surfaces and Their Implementations

**Run-mode selection on host (both surfaces compiled):**

The desktop binary (`emulator`) accepts a `--headless` flag at runtime. Both headless and windowed surfaces remain linked in the same binary, preventing code drift.

#### Surface 1: ST7789 Hardware Surface

**Target**: Real ESP32-S3 + Lilygo T-Embed hardware.

**Implementation** (`firmware/src/platform/display/st7789.rs`):
```rust
pub struct St7789Surface {
    spi: SpiDevice,
    dc: GpioPin,
    rst: GpioPin,
    driver: mipidsi::Builder</* ... */>,
}

impl DisplaySurface for St7789Surface {
    fn flush(&mut self, fb: &FrameBuffer) -> Result<(), Self::Error> {
        // Transfer framebuffer to ST7789 via SPI (single transaction)
        // mipidsi's DrawTarget implementation handles the protocol
        Ok(())
    }
}
```

**Key property**: `mipidsi` natively implements `DrawTarget<Color=Rgb565>`, so the core can render to the framebuffer and the adapter blits it without quantization.

#### Surface 2: Minifb Windowed Surface

**Target**: Desktop emulator with graphical window.

**Implementation** (`emulator/src/platform/display/minifb.rs`):
```rust
pub struct MinifbSurface {
    window: minifb::Window,
    scale: u32,  // 2x or 3x upscaling for visibility
}

impl DisplaySurface for MinifbSurface {
    fn flush(&mut self, fb: &FrameBuffer) -> Result<(), Self::Error> {
        // Convert Rgb565 -> ARGB8888 (minifb format)
        let pixels: Vec<u32> = fb.pixels.iter()
            .map(|rgb| rgb565_to_argb8888(*rgb))
            .collect();
        
        // Upscale and present
        self.window.update_with_buffer(&pixels, fb.width as usize * self.scale as usize, ...)?;
        Ok(())
    }
}
```

**Key property**: The Rgb565 framebuffer is converted to minifb's native ARGB8888, then optionally upscaled for visibility. No quantization artifacts.

#### Surface 3: Headless PNG Capture Surface

**Target**: Desktop emulator in headless mode (CI, automated testing, screenshot capture).

**Implementation** (`emulator/src/platform/display/headless.rs`):
```rust
pub struct HeadlessSurface {
    last_frame: FrameBuffer,  // Cached for on-demand PNG export
}

impl DisplaySurface for HeadlessSurface {
    fn flush(&mut self, fb: &FrameBuffer) -> Result<(), Self::Error> {
        // Buffer the framebuffer (no window update)
        self.last_frame = fb.clone();
        Ok(())
    }
}

impl HeadlessSurface {
    pub fn export_png(&self) -> Vec<u8> {
        // Encode self.last_frame -> PNG (e.g., using `image` crate)
        // Rgb565 -> PNG is lossless (no further quantization)
    }
}
```

**HTTP integration**: The headless emulator runs an HTTP server (tiny_http) with an endpoint:
```
POST /api/screenshot
Response: PNG bytes (Content-Type: image/png)
```

The HTTP handler calls `headless_surface.export_png()` and returns the cached framebuffer as PNG.

**Key property**: Headless PNG is pixel-identical to what the window shows (both source from the same Rgb565 framebuffer). This enables CI screenshot tests to verify UI correctness without a display.

### Finding 4: Compile-Time Boundary Enforcement via Workspace

**The portability boundary is enforced by Cargo workspace structure, not conventions.**

Current problem: A single crate with `cfg(target_arch=...)` allows invalid imports to slip past until CI. New structure:

```
workspace/
├── Cargo.toml (workspace root, resolves all dependency versions)
│
├── core/
│   ├── Cargo.toml (NO esp_idf*, NO minifb, NO platform-specific deps)
│   └── src/
│       ├── platform/
│       │   ├── display.rs     (trait definitions + FrameBuffer)
│       │   ├── input.rs       (trait definitions + InputEvent)
│       │   ├── clock.rs       (trait definitions + Instant)
│       │   ├── storage.rs     (trait definitions)
│       │   └── mod.rs
│       ├── gui/               (Widget trait + implementations)
│       ├── credentials/       (VaultItem view-model, platform-free)
│       ├── app/               (main loop, state machine)
│       └── lib.rs
│
├── firmware/
│   ├── Cargo.toml (depends on `core`; includes esp-idf-hal, esp-idf-svc, mipidsi)
│   └── src/
│       ├── board/
│       │   ├── t_embed.rs     (BoardConfig: pin map, SPI/GPIO setup)
│       │   └── mod.rs
│       ├── platform/
│       │   ├── display.rs     (St7789Surface implementation)
│       │   ├── input.rs       (RotaryEncoderSource implementation)
│       │   ├── storage.rs     (NvsStorage implementation)
│       │   ├── clock.rs       (EspClock implementation)
│       │   └── mod.rs
│       └── main.rs
│
└── emulator/
    ├── Cargo.toml (depends on `core`; includes minifb, tiny_http, image)
    └── src/
        ├── platform/
        │   ├── display/
        │   │   ├── minifb.rs  (MinifbSurface)
        │   │   ├── headless.rs (HeadlessSurface)
        │   │   └── mod.rs
        │   ├── input.rs       (KeyboardSource + HTTP NavIntent injection)
        │   ├── storage.rs     (FileSystemStorage)
        │   ├── clock.rs       (SystemClock)
        │   └── mod.rs
        ├── desktop.rs         (main loop + --headless flag routing)
        └── main.rs
```

**Boundary enforcement**:
- The `core` crate lists no esp-idf-hal, no minifb, no platform deps in Cargo.toml. The compiler rejects any `use esp_idf_svc::...` in `core/src/`.
- Both `firmware/` and `emulator/` implement all four traits from `core`. Drift is caught at compile time: if a new trait method is added to `core`, both will fail to compile until both update.
- New contributors see the structure immediately: platform-specific code is in `firmware/src/platform/` and `emulator/src/platform/`.

**Fallback rejected**: Single crate + CI import-grep on `use esp_idf*` is convention-based and fails silently.

### Finding 5: Board Adapter (Hardware Configuration, Not Custom HAL)

**`firmware/src/board/t_embed.rs` is NOT a custom HAL abstraction.**

It's a pin map and initialization helper:

```rust
pub struct BoardConfig {
    pub spi: SpiConfig,
    pub gpio: GpioConfig,
    pub encoder: EncoderConfig,
}

impl BoardConfig {
    pub fn t_embed() -> Self {
        Self {
            spi: SpiConfig {
                sclk: Gpio18,
                mosi: Gpio19,
                miso: Gpio20,
                freq_hz: 60_000_000,
            },
            gpio: GpioConfig {
                dc: Gpio32,
                rst: Gpio33,
            },
            encoder: EncoderConfig {
                a: Gpio16,
                b: Gpio17,
                button: Gpio5,
            },
        }
    }
}

// In main.rs:
let board = BoardConfig::t_embed();
let spi = SPI::new(board.spi);
let display = St7789Surface::new(spi, board.gpio);
```

**Why NOT a custom HAL wrapper**:
- `esp-idf-hal` and `mipidsi` (for ST7789) already abstract SPI and GPIO. Re-wrapping them adds indirection without payoff.
- Swapping to a different ESP32 board is a localized change to `board/t_embed.rs` (new pin constants, possibly new SPI config).
- Swapping to a non-ESP32 MCU is a new `firmware/src/board/` file + new trait implementations in `firmware/src/platform/`. The app core is unchanged.

**Rationale**: Traits are the HAL boundary, not custom wrappers. This keeps the codebase focused on the app logic and respects the ecosystem's existing abstractions.

### Finding 6: App Architecture — Main Loop and State Machine

**The core app is a synchronous main loop.**

```rust
// core/src/app/mod.rs
pub struct App {
    nav_stack: NavigationStack,   // Owns the View/Document model
    viewport: Rectangle,          // Screen bounds
    dirty: bool,                  // Render dirty flag
}

impl App {
    pub fn step(
        &mut self, 
        input_source: &mut dyn InputSource,
        storage: &dyn Storage,
        clock: &dyn Clock,
    ) -> Result<(), AppError> {
        // 1. Poll input
        for event in input_source.poll() {
            self.nav_stack.handle_input(event)?;
            self.dirty = true;
        }
        
        // 2. Render (if dirty)
        if self.dirty {
            let fb = self.nav_stack.render(self.viewport)?;
            // fb is a FrameBuffer ready for display.flush(fb)
            self.dirty = false;
        }
        
        Ok(())
    }
}

// In firmware/main.rs or emulator/main.rs:
loop {
    app.step(&mut input_source, &storage, &clock)?;
    display.flush(&fb)?;
    clock.sleep(Duration::from_millis(16));  // ~60 FPS
}
```

**Key properties**:
- The loop is **platform-free**: it references only traits, not concrete implementations.
- The loop is **synchronous**: no async/await. Credential sync (when it comes) will be a background task with a channel to the main loop.
- Dirty-flag driven: render only if input changed something.
- The app logic is re-testable: inject mock Display, InputSource, Clock, Storage; run `step()` in a unit test.

### Finding 7: Credential Model — Platform-Free View-Model

**Credentials are decoupled from the wire format (Bitwarden SDK Cipher) and the storage format (NVS blob).**

```rust
// core/src/credentials/mod.rs
#[derive(Clone, Debug)]
pub struct VaultItem {
    pub id: String,
    pub name: String,
    pub kind: VaultItemKind,  // Login, SecureNote, etc.
}

#[derive(Clone, Debug)]
pub enum VaultItemKind {
    Login {
        username: String,
        password: String,  // plaintext; encryption at rest is storage's concern
        uri: Option<String>,
    },
    SecureNote {
        text: String,
    },
}
```

**Why separate view-model**:
- The core GUI renders `VaultItem`, not the SDK's `Cipher` struct.
- On pull from the SDK, VaultItem is constructed once; on render, the GUI is unchanged.
- On M4 (sync), the app will push changes back to the SDK; the decoupling makes this explicit (no accidental bidirectional coupling to the SDK).

**Password storage today**: plaintext in RAM. Encrypted at rest in NVS (the Storage trait's concern). Wiped on credential removal (Android-style secure delete, if PSRAM is available).

**Note**: `password: String` is plaintext in memory. This is acceptable for M0 (PoC on trusted hardware). On M4 (BLE pairing + multiuser), consider zeroizing after use or using `zeroize` crate.

### Finding 8: Storage Trait — Opaque Blob KV for Credential Cache and Keys

**The Storage trait is a simple key-value blob store.**

```rust
pub trait Storage {
    fn get(&self, key: &str) -> Option<Vec<u8>>;
    fn set(&mut self, key: &str, value: Vec<u8>) -> Result<(), Self::Error>;
    type Error: std::error::Error;
}
```

**Usage**:
- **Credential cache**: `key = "creds:vault_{user_id}"`, value = CBOR-encoded Vec<VaultItem>.
- **Sync metadata**: `key = "meta:last_sync"`, value = timestamp.
- **Session keys** (M4): `key = "keys:session_{device_id}"`, value = encrypted session key blob.

**Implementations**:
- **Firmware** (`firmware/src/platform/storage.rs`): `NvsStorage` wrapping esp-idf-svc::nvs. Data persists in flash.
- **Emulator** (`emulator/src/platform/storage.rs`): `FileSystemStorage` using `./data/credentials.json`. Data persists to project directory (not user's home, to avoid polluting it).

**Encryption-at-rest roadmap**:
- M0: plaintext credentials in NVS (acceptable for PoC; Bitwarden accounts are already salty + hashed).
- M4: add a `StorageCodec` trait wrapping the Storage trait to encrypt/decrypt blobs. Inject encryption with the storage instance. This defers crypto design to M4 and doesn't block M0 shipping.

### Finding 9: Sync Source — Abstraction for the SDK Integration

**The app doesn't import the Bitwarden SDK directly.**

A `SyncSource` trait abstracts the sync backend:

```rust
pub trait SyncSource {
    fn sync(&mut self, user_id: &str, access_token: &str) -> Result<Vec<VaultItem>, Self::Error>;
    type Error: std::error::Error;
}
```

**Usage** (M2+):
- App calls `sync_source.sync(user_id, token)` on a background thread or in response to a user request.
- The implementation (e.g., `BwClientSync`) calls the Bitwarden SDK, transforms Cipher → VaultItem, returns the list.
- Results are pushed back to the Storage trait (cache).

**Why the indirection**:
- The core app is free from the SDK dependency (reduced footprint for headless tests).
- The SDK integration lives in `firmware/src/platform/sync.rs` + `emulator/src/platform/sync.rs`.
- On M4 (encrypted sync), the SDK integration is a localized change; the core app is unchanged.

**M0 status**: `SyncSource` trait is defined but not implemented. The app will have a stub. Credentials are seeded by hand for the PoC (see `.planning/progress.md` and `roadmap.md`).

### Finding 10: Risks and Open Questions

**Render/Input interface mismatch with Fern**:
- The FrameBuffer and InputEvent types must be frozen jointly before implementation begins (W1).
- Mismatch: if Fern's framework expects `DrawTarget<Color=Rgb888>` but Ada's arch specifies `Rgb565`, the implementations will collide.
- **Resolution**: Joint design session (Vera + Ada + Fern) to finalize FrameBuffer, InputEvent, and NavIntent. Documented in ADR comments.

**Encoder input vocabulary**:
- Rotary encoder fires discrete CW/CCW detent events + button press/hold/release.
- Mapping to NavIntent is assumed (CW→Next, CCW→Prev, short→Activate, long→Back).
- If UX requires different mappings (e.g., CW→PrevN for fast scroll in large vaults), the InputEvent enum must be extended.
- **Defer decision**: Uma (ux-designer) to finalize encoder behavior based on M0 UX mockups. Core framework supports flexible mapping.

**SDK footprint and latency**:
- The Bitwarden SDK (esp32-rust, if it exists, or vendored C SDK via esp-idf-svc) will add ~500 KB to firmware binary size.
- Argon2id key derivation on ESP32-S3 can take 1-5 seconds depending on params (acceptable for login, not for every credential display).
- **M4 mitigation**: Cache derived keys in NVS (encrypted session key slot); only re-derive on device unlock or timeout.
- **Risk**: If SDK is unavailable in Rust or for esp-rs, fallback to C SDK via esp-idf-svc or implement minimal CBOR-only protocol for M0.

**TLS and async sync**:
- Credential sync requires TLS (HTTPS to Bitwarden). Options:
  1. **rustls** (pure Rust): smaller footprint, but requires async-std or tokio runtime.
  2. **mbedtls** (C, wrapped via esp-idf-svc): larger, but integrates with esp-idf's network stack.
- **Decision pending** (M2): Benchmarks on ESP32-S3 (binary size, latency, RAM). Recommend starting with mbedtls (simpler integration) unless footprint is prohibitive.
- **Async complexity**: Sync should run on a background task (or scheduled coroutine via embassy). Core loop stays synchronous; sync results are delivered via a queue or callback.

**Color depth parity (RGB888 core vs. RGB565 device)**:
- **Resolved by Vera** (orchestrator): Rgb565-canonical was chosen. Headless PNGs are therefore as colorful as hardware (no fidelity mismatch).
- **Record**: This resolution was a trade-off. RGB888 core would allow future high-color-depth targets (e.g., an IPS panel), but at the cost of headless-hardware divergence. Rgb565 commitment is acceptable for M0-M2; M3+ can revisit if a new panel is adopted.

**Storage/crypto-at-rest shape**:
- The Storage trait is deliberately opaque (blob KV). Encryption logic is deferred to M4.
- **Decision needed** (M4): How are keys stored? Baked in firmware? User-derived password? Hardware security element (if ESP32-S3 has a suitable crypto accelerator)?
- **M0 implication**: Credentials are plaintext in NVS. Acceptable for PoC on trusted hardware. Clearly document this risk in release notes.

**Async sync vs. synchronous UI loop**:
- The app's main loop is synchronous (non-blocking 16ms cycles).
- Credential sync is I/O-bound (HTTP to Bitwarden). Blocking on the main loop would freeze the UI.
- **Solution**: Sync on a background task (e.g., embassy task, FreeRTOS task) that shares `Storage` with the main loop via a queue or RwLock. Main loop polls the sync-result queue and updates the UI.
- **M0 status**: Not implemented. Credentials are static (seeded once). M2+ will implement async sync.

## Recommendations

### Implementation Order (Workstreams W1-W9)

**W1 — Workspace Setup & Platform Traits**
- Create Cargo workspace: core/, firmware/, emulator/
- Define four trait modules in core/src/platform/
- Define FrameBuffer and InputEvent structs
- Document the three-layer architecture in README.md

**W2 — Presentation Surface Integration**
- Implement DisplaySurface for ST7789 (firmware/) + minifb (emulator/) + headless PNG (emulator/)
- Implement embedded-graphics-framebuf integration (core/)
- HTTP screenshot endpoint in emulator

**W3 — Input and Clock Traits**
- Implement InputSource for rotary encoder (firmware/) + keyboard (emulator/)
- Implement Clock trait (firmware/ uses esp-idf, emulator/ uses std::time)
- NavIntent enum + desktop keyboard mapping

**W4 — App Core and Navigation Stack**
- Implement main loop (core/src/app/mod.rs)
- Implement NavigationStack (core/src/navigation/mod.rs) + View/Document
- Widget trait + basic implementations (Label, Spacer, VerticalMenu)

**W5 — Credential Model and Storage**
- VaultItem struct (core/src/credentials/mod.rs)
- Storage trait + implementations (firmware/NvsStorage, emulator/FileSystemStorage)
- Seed credential data for M0 (hardcoded list or JSON file)

**W6 — GUI Components**
- CredentialListView (scrolling menu)
- CredentialDetailView (read-only detail pane)
- Focus management + FocusEvent handling

**W7 — Integration and Desktop Testing**
- Assemble core + platform layers in firmware/ and emulator/
- Unit tests for navigation stack, widgets, and input handling
- Screenshot tests (headless mode)
- Emulator visual testing

**W8 — Hardware Port**
- Build and deploy to T-Embed hardware
- Debug display initialization, SPI timing, encoder input
- Real-world performance measurement

**W9 — Optimization and Hardening**
- Profile and optimize render loop (dirty-region redraw if needed)
- Memory usage audit (framebuffer, widget allocations, stack depth)
- Error handling and graceful degradation (e.g., NVS read fails → show "Offline" state)

## Status

This research informed two M0 ADRs:
- `2026-08-11-presentation-surface-run-mode-seam.md` — Display abstraction and Rgb565-canonical decision
- `2026-08-11-portability-boundary-and-workspace-split.md` — Workspace organization and boundary enforcement

Implementation work is tracked in epic `ai-bitwarden-hw-key-8d7` (workstreams W1-W9).
