# Desktop Emulation for Rapid Development

**Date**: 2026-01-21
**Status**: Accepted
**Category**: Technology, Architecture

## Context

Developing embedded GUI applications on ESP32 hardware requires a time-consuming cycle:
1. Write code
2. Compile for ESP32 (slow due to cross-compilation)
3. Flash to hardware
4. Observe behavior
5. Repeat

This cycle makes rapid iteration difficult, especially when developing and testing UI components. Additionally, debugging embedded systems is more challenging than desktop debugging.

We needed a way to speed up development without compromising the ESP32 target.

## Decision

Implement desktop emulation using separate binaries with shared GUI code:
- **ESP32 Binary**: `src/main.rs` → `esp32` binary (unchanged)
- **Desktop Binary**: `src/bin/desktop.rs` → `desktop` binary (new)
- **Shared Code**: All `simple_gui` modules work on both platforms

### Technical Implementation

**Windowing**: minifb library
- Lightweight pixel buffer rendering
- Cross-platform (macOS, Linux, Windows)
- Simple API with direct RGB buffer access

**Display Scaling**: 8x magnification
- Native: 128x32 pixels
- Window: 1024x256 pixels
- Makes the small display easily visible

**Input Mapping**:
- Arrow Up → KeyCode::Up
- Arrow Down → KeyCode::Down
- Space → KeyCode::Middle

**Dependency Management**:
- ESP32 dependencies (esp-idf-svc, ssd1306, etc.) only compile for `target_arch = "xtensa"`
- Desktop dependencies (minifb) only compile for non-xtensa targets
- Shared dependencies (embedded-graphics) compile for all targets

**Module Structure**:
- Created `src/lib.rs` to export shared modules
- Made `simple_gui` and `simple_view` public
- Added conditional `desktop` module for non-ESP32 targets

## Rationale

### Why Separate Binaries?
- **Zero Bloat**: Desktop code never touches ESP32 binary
- **Clean Separation**: Each platform has its own entry point
- **Maintainability**: Clear distinction between platform-specific code

### Why minifb?
- Lightweight (single dependency)
- Direct pixel buffer access matches our Canvas model
- No heavy GUI framework overhead
- Cross-platform support for team members

### Why 8x Scaling?
- Original 128x32 is too small to see on modern displays
- 8x gives 1024x256, easily visible but not huge
- Clean integer scaling preserves pixel boundaries

### Why Target-Specific Dependencies?
- Prevents ESP32 linker errors when building for desktop
- Prevents desktop compilation of ESP-IDF (requires special toolchain)
- Cargo handles this automatically with `target.'cfg(...)'` sections

## Alternatives Considered

### Option 1: Simulator with Mock Hardware
- **Pros**: Could simulate I2C bus, test display driver
- **Cons**: Complex, slow to implement, over-engineered for GUI development
- **Verdict**: Rejected - we only need GUI iteration, not hardware simulation

### Option 2: Conditional Compilation with Features
- **Pros**: Single binary target
- **Cons**: Pollutes codebase with cfg attributes, harder to maintain
- **Verdict**: Rejected - separate binaries are cleaner

### Option 3: Web-based Emulator (WASM)
- **Pros**: Could share with non-technical stakeholders via browser
- **Cons**: Complex build setup, limited debugging, slower development
- **Verdict**: Rejected - native desktop is simpler and faster

### Option 4: SDL2 or winit
- **Pros**: More features, event handling, standard game dev libraries
- **Cons**: Heavier dependencies, more complex APIs, overkill for simple pixel buffer
- **Verdict**: Rejected - minifb is simpler and sufficient

## Consequences

### Positive
- **Rapid Iteration**: Edit code, run, see results in seconds
- **Better Debugging**: Full desktop debugging tools (lldb, gdb, println)
- **No Hardware Needed**: Develop without ESP32 connected
- **Cross-Platform Dev**: Team members can develop on any OS
- **Zero ESP32 Impact**: No binary size increase, no performance impact

### Negative
- **Two Main Loops**: Must maintain consistency between ESP32 and desktop main loops
- **Timing Differences**: Desktop runs faster, may hide timing bugs
- **No Hardware Testing**: Can't catch I2C issues or display quirks until flashing
- **Input Differences**: Desktop keyboard vs. hardware buttons behave differently

### Mitigations
- Keep main loops similar in structure
- Test on hardware before considering features "done"
- Use the same `InputInterface` trait for consistency
- Document that desktop is for GUI dev only, not final testing

## Usage

### Building and Running Desktop Emulator
```bash
cargo build --bin desktop --target aarch64-apple-darwin
cargo run --bin desktop --target aarch64-apple-darwin
```

### Building ESP32 Binary
```bash
cargo build --bin esp32
# Or with environment:
. $HOME/export-esp.sh && cargo build --bin esp32
```

### Controls
- **Arrow Up**: Navigate up / KeyCode::Up
- **Arrow Down**: Navigate down / KeyCode::Down
- **Space**: Select / KeyCode::Middle

## References

- minifb crate: https://crates.io/crates/minifb
- Cargo target-specific dependencies: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies
- Implementation files:
  - `src/bin/desktop.rs` - Desktop main entry point
  - `src/desktop/input.rs` - Keyboard input handling
  - `src/lib.rs` - Library exports
  - `Cargo.toml` - Target-specific dependencies

## Future Considerations

- Consider adding screenshot capability for documentation
- Could add FPS counter for performance monitoring
- Might add hot reload if development speed becomes critical
- Could bridge desktop input to full InputInterface for testing click patterns
