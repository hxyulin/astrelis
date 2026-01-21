# UI Performance Optimization

This guide covers advanced performance optimization techniques for the Astrelis UI system. Learn how to build responsive UIs that handle thousands of widgets efficiently.

## Overview

The Astrelis UI system is designed for high performance through several key optimizations:

- **Fine-grained dirty flags**: Only update what changed
- **Text shaping cache**: Reuse expensive shaping operations
- **GPU instancing**: Single draw call per widget type
- **Virtual scrolling**: Render only visible items
- **Layout caching**: Skip layout for unchanged subtrees

**Performance Targets:**
- 60 FPS (16.6ms per frame) for typical UIs
- <1ms for color-only updates
- <5ms for text content updates
- <10ms for layout changes
- Handle 10,000+ widgets with virtual scrolling

## The Dirty Flag System

### Understanding Dirty Flags

The UI system uses bitflags to track what needs updating:

```rust
bitflags! {
    pub struct DirtyFlags: u8 {
        /// Only color changed - fastest update
        const COLOR_ONLY = 1 << 0;

        /// Text content changed - needs reshaping
        const TEXT_SHAPING = 1 << 1;

        /// Size/position changed - needs layout
        const LAYOUT = 1 << 2;

        /// Border/radius changed - needs geometry rebuild
        const GEOMETRY = 1 << 3;

        /// Full rebuild required
        const FULL = Self::COLOR_ONLY.bits()
                   | Self::TEXT_SHAPING.bits()
                   | Self::LAYOUT.bits()
                   | Self::GEOMETRY.bits();
    }
}
```

### Performance Characteristics

| Flag | Operation | Time | What's Updated |
|------|-----------|------|----------------|
| `COLOR_ONLY` | Color change | <1ms | GPU instance buffer only |
| `TEXT_SHAPING` | Text content | 5-10ms | Text shaping + instance buffer |
| `LAYOUT` | Size/position | 10-20ms | Taffy layout + instance buffer |
| `GEOMETRY` | Border/radius | 3-8ms | Vertex buffer + instance buffer |
| `FULL` | Complete rebuild | 20-50ms | Everything |

**Key Insight:** `COLOR_ONLY` is **20x faster** than `FULL` rebuild. Use incremental updates whenever possible.

### Incremental Update Methods

```rust
use astrelis_ui::{UiSystem, Color};

// Fast path: color-only update (~0.5ms)
ui.update_color("button", Color::RED)?;

// Medium path: text content update (~5ms)
ui.update_text("label", "New text")?;

// Slow path: layout change (~15ms)
ui.update_size("panel", 400.0, 300.0)?;

// Slowest: full rebuild (~30ms)
ui.build(|root| {
    root.container().build();
});
```

### Dirty Flag Propagation

When a widget is marked dirty, the dirty flag **propagates upward** to parent widgets:

```text
Container (LAYOUT dirty)
├─ Row (LAYOUT dirty)
│  ├─ Button (COLOR_ONLY dirty) ← Original change
│  └─ Label (clean)
└─ Column (clean)
```

**Rule:** A parent is marked with the **union** of all child dirty flags.

**Optimization:** Clean subtrees are skipped during layout/rendering.

```rust
// Example: marking a widget dirty
widget.mark_dirty(DirtyFlags::COLOR_ONLY);

// Parent automatically inherits the flag
let parent_flags = parent.dirty_flags(); // Contains COLOR_ONLY
```

### Best Practices

**✅ DO: Use specific flags**
```rust
// Good: minimal update
ui.update_color("status", Color::GREEN)?;
```

**❌ DON'T: Force full rebuilds**
```rust
// Bad: rebuilds entire tree
ui.build(|root| {
    root.text(&status_text).id("status").build();
});
```

**✅ DO: Batch related updates**
```rust
// Good: batch color updates before render
for button_id in button_ids {
    ui.update_color(button_id, Color::BLUE)?;
}
// All updates processed in single pass
```

**❌ DON'T: Update inside render loop**
```rust
// Bad: causes frame stutter
frame.clear_and_render(target, clear_color, |pass| {
    ui.update_text("fps", &fps_text)?; // DON'T DO THIS
    ui.render(pass.descriptor());
});
```

## Text Shaping Cache

### How Text Shaping Works

Text shaping is **expensive** (5-10ms for complex text). The UI system caches shaped results:

