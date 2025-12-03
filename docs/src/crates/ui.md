# astrelis-ui

The `astrelis-ui` crate implements a retained-mode UI system with flexbox/grid layout (via Taffy) and GPU-accelerated rendering.

## Features

- **Declarative API**: Builder pattern for constructing UI trees.
- **Layout**: Full Flexbox and Grid support using Taffy.
- **Rendering**: Batched, instanced rendering for high performance.
- **Incremental Updates**: Dirty tracking to minimize layout and rendering work.
- **Widgets**: Built-in widgets like Container, Text, Button, TextInput.
- **Styling**: CSS-like styling properties.

## Usage

```rust
use astrelis_ui::{UiSystem, Color};

let mut ui = UiSystem::new(graphics_context);

ui.build(|root| {
    root.container()
        .width(300.0)
        .height(200.0)
        .background_color(Color::WHITE)
        .child(
            root.button("Click Me")
                .on_click(|| println!("Clicked!"))
        );
});

// In render loop
ui.render(&mut render_pass);
```

## Modules

### `widgets`

Contains built-in widget implementations:
- `Container`: Generic layout container.
- `Text`: Text display.
- `Button`: Interactive button.
- `TextInput`: Editable text field.

### `builder`

- `UiBuilder`: Context for building the UI tree.
- `WidgetBuilder`: Fluent API for configuring widgets.

### `renderer`

- `UiRenderer`: Handles GPU resource management and drawing of the UI tree.

### `tree`

- `UiTree`: The core data structure storing widgets and layout information.

### `event`

- `UiEventSystem`: Handles event dispatch, hit testing, and focus management.
