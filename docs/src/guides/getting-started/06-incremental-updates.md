# Incremental Updates

One of Astrelis UI's most powerful features is its **incremental update system** with **fine-grained dirty flags**. Instead of rebuilding your entire UI every frame, you can update just what changed - achieving 100-200x performance improvements.

In this guide, you'll learn:
- When to rebuild vs update
- The dirty flag system
- Update methods for different widget properties
- Performance best practices

## Prerequisites

- Completed [First UI](05-first-ui.md)
- Understanding of the counter example

## The Problem: Full Rebuilds Are Expensive

When you call `ui.build()`, Astrelis:
1. **Destroys** the old widget tree
2. **Creates** new widgets from scratch
3. **Shapes text** with cosmic-text (~5-10ms per text widget)
4. **Computes layout** with Taffy (~10-20ms for complex layouts)
5. **Generates geometry** for rendering (~2-5ms)

**Total: 20-50ms** for a moderately complex UI.

At 60 FPS, you only have **16.67ms** per frame!

## The Solution: Incremental Updates

Instead of rebuilding, **mark specific widgets dirty** and update only what changed:

```rust
// Bad: Full rebuild (20-50ms)
let counter_text_id = build_ui(&mut self.ui, &self.state);

// Good: Incremental update (<1ms)
self.ui.update_text(&counter_text_id, format!("Count: {}", self.count));
```

**Performance**: ~200x faster for text updates!

## Dirty Flag System

Astrelis tracks changes with **bitflags**:

```rust
pub struct DirtyFlags {
    COLOR_ONLY,      // Color changed (fastest)
    TEXT_SHAPING,    // Text content changed
    LAYOUT,          // Size/position changed
    GEOMETRY,        // Borders/radius changed
}
```

### Dirty Flag Performance

| Flag | Operation | Time | Example |
|------|-----------|------|---------|
| `COLOR_ONLY` | Skip text shaping, skip layout, skip geometry | **~0.1ms** | Color animation |
| `TEXT_SHAPING` | Reshape text, skip layout | **~5ms** | Update label text |
| `LAYOUT` | Recompute layout for dirty subtree | **~10ms** | Resize widget |
| `GEOMETRY` | Rebuild borders/radius | **~2ms** | Change border |
| (none) | Skip everything | **~0ms** | No changes |

**Key insight**: The more you can skip, the faster the update.

## Update Methods

### update_text()

Updates text content (sets `TEXT_SHAPING` flag):

```rust
let text_id = WidgetId::new("label");

// During build
parent
    .text("Count: 0")
    .id(text_id)
    .build()

// Later
self.ui.update_text(&text_id, format!("Count: {}", self.count));
```

