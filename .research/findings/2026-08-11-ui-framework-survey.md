# UI Framework Analysis: Custom Rasterizer Dead End vs. Embedded-Graphics Clean Rewrite

**Date**: 2026-08-11  
**Researcher**: Fern (fe-architect)  
**Status**: Complete — informed ADR `2026-08-11-ui-framework-reuse-vs-rewrite.md`

## Question/Goal

Evaluate the existing GUI engines (`src/gui/` and `src/simple_gui/`) to determine: (1) What is actually running in production? (2) Are they salvageable for the T-Embed color-display migration? (3) If rewriting, what rendering foundation should replace them?

## Key Findings

### Finding 1: Ground Truth — What Code Actually Runs

**The codebase contains two GUI engines. Neither is fully operational.**

#### `src/gui/` — Dead Code Browser Engine

The flexbox/DOM engine at `src/gui/` is **not called anywhere in the app**. Evidence:
- `src/main.rs:119` — `// let mut document = create_view(...)` (commented out)
- `src/main.rs:135` — `// document.handle_input(...)` (commented out)
- `src/main.rs:163` — `// let canvas = document.render(...)` (commented out)

**Structure**: Incomplete browser-style layout engine with:
- `layout_tree.rs` + `layout_box.rs` (CSS flexbox algorithm)
- `style_tree.rs`, `styles.rs` (styling system)
- `node.rs` (DOM node abstraction)

**Known gaps**:
- Auto-width calculation unsupported (`layout_box.rs:202`): `// TODO: handle auto width in row`
- Scroll stub (`node.rs:127`): `scroll.translate(0,5)` with real scrolling logic commented out
- Per-node UUID + leaked fonts (`Box::leak` in style/font.rs)

**Verdict**: Abandoned experiment. A CSS-flexbox+DOM model is fundamentally mismatched for a 320x170 monochrome-then-color device. Salvage only: the *concept* of state-based styling (base style + `:focus` override, `styles.rs:155-165`) and box-model edge math.

#### `src/simple_gui/` — Shipped on 128x32 OLED, Architecturally Broken

This engine drives the current emulator and shipped on the Adafruit HUZZAH32 + SSD1306. Evidence:
- `src/main.rs:117, 133` references
- `bin/desktop.rs:68-70` calls it directly

**Structure**: Custom RGBA software rasterizer with:
- `simple_gui/render/image_buffer.rs:6-10` — RGBA frame buffer (4 bytes/pixel, ~213 KB/frame)
- `simple_gui/document/mod.rs:8-60` — navigation stack (view push/pop + per-entry focus memory)
- `simple_gui/components/` — button-responsive components returning `ComponentAction` (PushView/PopView/None)

**Critical blocker for color**: The rendering pipeline is fundamentally hostile to color displays:
1. All rendering targets a custom RGBA `Vec<Color>` framebuffer (not a drawable surface API).
2. Only the final blit thresholds RGBA → 1-bit binary for SSD1306 (`simple_view.rs:140-171`).
3. Adding RGB565 would require rewriting the rasterizer; the custom layer adds no value over `embedded-graphics`.

**Font rendering broken**:
- Baked ASCII-only mono font at build time; cells leaked to static memory (`simple_gui/style/font.rs:46-71`, `Box::leak`)
- Recoloring unimplemented (`simple_gui/render/canvas.rs:27` and identically `gui/render/canvas.rs:27`): `// TODO: Implement re-coloring fonts`
- Concrete bug: `simple_view.rs:103` shows `Pass: [bullet]` but renders as `Pass: ????????` because bullet U+2022 isn't in the ASCII table and `get_character()` fallback returns `?` (`font.rs:38-43`)

**Rendering architecture issue**: Both `src/gui/` and `src/simple_gui/` implement **parallel, near-duplicate render layers**:
- `gui/render/canvas.rs` vs. `simple_gui/render/canvas.rs` — nearly identical, both with the same unimplemented recoloring TODO.
- Neither uses `embedded-graphics` as the render target.
- `embedded-graphics` is consumed *only* as a font bake tool and final-blit driver, not as the primary rendering abstraction.

### Finding 2: Why Embedded-Graphics Is the Right Foundation

**Embedded-graphics is the appropriate standard for this hardware class.**

The ecosystem crate `embedded-graphics` 0.8 provides:
- **`DrawTarget` trait**: a unified abstraction for any drawing surface (framebuffer, display driver, test double).
- **Color abstraction**: `Rgb565`, `Rgb888`, `BinaryColor`, etc., with trait-based composition.
- **Real clipping**: `DrawTargetExt::clipped()` and `translated()` for viewport composition (no custom rasterizer needed).
- **Font rendering**: `MonoTextStyle` with foreground color, no baking required.