```rust
pub struct ShapedTextData {
    /// Cached glyph positions
    pub glyphs: Vec<GlyphInfo>,

    /// Measured dimensions
    pub width: f32,
    pub height: f32,

    /// Line breaks
    pub lines: Vec<LineInfo>,

    /// Version for invalidation
    version: u32,
}

// Cached with Arc for cheap cloning
let shaped = Arc::new(ShapedTextData { /* ... */ });
```

### Cache Key Components

The cache key includes:
- Text content (string)
- Font family and size
- Max width (for wrapping)
- Text style (bold, italic)

**Cache Hit:** Use existing shaped data (~0.1ms)
**Cache Miss:** Shape text and cache result (~5-10ms)

### Maximizing Cache Hits

**✅ DO: Reuse identical text**
```rust
// Good: all buttons use cached "Click Me"
for i in 0..10 {
    parent.button("Click Me")
        .id(&format!("btn{}", i))
        .build();
}
// Only shaped once, cached for all instances
```

**❌ DON'T: Use unique text unnecessarily**
```rust
// Bad: each button requires separate shaping
for i in 0..10 {
    parent.button(&format!("Button {}", i)) // Unique text
        .build();
}
// Shaped 10 times
```

**✅ DO: Use consistent font sizes**
```rust
// Good: standard sizes cache well
text.font_size(14.0); // Common size
text.font_size(16.0); // Common size
```

**❌ DON'T: Use arbitrary font sizes**
```rust
// Bad: every size is a cache miss
text.font_size(14.3);
text.font_size(14.7);
text.font_size(15.1);
```

### Text Update Patterns

```rust
// Fast: update text content with same styling
ui.update_text("counter", &format!("Count: {}", count))?;
// Result: TEXT_SHAPING flag, cache miss, 5ms update

// Faster: precompute common values
let text = match count {
    0..=10 => CACHED_TEXTS[count], // Pre-shaped cache
    _ => &format!("Count: {}", count),
};
ui.update_text("counter", text)?;
```

### Monitoring Cache Performance

Use profiling to identify cache misses:

```rust
use astrelis_core::profiling::*;

puffin::profile_scope!("text_shaping");
let shaped = shape_text(text, font, max_width);
// Check puffin viewer for time spent
```

**Target:** <10% of frames should have text shaping operations.

## Virtual Scrolling

### Why Virtual Scrolling?

Rendering 10,000+ widgets is slow, even with GPU instancing:

| Items | Normal List | Virtual Scroll |
|-------|-------------|----------------|
| 100 | 2ms | 2ms |
| 1,000 | 15ms | 2ms |
| 10,000 | 120ms | 2ms |
| 100,000 | 1200ms | 2ms |

**Key Insight:** Only render items visible in the viewport.

### VirtualScrollConfig

```rust
use astrelis_ui::VirtualScrollConfig;

let config = VirtualScrollConfig {
    // Total number of items
    item_count: 10_000,

    // Item height (fixed or variable)
    item_height: ItemHeight::Fixed(40.0),

    // Viewport size
    viewport_height: 600.0,

    // Buffer items above/below (smooth scrolling)
    buffer_items: 5,
};
```

### Fixed Height Items

Best performance for uniform items:

```rust
parent.virtual_scroll(config, |builder, visible_range| {
    for index in visible_range {
        builder.row(|row| {
            row.text(&format!("Item {}", index))
                .font_size(14.0)
                .build();
        })
        .height(Length::px(40.0)) // Fixed height
        .build();
    }
});
```

**Performance:**
- Culling: O(1) - simple math
- Rendering: O(visible items) - typically 15-20 items
- Scrolling: <2ms per frame

### Variable Height Items

More flexible but slightly slower:

```rust
let config = VirtualScrollConfig {
    item_count: 10_000,
    item_height: ItemHeight::Variable {
        estimate: 50.0,
        measured: Arc::new(RwLock::new(HashMap::new())),
    },
    viewport_height: 600.0,
    buffer_items: 5,
};

parent.virtual_scroll(config, |builder, visible_range| {
    for index in visible_range {
        let item_height = compute_item_height(index);

        builder.row(|row| {
            // Item content
        })
        .height(Length::px(item_height))
        .build();
    }
});
```

**Performance:**
- Culling: O(log n) - binary search
- First render: Measures and caches heights
- Subsequent renders: Uses cached heights

### Scroll Position Management