**Performance**: ~5ms (reshapes text, but skips layout if size doesn't change)

**When to use**: Updating counters, labels, scores, timers, etc.

### update_color()

Updates widget color (sets `COLOR_ONLY` flag):

```rust
let text_id = WidgetId::new("health");

// Update color based on health
if health < 20 {
    self.ui.update_color(&text_id, Color::RED);
} else {
    self.ui.update_color(&text_id, Color::WHITE);
}
```

**Performance**: ~0.1ms (fastest update!)

**When to use**: Color animations, state indicators, health bars.

### update_size()

Updates widget dimensions (sets `LAYOUT` flag):

```rust
let container_id = WidgetId::new("container");

self.ui.update_size(&container_id, Size::new(200.0, 100.0));
```

**Performance**: ~10ms (recomputes layout for affected subtree)

**When to use**: Responsive layouts, window resizing, collapsible panels.

### Multiple Updates

You can chain multiple updates:

```rust
self.ui.update_text(&label_id, "Warning!");
self.ui.update_color(&label_id, Color::YELLOW);
```

The UI system coalesces flags efficiently.

## When to Rebuild vs Update

### Rebuild When:

1. **Window resized** - Layout needs full recomputation
2. **Theme changed** - All colors change
3. **UI structure changes** - Adding/removing widgets
4. **Initial setup** - First time creating UI

```rust
fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    events.dispatch(|event| {
        if let Event::WindowResized(size) = event {
            self.window.resized(*size);
            self.ui.set_viewport(self.window.viewport());

            // Rebuild for new viewport
            self.counter_text_id = build_ui(&mut self.ui, &self.state);

            HandleStatus::consumed()
        } else {
            HandleStatus::ignored()
        }
    });

    // ... render ...
}
```

### Update When:

1. **Text content changes** - Counter increments, timer updates
2. **Colors change** - Hover states, health bars, animations
3. **Individual widget properties change** - Size, visibility, etc.

```rust
fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
    // Fast incremental updates
    let count = self.state.get();
    self.ui.update_text(&self.counter_text_id, format!("Count: {}", count));

    // Color animation based on count
    let color = if count > 10 {
        Color::from_rgb(1.0, 0.5, 0.0)  // Orange
    } else {
        Color::from_rgb(0.4, 0.8, 1.0)  // Blue
    };
    self.ui.update_color(&self.counter_text_id, color);
}
```

## Practical Example: Animated Health Bar

Let's build a health bar that updates efficiently:

```rust
struct HealthBarApp {
    // ... other fields ...
    health_text_id: WidgetId,
    health_bar_id: WidgetId,
    current_health: f32,
    max_health: f32,
}

fn build_health_bar(ui: &mut UiSystem, health: f32, max_health: f32) -> (WidgetId, WidgetId) {
    let health_text_id = WidgetId::new("health_text");
    let health_bar_id = WidgetId::new("health_bar");

    ui.build(|root| {
        root.column()
            .gap(10.0)
            .child(|parent| {
                // Health text
                parent
                    .text(format!("Health: {:.0}/{:.0}", health, max_health))
                    .id(health_text_id)
                    .size(18.0)
                    .color(Color::WHITE)
                    .build()
            })
            .child(|parent| {
                // Health bar background
                parent
                    .container()
                    .width(200.0)
                    .height(30.0)
                    .background_color(Color::from_rgb(0.2, 0.2, 0.2))
                    .border_width(2.0)
                    .border_color(Color::from_rgb(0.5, 0.5, 0.5))
                    .child(|parent| {
                        // Health bar fill
                        let width = (health / max_health) * 200.0;
                        parent
                            .container()
                            .id(health_bar_id)
                            .width(width)
                            .height(30.0)
                            .background_color(Color::from_rgb(0.0, 0.8, 0.0))
                            .build()
                    })
                    .build()
            })
            .build();
    });

    (health_text_id, health_bar_id)
}

impl App for HealthBarApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        // Simulate damage over time
        self.current_health -= 5.0 * time.delta_seconds();
        self.current_health = self.current_health.max(0.0);

        // Fast text update (<1ms)
        self.ui.update_text(
            &self.health_text_id,
            format!("Health: {:.0}/{:.0}", self.current_health, self.max_health),
        );

        // Fast size update (~10ms, only affects health bar)
        let width = (self.current_health / self.max_health) * 200.0;
        self.ui.update_size(&self.health_bar_id, Size::new(width, 30.0));

        // Color update based on health (<0.1ms)
        let color = if self.current_health < 30.0 {
            Color::from_rgb(0.8, 0.0, 0.0)  // Red (danger)
        } else if self.current_health < 60.0 {
            Color::from_rgb(0.8, 0.8, 0.0)  // Yellow (warning)
        } else {
            Color::from_rgb(0.0, 0.8, 0.0)  // Green (healthy)
        };
        self.ui.update_color(&self.health_bar_id, color);
    }

    // ... render ...
}
```

**Performance**: ~10ms per frame (vs ~30ms if rebuilding every frame)

## Text Shaping Cache

Astrelis caches shaped text in `Arc<ShapedTextData>`:

```rust
// First time: Shape text (~5ms)
self.ui.update_text(&text_id, "Hello");

// Second time with same text: Use cache (~0ms)
self.ui.update_text(&text_id, "Hello");

// Different text: Reshape (~5ms)
self.ui.update_text(&text_id, "World");
```

**Optimization**: If text doesn't change, don't call `update_text()` at all!

```rust
fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
    let new_count = self.state.get();

    // Only update if changed
    if new_count != self.last_count {
        self.ui.update_text(&self.counter_text_id, format!("Count: {}", new_count));
        self.last_count = new_count;
    }
}
```

## Layout Dirty Propagation

When a widget is marked dirty, Astrelis propagates flags **up the tree**:

```
Root (LAYOUT)
  ↑
Container (LAYOUT)
  ↑
Text (TEXT_SHAPING) ← Updated here
```

**Why?**: Changing text size might affect parent container size.

**Optimization**: Layout is only recomputed for the **dirty subtree**, not the entire tree.

## Performance Profiling

Use `puffin` to see dirty flag performance:

```rust
use astrelis_core::profiling::{init_profiling, ProfilingBackend, new_frame};

fn main() {
    init_profiling(ProfilingBackend::PuffinHttp);
    // ... run app ...
}

impl App for MyApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        new_frame();  // Mark new profiling frame
        // ... updates ...
    }
}
```

Open `http://127.0.0.1:8585` in your browser to see the puffin viewer.

**Look for**:
- `ui_update`: Should be <1ms for incremental updates
- `text_shaping`: Only appears when text changes
- `layout_compute`: Only appears when layout is dirty

## Anti-Patterns to Avoid

### 1. Rebuilding Every Frame

```rust
// BAD: 30ms per frame
fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
    self.text_id = build_ui(&mut self.ui, &self.state);
}

// GOOD: <1ms per frame
fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
    let text = format!("Count: {}", self.state.get());
    self.ui.update_text(&self.text_id, text);
}
```

### 2. Updating Unchanged Values

```rust
// BAD: Updates every frame even if unchanged
fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
    self.ui.update_text(&self.fps_text, format!("FPS: {}", self.fps));
}

// GOOD: Only update when changed
fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
    let new_fps = calculate_fps(time);
    if new_fps != self.last_fps {
        self.ui.update_text(&self.fps_text, format!("FPS: {}", new_fps));
        self.last_fps = new_fps;
    }
}
```

### 3. Using update_text() for Color-Only Changes

```rust
// BAD: Triggers text reshaping (~5ms)
fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
    if self.is_danger {
        self.ui.update_text(&self.text_id, "DANGER!");  // Already says "DANGER!"
    }
}

// GOOD: Just update color (~0.1ms)
fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
    let color = if self.is_danger { Color::RED } else { Color::WHITE };
    self.ui.update_color(&self.text_id, color);
}
```

## Best Practices

### 1. Build Once, Update Many

```rust
fn main() {
    // Build UI once during initialization
    let text_id = build_ui(&mut ui);

    // Update hundreds of times per second
    // (in the update() method)
}
```

### 2. Cache Previous Values

```rust
struct MyApp {
    text_id: WidgetId,
    last_value: i32,  // Cache
}

impl App for MyApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        let current_value = self.calculate_value();

        if current_value != self.last_value {
            self.ui.update_text(&self.text_id, format!("{}", current_value));
            self.last_value = current_value;
        }
    }
}
```

### 3. Use Color Updates for Animations

```rust
fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
    // Smooth color animation (60 FPS possible!)
    let t = (time.elapsed_seconds() * 2.0).sin() * 0.5 + 0.5;
    let color = Color::from_rgb(t, 0.5, 1.0 - t);
    self.ui.update_color(&self.text_id, color);
}
```

### 4. Minimize Layout Updates

Layout updates are expensive. Avoid:

```rust
// BAD: Layout recomputes every frame
fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
    self.ui.update_size(&self.container_id, Size::new(200.0 + time.elapsed_seconds().sin() * 10.0, 100.0));
}
```

Unless absolutely necessary for the effect.

## Comparison to Other Engines

### Unity UGUI

Unity rebuilds Canvas when needed:

```csharp
// Unity: Canvas rebuild is automatic but expensive
text.text = $"Count: {count}";  // Triggers canvas rebuild
```

Astrelis gives you control over what gets recomputed.

### React/Web

React uses virtual DOM diffing:

```jsx
// React: Diffs virtual DOM, updates real DOM
<div>{count}</div>
```

Astrelis's dirty flags are **more granular** than virtual DOM diffing.

### Immediate-Mode (Dear ImGui, egui)

Immediate-mode rebuilds every frame:

```rust
// egui: Rebuild entire UI every frame
ui.label(format!("Count: {}", count));
```

Astrelis's retained-mode UI is **faster for complex UIs** that don't change every frame.

## Summary

**Key takeaways**:
- **Rebuild rarely**: Only on structure changes, theme changes, or window resize
- **Update frequently**: Use `update_text()`, `update_color()`, `update_size()`
- **Cache values**: Don't update if value hasn't changed
- **Use the right update**: Color-only changes should use `update_color()`, not `update_text()`
- **Profile**: Use puffin to verify performance

**Performance hierarchy** (fastest to slowest):
1. **No update** (~0ms) - Best!
2. **`update_color()`** (~0.1ms) - Color changes
3. **`update_text()`** (~5ms) - Text content changes
4. **`update_size()`** (~10ms) - Layout changes
5. **Rebuild** (~20-50ms) - Slowest, avoid in update()

With incremental updates, you can build complex, interactive UIs that run smoothly at 60+ FPS!

## Next Steps

You've mastered the fundamentals of Astrelis! Continue learning:

1. **[Asset Loading Guide](../asset-system/loading-assets.md)** (Phase 4) - Load textures and fonts
2. **[Custom Widgets Guide](../ui/custom-widgets.md)** (Phase 2) - Create reusable components
3. **[Performance Tuning Guide](../advanced/performance-tuning.md)** (Phase 5) - Advanced optimization

Congratulations on completing the Getting Started guides! 🚀