**Why replace custom rasterizer with embedded-graphics**:
- The custom RGBA rasterizer in `simple_gui/render/image_buffer.rs` is ~700 lines of code doing (poorly) what `embedded-graphics` does for free.
- Rewriting onto `embedded-graphics` is *less code* than porting the rasterizer to RGB565.
- The old layer structurally blocks color: it exists to produce an intermediate RGBA buffer that can blit to minifb or OLED, but `embedded-graphics`' `DrawTarget` trait IS that abstraction and already in the dependency tree.

### Finding 3: OSS Integration Survey (Adopt/Reject/Defer)

**Canonical ecosystem baseline**:

| Crate | Version | Decision | Rationale |
|-------|---------|----------|-----------|
| `embedded-graphics` | 0.8 | **ADOPT** | Core foundation. `DrawTarget` abstraction, `Rgb565`, clipping via `DrawTargetExt`, `MonoTextStyle`. Battle-tested in the embedded Rust ecosystem. |
| `embedded-graphics-framebuf` | latest | **ADOPT** | In-RAM framebuffer that is itself a `DrawTarget`. Memory-efficient Rgb565 storage (106 KB/frame vs. 213 KB/frame RGBA8888). Unifies headless + windowed rendering (both use same buffer). |
| `mipidsi` | latest | **ADOPT** | Canonical ST7789 driver implementing `DrawTarget`. Net-new for this project (current `Cargo.toml` has only `ssd1306`). Mature, widely used. |
| `u8g2-fonts` | latest | **ADOPT** | Large bitmap font collection with real glyph coverage (fixes bullet, emoji, Unicode gaps). Direct-to-DrawTarget rendering. Replaces baked-font hack. Or use embedded-graphics' built-in `MonoFont` to start. |
| `embedded-layout` | latest | **ADOPT LIGHTLY** | Alignment + LinearLayout helpers for composing regions. Don't use as a full layout engine; use for fixed chrome + linear stacks (title bar, content, footer). |
| `embedded-menu` | latest | **DEFER TO M1** | Ready scrolling-list-with-selection widget. Useful for credential list view, but premature for M0. Inputs/theming model may conflict with navigation-stack app model; not blocking M0 completion. |
| `slint` (MCU backend) | latest | **REJECT** | Declarative UI framework. Heavy for ESP32-S3 PoC. Doesn't expose a clean `DrawTarget` abstraction; would require custom render-to-framebuffer bridge, breaking the three-mode screenshot-fidelity story. Toolchain complexity. |
| `lvgl` + `lvgl-rs` | latest | **REJECT** | Feature-rich, large footprint, not Rust-native. Over-engineered for M0 credential browser. Same `DrawTarget` abstraction problem as `slint`. |
| `embedded-graphics-simulator` (SDL2) | latest | **REJECT** | SDL2-backed windowed simulator. Incompatible with three-mode architecture: introduces an extra rendering path (SDL2 → framebuffer → PNG) that differs from headless (framebuffer → PNG) and minifb (framebuffer → window), risking pixel-level divergence. Use custom in-RAM-framebuffer + minifb + png instead. |

### Finding 4: Design for the New Framework (Retained Concepts, Clean Implementation)

**What to salvage from `simple_gui` (reimplemented cleanly, not ported)**:

1. **Navigation stack** (`simple_gui/document/mod.rs:8-60`): Document model holding a stack of Views. Each view has a focused component index and per-view focus memory. Keep the pattern; reimplement on `embedded-graphics` DrawTarget semantics.

2. **ComponentAction** (`simple_gui/components/mod.rs:21-28`): Components return an action to the document: `PushView(new_view)`, `PopView`, or `None`. Clean inversion-of-control pattern for user input. Retain.

3. **FocusEvent** (`simple_gui/components/mod.rs:10-18`): High-level focus state changes: `Gained`, `Lost`, `Activated`. Already formalized in prior ADR (`2026-01-21-focus-management-system.md`). Retain.

**Component model** (trait-driven, retained-mode):

```rust
pub trait Widget {
    fn measure(&self, constraints: Size) -> Size;  
    fn render(&self, area: Rectangle, target: &mut Surface);
    fn is_focusable(&self) -> bool { false }
    fn on_focus(&mut self, ev: FocusEvent) -> ComponentAction;
    fn on_intent(&mut self, intent: NavIntent) -> ComponentAction;
}
```

