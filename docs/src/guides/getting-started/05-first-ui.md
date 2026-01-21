# First UI

In this guide, you'll create your first interactive UI with Astrelis. We'll build a simple counter app with buttons and text, demonstrating the declarative UI system and event handling.

## Prerequisites

- Completed [Rendering Fundamentals](04-rendering-fundamentals.md)
- Understanding of Flexbox layout (helpful but not required)

## What You'll Build

A counter app with:
- A text display showing the current count
- Buttons to increment, decrement, and reset
- Styled with colors, padding, and borders
- Interactive click handlers

## The Complete Example

**`Cargo.toml`** additions:
```toml
[dependencies]
astrelis-core = { git = "..." }
astrelis-winit = { git = "..." }
astrelis-render = { git = "..." }
astrelis-ui = { git = "..." }      # Add UI
astrelis-text = { git = "..." }    # Required by UI
glam = "0.29"
taffy = "0.5"  # For Flexbox layout types
```

**`src/main.rs`**:
```rust
use std::sync::Arc;
use astrelis_core::logging;
use astrelis_render::{Color, GraphicsContext, RenderTarget, RenderableWindow};
use astrelis_ui::{UiSystem, WidgetId};
use astrelis_winit::{
    WindowId, FrameTime,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::WindowDescriptor,
};

struct CounterApp {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
    count: i32,
    counter_text_id: WidgetId,
}

impl App for CounterApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        // Update UI animations
        self.ui.update(time.delta_seconds());

        // Update counter text display
        self.ui.update_text(&self.counter_text_id, format!("Count: {}", self.count));
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle resize
        events.dispatch(|event| {
            use astrelis_winit::event::{Event, HandleStatus};
            if let Event::WindowResized(size) = event {
                self.window.resized(*size);
                self.ui.set_viewport(self.window.viewport());
                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        // Process UI events (clicks, hovers, keyboard)
        self.ui.process_events(events);

        // Render
        let mut frame = self.window.begin_drawing();

        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb(0.1, 0.1, 0.15),
            |pass| {
                self.ui.render(pass.descriptor());
            },
        );

        frame.finish();
    }
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics = GraphicsContext::new_owned_sync_or_panic();

        let window = ctx
            .create_window(&WindowDescriptor {
                title: "Counter App".to_string(),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window_id = window.id();
        let renderable = RenderableWindow::new(window, graphics.clone());

        // Create UI system
        let mut ui = UiSystem::new(graphics.clone());
        ui.set_viewport(renderable.viewport());

        // Build UI
        let counter_text_id = build_ui(&mut ui);

        Box::new(CounterApp {
            graphics,
            window: renderable,
            window_id,
            ui,
            count: 0,
            counter_text_id,
        })
    });
}

fn build_ui(ui: &mut UiSystem) -> WidgetId {
    let counter_text_id = WidgetId::new("counter_text");

    ui.build(|root| {
        // Main container
        root.container()
            .width(800.0)
            .height(600.0)
            .padding(20.0)
            .background_color(Color::from_rgb(0.1, 0.1, 0.15))
            .child(|parent| {
                // Center column
                parent
                    .column()
                    .gap(20.0)
                    .justify_content(taffy::JustifyContent::Center)
                    .align_items(taffy::AlignItems::Center)
                    .child(|parent| {
                        // Title
                        parent
                            .text("Counter App")
                            .size(24.0)
                            .color(Color::WHITE)
                            .bold()
                            .build()
                    })
                    .child(|parent| {
                        // Counter display
                        parent
                            .container()
                            .background_color(Color::from_rgb(0.2, 0.2, 0.3))
                            .border_color(Color::from_rgb(0.4, 0.4, 0.6))
                            .border_width(2.0)
                            .border_radius(8.0)
                            .padding(20.0)
                            .child(|parent| {
                                parent
                                    .text("Count: 0")
                                    .id(counter_text_id)
                                    .size(32.0)
                                    .color(Color::from_rgb(0.4, 0.8, 1.0))
                                    .bold()
                                    .build()
                            })
                            .build()
                    })
                    .child(|parent| {
                        // Button row
                        parent
                            .row()
                            .gap(10.0)
                            .child(|parent| {
                                parent
                                    .button("-")
                                    .background_color(Color::from_rgb(0.8, 0.3, 0.3))
                                    .hover_color(Color::from_rgb(1.0, 0.4, 0.4))
                                    .padding(15.0)
                                    .font_size(20.0)
                                    .on_click(|| {
                                        println!("Decrement clicked!");
                                    })
                                    .build()
                            })
                            .child(|parent| {
                                parent
                                    .button("Reset")
                                    .background_color(Color::from_rgb(0.4, 0.4, 0.5))
                                    .hover_color(Color::from_rgb(0.5, 0.5, 0.6))
                                    .padding(15.0)
                                    .font_size(20.0)
                                    .on_click(|| {
                                        println!("Reset clicked!");
                                    })
                                    .build()
                            })
                            .child(|parent| {
                                parent
                                    .button("+")
                                    .background_color(Color::from_rgb(0.3, 0.7, 0.3))
                                    .hover_color(Color::from_rgb(0.4, 0.8, 0.4))
                                    .padding(15.0)
                                    .font_size(20.0)
                                    .on_click(|| {
                                        println!("Increment clicked!");
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .build()
            })
            .build();
    });

    counter_text_id
}
```