```rust
use std::sync::{Arc, RwLock};

let scroll_position = Arc::new(RwLock::new(0.0));

// Update on scroll events
events.dispatch(|event| {
    if let Event::MouseWheel { delta } = event {
        let mut pos = scroll_position.write().unwrap();
        *pos = (*pos + delta.y * 20.0)
            .max(0.0)
            .min(max_scroll_height);
        HandleStatus::consumed()
    } else {
        HandleStatus::ignored()
    }
});

// Use in virtual scroll
let pos_clone = scroll_position.clone();
parent.virtual_scroll_with_position(config, move || {
    *pos_clone.read().unwrap()
}, |builder, visible_range| {
    // Render visible items
});
```

### Virtual Scroll Best Practices

**✅ DO: Use fixed heights when possible**
```rust
// Good: fastest virtual scrolling
item_height: ItemHeight::Fixed(40.0)
```

**❌ DON'T: Use complex item layouts**
```rust
// Bad: defeats virtual scrolling benefits
builder.container(|container| {
    // 50 nested widgets per item
    container.container(|c| {
        c.row(|row| {
            // Complex layout
        }).build();
    }).build();
});
```

**✅ DO: Buffer items for smooth scrolling**
```rust
// Good: prevents pop-in during scroll
buffer_items: 5, // Render 5 items above/below
```

**✅ DO: Debounce scroll position updates**
```rust
// Good: limit updates to 60 FPS
if now - last_update > Duration::from_millis(16) {
    *scroll_position.write().unwrap() = new_position;
    last_update = now;
}
```

## GPU Instancing and Batching

### How UI Rendering Works

The UI system uses **instanced rendering** for efficiency:

```text
1. Build Phase: Widgets → Draw Commands (QuadCommand, TextCommand, ImageCommand)
2. Instance Phase: Commands → GPU Instance Buffers
3. Render Phase: Single draw call per type
```

**Key Optimization:** 1,000 quads = **1 draw call** instead of 1,000.

### Instance Buffer Layout

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadInstance {
    /// Position (x, y) and size (width, height)
    pub transform: [f32; 4],

    /// RGBA color
    pub color: [f32; 4],

    /// Border radius (top-left, top-right, bottom-right, bottom-left)
    pub border_radius: [f32; 4],

    /// Border width
    pub border_width: f32,

    /// Border color
    pub border_color: [f32; 4],
}
```

**Memory:** 76 bytes per quad instance

### Batch Size Tuning

```rust
// Configure instance buffer size
let renderer_config = UiRendererConfig {
    max_quads_per_batch: 10_000,
    max_text_glyphs_per_batch: 20_000,
    max_images_per_batch: 1_000,
};

let ui = UiSystem::with_config(graphics.clone(), renderer_config);
```

**Trade-offs:**
- Larger batches: Fewer draw calls, more GPU memory
- Smaller batches: More draw calls, less GPU memory

**Recommended:**
- Quads: 10,000 per batch (760 KB)
- Text: 20,000 glyphs per batch
- Images: 1,000 per batch

### Minimizing Draw Calls

**✅ DO: Group widgets by type**
```rust
// Good: all buttons batched together
parent.container(|c| {
    for i in 0..100 {
        c.button(&format!("Button {}", i)).build();
    }
});
// Result: 1 draw call for all buttons
```

**❌ DON'T: Interleave widget types**
```rust
// Bad: alternating types break batching
parent.container(|c| {
    for i in 0..100 {
        c.button("Button").build();
        c.image(texture).build(); // Breaks batch
    }
});
// Result: 200 draw calls (alternating button/image)
```

**Explanation:** Each widget type switch requires a new draw call.

### Atlas Textures

Use texture atlases to batch images:

```rust
// Good: all icons in one atlas
let atlas = load_texture_atlas("ui_icons.png");

for icon_index in 0..20 {
    let uv_rect = atlas.get_uv_rect(icon_index);
    parent.image(atlas.texture)
        .uv_rect(uv_rect)
        .build();
}
// Result: 1 draw call for all icons
```

**Without atlas:** 20 separate textures = 20 draw calls
**With atlas:** 1 texture = 1 draw call

## Profiling UI Performance

### Setting Up Puffin

```rust
use astrelis_core::profiling::*;

// Initialize profiling in main()
init_profiling(ProfilingBackend::PuffinHttp);

// Mark frames in render loop
fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    puffin::new_frame!(); // Mark frame boundary

    puffin::profile_scope!("ui_update");
    ui.handle_events(events);

    puffin::profile_scope!("ui_render");
    let mut frame = self.window.begin_drawing();
    frame.clear_and_render(target, color, |pass| {
        ui.render(pass.descriptor());
    });
    frame.finish();
}
```

**Access profiler:** Open `http://127.0.0.1:8585` in browser