Where `Surface = &mut impl DrawTarget<Color=Rgb565>`.

Widgets draw directly into their assigned viewport rectangle using `DrawTarget`. Clipping is via `target.clipped(&area)` (embedded-graphics native). No custom rasterizer, no intermediate RGBA buffer.

**Layout strategy** — fixed regions + linear stacks (NOT flexbox):

- **Chrome**: Fixed title bar (top), content area (center), hint/action bar (bottom). Rectangles derived from screen size constants `WIDTH` and `HEIGHT`.
- **Content area**: Vertical scrolling list (menu) or horizontal detail stack. Both use linear stack + scroll offset + viewport clipping.
- **No auto-width, no grid, no flex.** Simplicity and compile-time predictability.
- **LinearLayout from `embedded-layout`**: scrolling list = LinearLayout over list items with viewport clipping and scroll offset math.
- **Auto-scroll-to-keep-focused-visible**: algorithm from old ADR carries over on a clipped viewport (true clipping, not character-skipping).

**Render pipeline** (three surfaces, one framebuffer):

1. **Core**: Widgets draw into Rgb565 `embedded-graphics-framebuf` (in-RAM shared framebuffer).
2. **Headless**: Framebuffer captured to PNG byte array on demand (HTTP `/api/screenshot` endpoint).
3. **Windowed**: Rgb565 framebuffer scaled and copied to minifb window (existing logic at `bin/desktop.rs:156-178`, migrated to Surface trait).
4. **Real-target**: Framebuffer transferred to ST7789 via `mipidsi` DrawTarget (single transaction, no quantization).

Redraw is dirty-flag driven.

**Input model** (two layers, supersedes old KeyCode):

Layer 1: Raw device events
- Rotary encoder: CW/CCW detent, button press/hold/release
- Desktop: keyboard arrows, Enter/Backspace/Esc, mouse wheel

Layer 2: Semantic `NavIntent`
```rust
pub enum NavIntent {
    Next,           // move to next item
    Prev,           // move to prev item
    NextN(u16),     // fast-scroll (large vault acceleration)
    Activate,       // select/confirm
    Back,           // go back (long-press or Esc)
}
```

Mappings:
- Encoder CW → Next; CCW → Prev; short press → Activate; long press → Back
- Desktop: arrows/wheel → Next/Prev; Enter → Activate; Backspace/Esc → Back
- Headless HTTP: POST NavIntent directly

This supersedes the old input/mod.rs (`KeyCode{Up, Middle, Down}`, `input/mod.rs:3-8`) and the synthesized '< Back' list item hack (`simple_view.rs:77-79`).

### Finding 5: Hacks to Retire (Do NOT Port)

**These are bugs/limitations that must not survive into the rewrite**:

1. **Character-skipping marquee** (`simple_gui/vertical_menu_item.rs:211-233`): Scrolls long text by skipping characters. Clunky UX. Retire in favor of true clipping + horizontal panning.

2. **Baked-to-RGBA leaked mono fonts** (`simple_gui/style/font.rs:46-71`, `Box::leak`): Static font bitmaps leak memory and are ASCII-only. Use `u8g2-fonts` (vector) or `embedded-graphics` `MonoFont` (built-in, glyph coverage > ASCII).

3. **95-char ASCII-only font** (`font.rs:11-17`): Concrete bug: bullet U+2022 renders as '?' and '•' in passwords shows as '????????' (`simple_view.rs:103`). Retire; adopt fonts with full Unicode coverage.

4. **RGBA8888 full-frame compositor** (`simple_gui/render/image_buffer.rs`, 213 KB/frame): Bloated, color-hostile. Replace with Rgb565 (106 KB/frame) + embedded-graphics native rendering.

5. **Threshold-to-BinaryColor blit** (`simple_view.rs:157-161`): Converts RGBA to 1-bit for SSD1306. Becomes moot when rendering directly to display driver.

6. **Synthesized '< Back' list item** (`simple_view.rs:77-79`): UI hack to show back as a clickable item. Retire; use long-press or back button intent instead.

7. **Entire `src/gui/` flexbox/DOM engine**: Dead code. Delete it. If flexbox is needed in M2+, a fresh implementation tailored to the use case is faster than salvaging.

### Finding 6: Risks and Open Questions