This example has placeholder click handlers. We'll fix this in the next section!

## Breaking It Down

### 1. UI System Initialization

```rust
let mut ui = UiSystem::new(graphics.clone());
ui.set_viewport(renderable.viewport());
```

**`UiSystem::new()`**: Creates the UI system with GPU rendering capabilities.

**`set_viewport()`**: Tells the UI system the window size for layout calculations.

### 2. Declarative UI Building

Astrelis UI uses a **builder pattern** with **closures** for hierarchy:

```rust
ui.build(|root| {
    root.container()
        .width(800.0)
        .height(600.0)
        .child(|parent| {
            parent.text("Hello").build()
        })
        .build();
});
```

**Pattern**:
1. Call widget method (`container()`, `text()`, `button()`)
2. Set properties (`.width()`, `.color()`, `.padding()`)
3. Add children with `.child(|parent| { ... })`
4. Finalize with `.build()`

**Why closures?**: They establish parent-child relationships automatically.

### 3. Layout with Flexbox

Astrelis uses **Taffy** (a Flexbox/Grid layout engine):

```rust
parent
    .column()  // Vertical layout
    .gap(20.0)  // Space between children
    .justify_content(taffy::JustifyContent::Center)  // Center vertically
    .align_items(taffy::AlignItems::Center)  // Center horizontally
    .child(|parent| { /* ... */ })
    .build()
```

**Common layouts**:
- **`.column()`**: Vertical stack (like CSS `flex-direction: column`)
- **`.row()`**: Horizontal stack (like CSS `flex-direction: row`)
- **`.container()`**: Single-child wrapper (like a `<div>`)

**Flexbox properties**:
- **`.justify_content()`**: Align along main axis (vertical for column, horizontal for row)
- **`.align_items()`**: Align along cross axis
- **`.gap()`**: Space between children
- **`.padding()`**: Inner spacing
- **`.margin()`**: Outer spacing

### 4. Widget Types

#### Text

```rust
parent
    .text("Hello, World!")
    .size(24.0)  // Font size
    .color(Color::WHITE)
    .bold()  // Bold weight
    .build()
```

#### Button

```rust
parent
    .button("Click Me")
    .background_color(Color::from_rgb(0.3, 0.7, 0.3))
    .hover_color(Color::from_rgb(0.4, 0.8, 0.4))  // Color on hover
    .padding(15.0)
    .font_size(20.0)
    .on_click(|| {
        println!("Clicked!");
    })
    .build()
```

#### Container

```rust
parent
    .container()
    .background_color(Color::from_rgb(0.2, 0.2, 0.3))
    .border_color(Color::from_rgb(0.4, 0.4, 0.6))
    .border_width(2.0)
    .border_radius(8.0)  // Rounded corners
    .padding(20.0)
    .child(|parent| { /* ... */ })
    .build()
```

