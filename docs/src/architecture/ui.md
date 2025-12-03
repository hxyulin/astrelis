# UI System

The `astrelis-ui` crate provides a flexible, GPU-accelerated UI system built on the Taffy layout engine. It features declarative widget building, incremental updates, and efficient rendering.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Application                          │
│              (Your game UI code)                        │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│                   UiSystem                              │
│  ┌──────────────┐  ┌────────────────┐  ┌────────────┐  │
│  │   UiTree     │  │ UiEventSystem  │  │ UiRenderer │  │
│  │  (widgets,   │  │ (hit testing,  │  │  (batched  │  │
│  │   layout)    │  │   dispatch)    │  │  drawing)  │  │
│  └──────────────┘  └────────────────┘  └────────────┘  │
│         │                   │                  │         │
│  ┌──────────────┐  ┌────────────────┐  ┌────────────┐  │
│  │WidgetIdReg.  │  │  Event Queue   │  │FontRenderer│  │
│  └──────────────┘  └────────────────┘  └────────────┘  │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│              External Dependencies                      │
│  ┌──────────────┐  ┌────────────────┐                   │
│  │    Taffy     │  │  astrelis-text │                   │
│  │  (flexbox,   │  │  (text render) │                   │
│  │    grid)     │  │                │                   │
│  └──────────────┘  └────────────────┘                   │
└─────────────────────────────────────────────────────────┘
```

## Core Components

### UiSystem

The main entry point managing all UI subsystems:

```rust
use astrelis_ui::{UiSystem, widgets::*, Color};
use astrelis_render::GraphicsContext;

let context = GraphicsContext::new_sync();
let mut ui = UiSystem::new(context);

// Build UI tree
ui.build(|root| {
    root.container()
        .width(800.0)
        .height(600.0)
        .child(
            root.text("Hello, World!")
                .size(24.0)
                .color(Color::WHITE)
        );
});

// Main loop
loop {
    ui.update(delta_time);
    ui.handle_events(&mut events);
    ui.render(&mut render_pass, viewport_size);
}
```

### UiTree

Hierarchical widget tree with Taffy integration:

```rust
pub struct UiTree {
    taffy: TaffyTree<()>,           // Layout engine
    nodes: IndexMap<NodeId, UiNode>, // Widget storage
    root: Option<NodeId>,            // Root node
    dirty_nodes: HashSet<NodeId>,   // Nodes needing layout
}
```

**Key features**:
- Hierarchical parent-child relationships
- Integration with Taffy for layout computation
- Dirty tracking for incremental updates
- Cached text measurements
- O(log n) node lookup via IndexMap

### UiNode

Individual node in the tree:

```rust
pub struct UiNode {
    pub widget: Box<dyn Widget>,        // Widget implementation
    pub taffy_node: taffy::NodeId,      // Taffy layout node
    pub layout: LayoutRect,             // Computed position/size
    pub dirty: bool,                    // Needs recomputation
    pub parent: Option<NodeId>,         // Parent node
    pub children: Vec<NodeId>,          // Child nodes
    pub text_measurement: Option<(f32, f32)>, // Cached text size
}
```

## Widget System

### Widget Trait

Core interface all widgets implement:

```rust
pub trait Widget {
    fn style(&self) -> &Style;
    fn style_mut(&mut self) -> &mut Style;
    fn render(&self, ctx: &mut RenderContext);
    fn handle_event(&mut self, event: &UiEvent) -> bool;
    fn measure(&self, constraints: Size) -> Size;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
```

### Built-in Widgets

#### Container

Generic container for layout:

```rust
root.container()
    .width(400.0)
    .height(300.0)
    .padding(20.0)
    .flex_direction(FlexDirection::Column)
    .gap(10.0)
    .background_color(Color::rgba(0.2, 0.2, 0.2, 1.0))
    .child(/* ... */);
```

#### Text

Static text display:

```rust
root.text("Hello, World!")
    .size(24.0)
    .color(Color::WHITE)
    .bold()
    .align(TextAlign::Center);
```

#### Button

Interactive button with label:

```rust
root.button("Click Me")
    .on_click(|| println!("Clicked!"))
    .padding(10.0)
    .background_color(Color::BLUE)
    .hover_color(Color::rgba(0.3, 0.3, 0.8, 1.0));
```

#### TextInput

Single-line text input:

```rust
root.text_input("Enter text...")
    .width(200.0)
    .on_change(|value| println!("Input: {}", value))
    .placeholder_color(Color::rgba(0.5, 0.5, 0.5, 1.0));
```

### Custom Widgets

Create custom widgets by implementing `Widget`:

```rust
pub struct ProgressBar {
    style: Style,
    progress: f32,
    background_color: Color,
    fill_color: Color,
}

impl Widget for ProgressBar {
    fn style(&self) -> &Style {
        &self.style
    }
    
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
    