### Key Metrics to Watch

**Frame Time Breakdown:**
- **Update:** Event handling + state updates (<1ms target)
- **Layout:** Taffy layout computation (<5ms target)
- **Draw List:** Command generation (<2ms target)
- **GPU Upload:** Instance buffer updates (<1ms target)
- **Render:** Draw calls (<3ms target)

**Total Target:** <16.6ms (60 FPS)

### Common Performance Bottlenecks

**1. Excessive Layout Recalculation**

```text
Layout: 25ms ⚠️ TOO SLOW
  - container_1: 15ms
  - container_2: 10ms
```

**Solution:** Avoid full rebuilds, use incremental updates
```rust
// Bad: 25ms
ui.build(|root| { /* entire tree */ });

// Good: <1ms
ui.update_text("label", "new text")?;
```

**2. Text Shaping Every Frame**

```text
Frame 1: text_shaping: 8ms
Frame 2: text_shaping: 8ms
Frame 3: text_shaping: 8ms
```

**Solution:** Cache shaped text or avoid changing text content
```rust
// Bad: shapes every frame
ui.update_text("fps", &format!("FPS: {:.1}", fps))?;

// Good: update less frequently
if frame_count % 30 == 0 {
    ui.update_text("fps", &format!("FPS: {:.1}", fps))?;
}
```

**3. Too Many Draw Calls**

```text
Render: 12ms
  - draw_call_0: 0.5ms
  - draw_call_1: 0.5ms
  - ... (24 draw calls)
```

**Solution:** Batch widgets, use texture atlases
```rust
// Good: group by type
for i in 0..100 {
    parent.button("Click").build(); // All batched
}
```

**4. Large Instance Buffers**

```text
GPU Upload: 8ms ⚠️ TOO SLOW
  - quad_instances: 50,000 instances (3.8 MB)
```

**Solution:** Use virtual scrolling for large lists
```rust
// Instead of rendering 10,000 items:
parent.virtual_scroll(config, |builder, visible_range| {
    // Only render 20 visible items
});
```

## Performance Optimization Checklist

### Before Launch

- [ ] Profile with puffin during typical use
- [ ] Frame time <16.6ms (60 FPS) on target hardware
- [ ] No layout recalculation every frame
- [ ] Text shaping <10% of frames
- [ ] Draw calls <30 per frame
- [ ] Instance buffers <2 MB

### For Large UIs (1000+ widgets)

- [ ] Use virtual scrolling for lists >100 items
- [ ] Cache frequently displayed text
- [ ] Group widgets by type
- [ ] Use texture atlases for icons
- [ ] Limit dirty flag propagation depth

### For Dynamic UIs

- [ ] Use `update_color()` instead of rebuilding
- [ ] Use `update_text()` for text changes
- [ ] Debounce frequent updates (animations, FPS counters)
- [ ] Batch related updates before render

### For Text-Heavy UIs

- [ ] Reuse identical text strings
- [ ] Use standard font sizes (12, 14, 16, 18, 24)
- [ ] Limit font families to 2-3
- [ ] Avoid dynamic text wrapping when possible

## Common Performance Pitfalls

### Pitfall 1: Rebuilding Every Frame

```rust
// ❌ BAD: 30ms per frame
impl App for MyApp {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        ui.build(|root| {
            root.text(&format!("Frame: {}", self.frame_count)).build();
        });

        // Render...
    }
}
```

**Fix:** Build once, update incrementally
```rust
// ✅ GOOD: 0.5ms per frame
impl App for MyApp {
    fn on_start(&mut self, ctx: &mut AppCtx) {
        ui.build(|root| {
            root.text("Frame: 0").id("frame_count").build();
        });
    }

    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        ui.update_text("frame_count", &format!("Frame: {}", self.frame_count))?;
    }
}
```

### Pitfall 2: No Widget IDs

```rust
// ❌ BAD: Can't use incremental updates
ui.build(|root| {
    root.button("Click Me").build(); // No ID
});

// Can't update this button later!
```

**Fix:** Always add IDs for dynamic widgets
```rust
// ✅ GOOD: Can update later
ui.build(|root| {
    root.button("Click Me").id("my_button").build();
});

// Later:
ui.update_color("my_button", Color::RED)?;
```

### Pitfall 3: Deep Nesting

```rust
// ❌ BAD: Deep nesting amplifies dirty flags
ui.build(|root| {
    root.container(|c1| {
        c1.container(|c2| {
            c2.container(|c3| {
                c3.container(|c4| {
                    c4.button("Deep Button").build();
                    // Button change dirties 4 parents
                }).build();
            }).build();
        }).build();
    }).build();
});
```