### 5. Widget IDs for Updates

To update widgets later, assign them IDs:

```rust
let counter_text_id = WidgetId::new("counter_text");

// During build
parent
    .text("Count: 0")
    .id(counter_text_id)
    .build()

// Later, in update()
self.ui.update_text(&counter_text_id, format!("Count: {}", self.count));
```

**Why IDs?**: Enables **incremental updates** without rebuilding the entire tree.

### 6. Event Handling

#### Processing Events

Call `process_events()` in `render()`:

```rust
fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    // ... handle resize ...

    // Process UI events (IMPORTANT!)
    self.ui.process_events(events);

    // ... render UI ...
}
```

This dispatches mouse/keyboard events to widgets.

#### Click Handlers

Buttons take closures as click handlers:

```rust
.on_click(|| {
    println!("Button clicked!");
})
```

**Problem**: Closures can't capture mutable state directly. Solution: Use shared state with `Arc<RwLock<T>>` or message channels.

### 7. Rendering the UI

```rust
frame.clear_and_render(
    RenderTarget::Surface,
    Color::from_rgb(0.1, 0.1, 0.15),
    |pass| {
        self.ui.render(pass.descriptor());
    },
);
```

The UI system batches all widgets into efficient GPU draw calls.

## Making Buttons Work with State

The example above has non-functional buttons. Here's how to fix it with shared state:

```rust
use std::sync::{Arc, RwLock};

#[derive(Clone)]
struct CounterState {
    count: Arc<RwLock<i32>>,
}

impl CounterState {
    fn new() -> Self {
        Self {
            count: Arc::new(RwLock::new(0)),
        }
    }

    fn get(&self) -> i32 {
        *self.count.read().unwrap()
    }

    fn increment(&self) {
        *self.count.write().unwrap() += 1;
    }

    fn decrement(&self) {
        *self.count.write().unwrap() -= 1;
    }

    fn reset(&self) {
        *self.count.write().unwrap() = 0;
    }
}
```

Update `build_ui()` to accept and clone state:

```rust
fn build_ui(ui: &mut UiSystem, state: &CounterState) -> WidgetId {
    let counter_text_id = WidgetId::new("counter_text");
    let count = state.get();

    // Clone state for each button
    let state_inc = state.clone();
    let state_dec = state.clone();
    let state_reset = state.clone();

    ui.build(|root| {
        // ... layout ...
        .child(|parent| {
            // Decrement button
            parent
                .button("-")
                .on_click(move || {
                    state_dec.decrement();
                })
                .build()
        })
        .child(|parent| {
            // Reset button
            parent
                .button("Reset")
                .on_click(move || {
                    state_reset.reset();
                })
                .build()
        })
        .child(|parent| {
            // Increment button
            parent
                .button("+")
                .on_click(move || {
                    state_inc.increment();
                })
                .build()
        })
        // ...
    });

    counter_text_id
}
```

Update `CounterApp`:

```rust
struct CounterApp {
    // ... other fields ...
    ui: UiSystem,
    state: CounterState,  // Add state
    counter_text_id: WidgetId,
}

impl App for CounterApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        self.ui.update(time.delta_seconds());

        // Read current count and update display
        let count = self.state.get();
        self.ui.update_text(&self.counter_text_id, format!("Count: {}", count));
    }

    // ... render ...
}
```

In `main()`:

```rust
let state = CounterState::new();
let counter_text_id = build_ui(&mut ui, &state);

Box::new(CounterApp {
    // ...
    ui,
    state,
    counter_text_id,
})
```

Now the buttons work!

## Styling and Colors

### Color Creation

```rust
// RGB (0.0 to 1.0)
Color::from_rgb(0.2, 0.5, 0.8)

// RGB with u8 (0 to 255)
Color::from_rgb_u8(50, 128, 200)

// RGBA with alpha
Color::rgba(1.0, 0.0, 0.0, 0.5)  // Semi-transparent red

// Named colors
Color::WHITE
Color::BLACK
Color::RED
Color::GREEN
Color::BLUE
```