    fn render(&self, ctx: &mut RenderContext) {
        // Draw background
        ctx.draw_rect(
            ctx.layout.position(),
            ctx.layout.size(),
            self.background_color
        );
        
        // Draw fill
        let fill_width = ctx.layout.width * self.progress;
        ctx.draw_rect(
            ctx.layout.position(),
            Vec2::new(fill_width, ctx.layout.height),
            self.fill_color
        );
    }
    
    fn handle_event(&mut self, event: &UiEvent) -> bool {
        false // No interaction
    }
    
    fn measure(&self, constraints: Size) -> Size {
        // Use style-defined size or constraints
        Size::default()
    }
    
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
```

## Layout System

### Taffy Integration

Astrelis uses Taffy for flexbox and grid layouts:

```rust
// Flexbox container
root.container()
    .flex_direction(FlexDirection::Row)
    .justify_content(JustifyContent::SpaceBetween)
    .align_items(AlignItems::Center)
    .gap(10.0);

// Grid container
root.container()
    .display(Display::Grid)
    .grid_template_columns(vec![
        GridTrack::fr(1.0),
        GridTrack::fr(1.0),
        GridTrack::fr(1.0),
    ])
    .grid_template_rows(vec![
        GridTrack::auto(),
        GridTrack::fr(1.0),
    ]);
```

### Style System

Comprehensive styling options:

```rust
pub struct Style {
    pub layout: taffy::Style,     // Taffy layout properties
    pub background_color: Color,   // Background fill
    pub border_color: Color,       // Border color
    pub border_width: f32,         // Border thickness
    pub border_radius: f32,        // Corner rounding
    pub opacity: f32,              // Transparency
}
```

### Sizing

Multiple sizing modes:

```rust
// Fixed size
.width(200.0)
.height(100.0)

// Percentage of parent
.width_percent(50.0)
.height_percent(100.0)

// Auto (fit content)
.width_auto()
.height_auto()

// Fill available space
.flex_grow(1.0)

// Min/max constraints
.min_width(100.0)
.max_width(500.0)
```

### Spacing

Margin, padding, and gap:

```rust
// Uniform spacing
.margin(10.0)
.padding(20.0)

// Per-side spacing
.margin_left(10.0)
.margin_right(10.0)
.padding_top(5.0)
.padding_bottom(5.0)

// Gap between children
.gap(15.0)
```

## Incremental Updates

### Problem: Full Rebuild Cost

Traditional approach rebuilds entire UI every frame:
- O(n) widget creation/destruction
- O(n) text layout operations
- O(n) style computations
- High allocation overhead

### Solution: Dirty Tracking

Only update changed widgets:

```rust
pub struct UiTree {
    dirty_nodes: HashSet<NodeId>,
    // ...
}

impl UiTree {
    pub fn mark_dirty(&mut self, node_id: NodeId) {
        self.dirty_nodes.insert(node_id);
        
        // Mark ancestors dirty (layout may change)
        let mut current = self.get_node(node_id).parent;
        while let Some(parent_id) = current {
            self.dirty_nodes.insert(parent_id);
            current = self.get_node(parent_id).parent;
        }
    }
}
```

### WidgetId System

Stable IDs for incremental updates:

```rust
use astrelis_ui::WidgetId;

// Register widget during build
let counter_id = WidgetId::new("counter");
ui.build(|root| {
    root.text("Count: 0")
        // Manual registration (automatic in future)
});
ui.register_widget(counter_id, node_id);

// Update without rebuild
ui.update_text(counter_id, format!("Count: {}", count));
```

**Benefits**:
- 10-100x faster than full rebuild for small changes
- No allocation churn
- Maintains scroll position, focus, hover state
- Animations remain smooth

### Cached Measurements

Text measurements are expensive (layout required):

```rust
pub struct UiNode {
    pub text_measurement: Option<(f32, f32)>,
    // ...
}

impl UiTree {
    pub fn measure_text(&mut self, node_id: NodeId, font_renderer: &FontRenderer) -> (f32, f32) {
        let node = &self.nodes[&node_id];
        
        // Return cached if available and not dirty
        if !node.dirty {
            if let Some(cached) = node.text_measurement {
                return cached;
            }
        }
        
        // Measure and cache
        let size = font_renderer.measure_text(/* ... */);
        self.nodes.get_mut(&node_id).unwrap().text_measurement = Some(size);
        size
    }
}
```

## Event System

### Event Types

```rust
pub enum UiEvent {
    MouseMove { position: Vec2 },
    MouseDown { button: MouseButton, position: Vec2 },
    MouseUp { button: MouseButton, position: Vec2 },
    MouseWheel { delta: f32 },
    KeyDown { key: KeyCode, modifiers: Modifiers },
    KeyUp { key: KeyCode, modifiers: Modifiers },
    Char { character: char },
    FocusGained { node_id: NodeId },
    FocusLost { node_id: NodeId },
}
```

### Event Handling Flow

```
1. EventBatch from winit
   ↓
2. UiEventSystem converts to UiEvent
   ↓
3. Hit testing (determine widget under cursor)
   ↓
4. Event dispatch to widget
   ↓
5. Widget handles event (returns true if consumed)
   ↓
6. Bubble to parent if not consumed
```

### Hit Testing

Determine which widget is under a point:

```rust
impl UiTree {
    pub fn hit_test(&self, point: Vec2) -> Option<NodeId> {
        self.hit_test_recursive(self.root?, point)
    }
    