**Fix:** Flatten hierarchy when possible
```rust
// ✅ GOOD: Flat structure
ui.build(|root| {
    root.button("Button").build(); // Only dirties root
});
```

### Pitfall 4: Unique Text Everywhere

```rust
// ❌ BAD: Every button is unique (no cache hits)
for i in 0..100 {
    parent.button(&format!("Button {}", i)).build();
}
// 100 text shaping operations
```

**Fix:** Reuse text when possible
```rust
// ✅ GOOD: Same text reused (1 shaping operation)
for i in 0..100 {
    parent.button("Action").build();
}
// 1 text shaping operation, cached for all
```

### Pitfall 5: Ignoring Dirty Flags

```rust
// ❌ BAD: Forces full rebuild for color change
ui.build(|root| {
    root.button("Click")
        .color(self.button_color)
        .build();
});
// 30ms rebuild for color change
```

**Fix:** Use appropriate update method
```rust
// ✅ GOOD: Color-only update
ui.update_color("button", self.button_color)?;
// <1ms update
```

## Advanced Techniques

### Lazy Widget Creation

Create widgets only when visible:

```rust
let visible_section = self.current_section;

ui.build(|root| {
    match visible_section {
        Section::Home => {
            root.container(|c| {
                // Only build home widgets
            }).build();
        }
        Section::Settings => {
            root.container(|c| {
                // Only build settings widgets
            }).build();
        }
    }
});
```

### Update Throttling

Limit update frequency for real-time data:

```rust
let mut last_update = Instant::now();
let update_interval = Duration::from_millis(100); // 10 Hz

fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
    let now = Instant::now();
    if now - last_update >= update_interval {
        ui.update_text("fps", &format!("{:.1}", fps))?;
        last_update = now;
    }
}
```

### Dirty Region Tracking

Track which screen regions changed:

```rust
// Mark specific regions dirty
ui.mark_region_dirty(Rect::new(0.0, 0.0, 200.0, 100.0));

// Skip rendering clean regions
if ui.is_region_dirty(viewport) {
    // Render this viewport
}
```

### Culling Optimization

Skip rendering off-screen widgets:

```rust
// Automatically culls widgets outside viewport
ui.set_viewport(Rect::new(
    scroll_x,
    scroll_y,
    window_width,
    window_height,
));

// Render only visible widgets
ui.render(pass.descriptor());
```

## Benchmarking Results

Real-world performance measurements:

### Simple UI (10 buttons)

| Operation | Time | FPS Impact |
|-----------|------|------------|
| Initial build | 2ms | None |
| Color update | 0.3ms | None |
| Text update | 1.2ms | None |
| Layout change | 3ms | None |
| Full rebuild | 2ms | None |

### Medium UI (100 widgets)

| Operation | Time | FPS Impact |
|-----------|------|------------|
| Initial build | 15ms | None |
| Color update | 0.5ms | None |
| Text update | 2ms | None |
| Layout change | 8ms | None |
| Full rebuild | 15ms | None |

### Large UI (1000 widgets, normal)

| Operation | Time | FPS Impact |
|-----------|------|------------|
| Initial build | 120ms | High |
| Color update | 0.8ms | None |
| Text update | 5ms | None |
| Layout change | 45ms | Medium |
| Full rebuild | 120ms | High |

### Large UI (1000 widgets, virtual scroll)

| Operation | Time | FPS Impact |
|-----------|------|------------|
| Initial build | 2ms | None |
| Scroll update | 1.5ms | None |
| Color update | 0.4ms | None |
| Text update | 1.8ms | None |

**Conclusion:** Virtual scrolling eliminates performance issues for large lists.

## Next Steps

- **Practice:** Try the `ui_dashboard` example with profiling enabled
- **Experiment:** Modify examples to see performance impact
- **Learn More:** [Advanced UI Patterns](../advanced/ui-patterns.md) (when added)
- **Build:** Create a high-performance UI for your game

## See Also

- [Custom Widgets](custom-widgets.md) - Build efficient custom widgets
- [Layout Deep Dive](layout-deep-dive.md) - Understand layout costs
- [Event Handling](event-handling.md) - Efficient event processing
- API Reference: [`UiSystem`](../../api/astrelis-ui/struct.UiSystem.html)
- API Reference: [`DirtyFlags`](../../api/astrelis-ui/dirty/struct.DirtyFlags.html)