**Minifb vs. embedded-graphics-simulator trade-off**:
- **Chose**: minifb + custom PNG encoding (headless)
- **Alternative**: SDL2-backed `embedded-graphics-simulator`
- **Risk**: SDL2 adds a native binary dependency on host. Minifb is lighter and avoids the extra rendering path.
- **Recommendation**: minifb. Measure if SDL2 simplifies the code significantly before reconsidering.

**Font quality and emoji**:
- Both `u8g2-fonts` and embedded-graphics' built-in `MonoFont` are bitmap fonts without anti-aliasing.
- May look crude for a credible demo on a 320x170 color screen.
- **Mitigation**: Start with bitmap, flag the glyph coverage as revisitable. If needed, explore vector fonts (e.g., `slint`'s SDF rendering) in M2+, but don't block M0.
- **Owner**: Uma (ux-designer) to assess visual acceptability.

**PSRAM framebuffer performance**:
- Full-frame Rgb565 (106 KB) on a 320x170 display may stress draw/flush timing on ESP32-S3, especially if the core loop is also handling input and credential sync.
- **Mitigation**: Implement dirty-region redraw (only flush changed regions). Measure on hardware.
- **Test plan**: Profile core loop (input poll → render → flush) under headless, windowed, and real-target conditions.

**Embedded-menu adoption risk**:
- The library provides a scrolling-list-with-selection widget, useful for M1's credential list.
- Its input/theming model might conflict with the app's navigation-stack pattern.
- **Defer to M1**: Evaluate after core framework is shipping and M1 UX requirements are finalized. Not blocking M0.

## Implications for the Project

### Rewrite Decision Confidence

The rewrite is justified for these reasons:

1. **No viable patch path**: `simple_gui`'s custom rasterizer is a structural blocker to color. Porting it to RGB565 is not incremental; it's a rewrite with higher risk.

2. **Foundation maturity**: `embedded-graphics` is widely adopted, battle-tested, and the de-facto standard for Rust embedded graphics. No risk of adopting it.

3. **Code clarity**: The new framework will be simpler (no custom rasterizer, no baked fonts, no half-finished layout engine) and more maintainable.

4. **Three-mode honesty**: Building on `embedded-graphics` DrawTarget enables a single render path that's true across headless, windowed, and real-target. No surprise divergence.

### Effort Estimate

Based on the components to build:
- Widget trait + trait implementations (Label, VerticalMenu, Spacer, CredentialListView, CredentialDetailView): ~500-700 lines.
- Navigation stack (document model): ~100-150 lines.
- Integration with display surfaces (headless, minifb, ST7789): ~200-300 lines (surface implementations differ; core is unchanged).
- Font setup (u8g2-fonts or MonoFont): ~50-100 lines.
- Testing (unit + integration + screenshot): ~300-500 lines.

**Total new code**: ~1200-1750 lines (vs. salvaging and extending ~1200 lines of `simple_gui` + 800 lines of `gui` = 2000 lines of debt).

The rewrite is net-neutral or net-positive on effort, with cleaner semantics.

## Recommendations

### Implementation Order

1. **W1**: Set up Cargo workspace (core/firmware/emulator).
2. **W2**: Implement DisplaySurface trait + framebuffer glue (headless, minifb, real-target).
3. **W3**: Build Widget trait and basic implementations (Label, Spacer, VerticalMenu).
4. **W4**: Implement navigation stack + ComponentAction/FocusEvent.
5. **W5**: Integrate NavIntent input model.
6. **W6**: Build CredentialListView and CredentialDetailView.
7. **W7**: Screenshot tests (headless) and visual validation (emulator).
8. **W8**: Port and test on T-Embed hardware.
9. **W9**: Performance profiling and optimization (dirty-region redraw if needed).

### Dependency Additions

In `core/Cargo.toml`:
- `embedded-graphics = "0.8"`
- `embedded-graphics-framebuf = "latest"` (or equivalent in-RAM buffer)
- `serde`, `serde_json` (credential serialization)

In `firmware/Cargo.toml`:
- `mipidsi` (ST7789 driver)
- `u8g2-fonts` or `embedded-graphics` built-in fonts

In `emulator/Cargo.toml`:
- `minifb` (existing)
- `png` (or `image` crate for PNG encoding)
- `tiny_http` (existing)

### Deprecations

Remove from dependency tree:
- `ssd1306` (no longer needed; ST7789 and headless/minifb are the three surfaces).

## Status

This research informed the M0 ADR: `2026-08-11-ui-framework-reuse-vs-rewrite.md`. Implementation work is tracked in epic `ai-bitwarden-hw-key-8d7` (workstreams W1-W9).