### Common Style Properties

```rust
.background_color(Color::from_rgb(0.2, 0.2, 0.3))  // Background
.color(Color::WHITE)  // Text color
.border_color(Color::from_rgb(0.4, 0.4, 0.6))  // Border color
.border_width(2.0)  // Border thickness
.border_radius(8.0)  // Rounded corners
.padding(20.0)  // Inner spacing (all sides)
.margin(10.0)  // Outer spacing
.width(200.0)  // Fixed width
.height(100.0)  // Fixed height
.min_width(50.0)  // Minimum width
.min_height(30.0)  // Minimum height
```

### Hover States

Buttons support hover colors:

```rust
.button("Click")
    .background_color(Color::from_rgb(0.3, 0.7, 0.3))
    .hover_color(Color::from_rgb(0.4, 0.8, 0.4))  // Lighter on hover
```

The UI system automatically interpolates between colors.

## Layout Examples

### Centered Content

```rust
root.column()
    .justify_content(taffy::JustifyContent::Center)  // Center vertically
    .align_items(taffy::AlignItems::Center)  // Center horizontally
    .child(|parent| {
        parent.text("Centered!").build()
    })
    .build()
```

### Horizontal Button Row

```rust
root.row()
    .gap(10.0)
    .child(|parent| { parent.button("Left").build() })
    .child(|parent| { parent.button("Middle").build() })
    .child(|parent| { parent.button("Right").build() })
    .build()
```

### Vertical List

```rust
root.column()
    .gap(5.0)
    .child(|parent| { parent.text("Item 1").build() })
    .child(|parent| { parent.text("Item 2").build() })
    .child(|parent| { parent.text("Item 3").build() })
    .build()
```

## Performance: Incremental Updates

Astrelis UI uses **dirty flags** for efficiency:

```rust
// Fast: Only updates text (< 1ms)
self.ui.update_text(&text_id, "New text");

// Fast: Only updates color (< 0.1ms)
self.ui.update_color(&text_id, Color::RED);

// Slow: Rebuilds entire tree (~20ms)
let text_id = build_ui(&mut self.ui, &self.state);
```

**Best practice**: Build UI once in `main()`, update with `update_text()` and `update_color()` in `update()`.

## Common Issues

### Buttons Not Responding

**Problem**: Clicks do nothing.

**Cause**: Forgot to call `self.ui.process_events(events)` in `render()`.

**Fix**: Always call `process_events()` before rendering.

### UI Not Visible

**Problem**: Window is blank.

**Causes**:
1. Forgot to call `self.ui.render(pass.descriptor())`
2. Viewport not set: `ui.set_viewport(window.viewport())`
3. Widget sizes are zero (no width/height set)

**Fix**: Check all three points.

### Text Doesn't Update

**Problem**: `update_text()` doesn't change displayed text.

**Cause**: Wrong `WidgetId` or widget isn't a text widget.

**Fix**: Ensure the ID matches and the widget was created with `.text()`.

### Layout Looks Wrong

**Problem**: Widgets overlap or are positioned incorrectly.

**Causes**:
1. Missing `.build()` call on widgets
2. Flexbox properties conflict
3. Fixed sizes too large for container

**Fix**: Check for missing `.build()` calls and review Flexbox properties.

## Next Steps

You've built your first interactive UI! Next:

1. **[Incremental Updates](06-incremental-updates.md)** - Master efficient UI updates
2. **[Custom Widgets Guide](../../ui/custom-widgets.md)** (Phase 2) - Create reusable widgets
3. **[Layout Deep Dive](../../ui/layout-deep-dive.md)** (Phase 2) - Advanced layouts

## Complete Working Example

See the `crates/astrelis-ui/examples/counter.rs` in the Astrelis repository for a complete, polished version with:
- Theme support (dark/light)
- Smooth animations
- Focus navigation with Tab key
- Performance profiling integration

Run it with:
```bash
cargo run -p astrelis-ui --example counter
```

Congratulations on building your first Astrelis UI! 🎉
