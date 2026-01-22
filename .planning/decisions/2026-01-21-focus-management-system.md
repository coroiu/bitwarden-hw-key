# Focus Management System for Simple GUI

**Date**: 2026-01-21
**Status**: Accepted
**Category**: Architecture, UI/UX

## Context

The `simple_gui` component system had no way to handle user input or navigation between components. While the older `gui` module had focus management with a DOM-like structure, the new simplified system needed a lightweight approach that:

1. Abstracts raw button/keyboard events into higher-level focus events
2. Allows components to manage their own visual representation when focused
3. Supports automatic scrolling to keep focused items visible
4. Works seamlessly with both desktop keyboard and ESP32 hardware buttons

The goal was component-level focus (not DOM-style element focus) where entire components receive focus and handle their own internal state.

## Decision

Implement a three-tier focus management architecture:

### 1. Focus Events (High-Level Abstraction)
```rust
enum FocusEvent {
    Gained,      // Component just received focus
    Lost,        // Component just lost focus
    Activated,   // User activated focused component (e.g., pressed select)
}
```

Components receive these high-level events instead of raw button presses.

### 2. Component Trait Extensions
```rust
trait Component {
    fn is_focusable(&self) -> bool { false }
    fn on_focus_event(&mut self, event: FocusEvent) {}
    fn on_input(&mut self, events: &[InputEvent]) {}
}
```

- **Opt-in focusability**: Components must explicitly return `true` from `is_focusable()`
- **Focus lifecycle**: `on_focus_event()` for focus state changes
- **Raw input access**: `on_input()` receives raw input when focused (for advanced components)

### 3. Document-Level Focus Manager
The `Document` tracks which component has focus and handles navigation:
- `handle_input()`: Processes input and routes to focused component
- `focus_next()` / `focus_previous()`: Navigate between focusable components
- `initialize_focus()`: Auto-focus first focusable component
- Translates Up/Down keys to focus navigation
- Translates Middle/Space key to Activation event

### 4. Component-Level Selection Management
Components like `VerticalMenu` handle their own internal selection:
- Menu tracks `selected_index` internally
- Handles Up/Down input to change selection
- Auto-scrolls viewport to keep selected item visible
- Delegates visual rendering to child items (`VerticalMenuItem`)

## Rationale

### Why High-Level Focus Events?
- **Abstraction**: Components don't need to know if input is from keyboard, buttons, or touchscreen
- **Simplicity**: No need to track press/release states or timing
- **Reusability**: Same component works on desktop and hardware

### Why Opt-In Focusability?
- **Performance**: Only focusable components are iterated during navigation
- **Clarity**: Explicit declaration of interactive vs. static components
- **Flexibility**: Labels and decorative elements can't receive focus

### Why Component-Internal Selection?
- **Encapsulation**: VerticalMenu owns its selection state
- **Auto-scrolling**: Menu calculates viewport and scroll offset internally
- **Flexibility**: Different components can handle selection differently

### Why Document-Level Navigation?
- **Single source of truth**: Only one component has focus at a time
- **Keyboard shortcuts**: Document can intercept global shortcuts
- **Predictable behavior**: Tab order follows component order

## Alternatives Considered

### Option 1: DOM-Style Element Focus
Like the old `gui` module with `tab_index` and element-level focus.

- **Pros**: More granular control, follows web standards
- **Cons**: Complex for embedded, over-engineered for simple menus
- **Verdict**: Rejected - too heavyweight for embedded constraints

### Option 2: Global Focus Manager Singleton
Separate FocusManager that tracks all focusable components.

- **Pros**: Centralized logic, easier debugging
- **Cons**: Another layer of indirection, breaks component encapsulation
- **Verdict**: Rejected - Document already owns components

### Option 3: Raw Input Events Only
No focus abstraction, components handle raw KeyCode events.

- **Pros**: Simple, no abstraction overhead
- **Cons**: Every component reimplements focus logic, harder to add touchscreen later
- **Verdict**: Rejected - doesn't scale to multiple input methods

### Option 4: Focus Chain / Linked List
Components store references to next/previous focusable components.

- **Pros**: Fast navigation, no iteration needed
- **Cons**: Complex lifetime management, breaks when components added/removed
- **Verdict**: Rejected - premature optimization, harder to maintain

## Implementation Details

### Auto-Scrolling Algorithm
```rust
fn auto_scroll(&mut self) {
    let selected_y = self.item_y_position(self.selected_index);
    let selected_height = self.items[self.selected_index].size().height;
    let viewport_height = self.bounds.height;

    // Scroll up if item is above viewport
    if selected_y < self.scroll {
        self.scroll = selected_y;
    }
    // Scroll down if item is below viewport
    else if selected_y + selected_height > self.scroll + viewport_height {
        self.scroll = (selected_y + selected_height - viewport_height).max(0);
    }
}
```

The algorithm ensures the selected item is always fully visible within the viewport.

### Selection Border Rendering
`VerticalMenuItem` draws a 1-pixel white border when `is_selected` is true:
- Four rectangles (top, bottom, left, right)
- Rendered before the label text
- Label offset by 1 pixel to accommodate border

## Consequences

### Positive
- **Clean Abstraction**: Components don't care about input hardware
- **Encapsulated State**: Each component manages its own focus rendering
- **Automatic Scrolling**: Menus handle viewport management internally
- **Desktop Development**: Full interactive testing without hardware
- **Extensible**: Easy to add new focusable components

### Negative
- **No Global Shortcuts**: Can't easily implement Ctrl+key shortcuts
- **Single Focus Only**: No multi-select or complex focus patterns
- **Navigation Overhead**: Must iterate components to find next focusable
- **Component Coupling**: Menu items must support `set_selected()`

### Mitigations
- Global shortcuts can be added to `Document.handle_input()` if needed
- Multi-select can be component-internal state (like selection index)
- Focusable components list could be cached if performance becomes issue
- Selection interface could be formalized in trait if needed

## Usage Example

```rust
// In simple_view.rs
let mut menu = VerticalMenu::new(Rectangle::new(0, 0, width, height), &font);
menu.items_mut().push(VerticalMenuItem::new(&font, "Item 1"));
document.components_mut().push(Box::new(menu));

// In desktop.rs
document.initialize_focus();  // Auto-focus menu
document.handle_input(&mut input);  // Process keyboard input
```

## Testing

Verified in desktop emulator:
- Focus initialization on startup
- Arrow Up/Down navigation between items
- Space key activation events
- Auto-scrolling when navigating off-screen items
- Visual border on selected items
- Wrapping disabled (stays at first/last item)

## References

- Implementation: `src/simple_gui/components/mod.rs` (FocusEvent, Component trait)
- Document manager: `src/simple_gui/document/mod.rs` (focus tracking, navigation)
- Menu implementation: `src/simple_gui/components/vertical_menu/vertical_menu.rs`
- Item rendering: `src/simple_gui/components/vertical_menu/vertical_menu_item.rs`
- Desktop integration: `src/bin/desktop.rs`, `src/desktop/input.rs`

## Future Considerations

- Add `on_blur()` / `on_focus()` helper methods as alternatives to `on_focus_event()`
- Consider focus indicators beyond borders (animations, color changes)
- Add focus sound effects for hardware buttons
- Implement focus memory (restore focus to last selected on re-entry)
- Consider accessibility features (screen reader integration)