    fn hit_test_recursive(&self, node_id: NodeId, point: Vec2) -> Option<NodeId> {
        let node = &self.nodes[&node_id];
        
        if !node.layout.contains(point) {
            return None;
        }
        
        // Test children (reverse order, top to bottom)
        for &child_id in node.children.iter().rev() {
            if let Some(hit) = self.hit_test_recursive(child_id, point) {
                return Some(hit);
            }
        }
        
        // No child hit, this node is hit
        Some(node_id)
    }
}
```

### Callbacks

Type-safe event callbacks:

```rust
// Click callback
.on_click(|| {
    println!("Button clicked!");
})

// Change callback with value
.on_change(|value: String| {
    println!("Input changed: {}", value);
})

// Hover callbacks
.on_hover_enter(|| { /* ... */ })
.on_hover_exit(|| { /* ... */ })

// Focus callbacks
.on_focus(|| { /* ... */ })
.on_blur(|| { /* ... */ })
```

## Rendering

### Render Pipeline

```
1. Compute layout (if dirty)
   Taffy computes positions/sizes
   ↓
2. Prepare text (measure, layout, cache)
   FontRenderer prepares glyphs
   ↓
3. Batch geometry
   Collect quads, sort by texture
   ↓
4. Upload vertices
   Update vertex buffer
   ↓
5. Draw calls
   One call per texture batch
   ↓
6. Present frame
```

### Batching Strategy

Minimize draw calls by batching:

```rust
pub struct UiRenderer {
    quad_batches: Vec<QuadBatch>,
    text_batches: Vec<TextBatch>,
}

struct QuadBatch {
    texture: Option<wgpu::TextureView>,
    vertices: Vec<Vertex>,
}
```

**Batching rules**:
- Same texture → same batch
- Text from same font/size → same batch
- Translucent widgets sorted back-to-front

### Vertex Format

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],  // Screen-space position
    uv: [f32; 2],        // Texture coordinates
    color: [f32; 4],     // Vertex color (RGBA)
}
```

### Shader Pipeline

Simple textured quad shader:

```wgsl
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Transform to NDC
    out.position = vec4<f32>(
        in.position.x * 2.0 / viewport_width - 1.0,
        1.0 - in.position.y * 2.0 / viewport_height,
        0.0,
        1.0
    );
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(texture, sampler, in.uv);
    return tex_color * in.color;
}
```

## Performance Characteristics

### Full Rebuild

- **Widget creation**: O(n) allocations
- **Layout computation**: O(n) with Taffy
- **Text layout**: O(n) expensive operations
- **Total**: ~5-10ms for 1000 widgets

### Incremental Update

- **Mark dirty**: O(log n) tree traversal
- **Layout computation**: O(m) where m = dirty count
- **Text remeasurement**: O(m) only changed text
- **Total**: ~0.1-1ms for typical updates

### Rendering

- **Batching**: O(n) geometry collection
- **Draw calls**: O(b) where b = batch count (~5-20)
- **GPU time**: ~0.5-2ms for typical UI
- **Total**: ~1-3ms per frame

### Memory Usage

- **Per widget**: ~200 bytes (node + widget + taffy)
- **Per text**: +layout buffer (~1-5KB)
- **Vertex buffer**: Dynamic, grows as needed
- **Texture atlas**: 1024x1024 RGBA (~4MB) for text

## Best Practices

### 1. Prefer Incremental Updates

```rust
// Bad: Full rebuild every frame
loop {
    ui.build(|root| {
        root.text(format!("FPS: {}", fps)); // Expensive!
    });
}

// Good: Incremental update
let fps_text_id = WidgetId::new("fps");
loop {
    ui.update_text(fps_text_id, format!("FPS: {}", fps));
}
```

### 2. Minimize Nesting Depth

Shallow trees are faster to traverse:

```rust
// Bad: Deep nesting
root.container()
    .child(root.container()
        .child(root.container()
            .child(root.text("Deep"))));

// Good: Flatter structure
root.container()
    .flex_direction(FlexDirection::Row)
    .child(root.text("Item 1"))
    .child(root.text("Item 2"))
    .child(root.text("Item 3"));
```

### 3. Reuse Widget IDs

Don't create new IDs every frame:

```rust
// Bad: New ID every frame
let id = WidgetId::new(&format!("widget_{}", frame_count));

// Good: Stable ID
static WIDGET_ID: WidgetId = WidgetId::new("my_widget");
```

### 4. Batch Style Changes

Group changes to avoid multiple dirty marks:

```rust
// Bad: Multiple updates
ui.update_widget(id, |w| w.set_color(Color::RED));
ui.update_widget(id, |w| w.set_size(100.0));

// Good: Single update
ui.update_widget(id, |w| {
    w.set_color(Color::RED);
    w.set_size(100.0);
});
```

### 5. Profile Your UI

Use puffin to identify bottlenecks:

```rust
{
    profile_scope!("ui_update");
    ui.update(delta_time);
}
{
    profile_scope!("ui_render");
    ui.render(&mut pass, viewport);
}
```

## Future Enhancements

1. **Auto-registration** - Builder automatically registers WidgetIds
2. **Animations** - Tween properties over time
3. **Transitions** - Smooth state changes
4. **Clipping** - Scissor rects for scroll views
5. **Virtualization** - Render only visible widgets
6. **Accessibility** - Screen reader support
7. **Theming** - Global style overrides
8. **Layout caching** - Persist layout across frames
9. **Spatial indexing** - O(log n) hit testing with quad-tree
10. **Multi-window** - UI per window with shared resources