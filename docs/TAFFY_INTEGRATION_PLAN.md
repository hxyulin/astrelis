# Taffy UI Layout Integration Plan

## Overview

This document outlines the plan for integrating Taffy (flexbox/grid layout library) into Astrelis to provide a production-grade UI layout system alongside or replacing the current egui integration.

## Goals

- Provide declarative UI layout using flexbox/grid paradigms
- Integrate with existing WGPU renderer
- Leverage existing text rendering system (cosmic-text)
- Support interactive elements (buttons, inputs, etc.)
- Maintain high performance for game UI use cases
- Support styling similar to CSS

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                 Application Code                     │
│         (Declarative UI tree definition)             │
└────────────────────┬────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────┐
│              UI System Manager                       │
│  - Widget tree management                            │
│  - Event routing                                     │
│  - State management                                  │
└────────────────────┬────────────────────────────────┘
                     │
        ┌────────────┴────────────┐
        ▼                         ▼
┌──────────────┐         ┌────────────────┐
│ Taffy Layout │         │ Event System   │
│  - Compute   │         │ - Hit testing  │
│  - Cache     │         │ - Propagation  │
└──────┬───────┘         └────────┬───────┘
       │                          │
       ▼                          ▼
┌─────────────────────────────────────────────────────┐
│              UI Renderer (WGPU)                      │
│  - Quad rendering (boxes, borders)                  │
│  - Text rendering (cosmic-text integration)         │
│  - Image/Icon rendering                             │
│  - Clipping & layers                                │
└────────────────────┬────────────────────────────────┘
                     │
                     ▼
                GPU Output
```

## Core Components

### 1. Widget System

```rust
// crates/astrelis-ui/src/widget.rs

pub trait Widget {
    /// Unique type identifier
    fn type_id(&self) -> WidgetTypeId;
    
    /// Build the widget's style for Taffy
    fn style(&self) -> Style;
    
    /// Render the widget given its computed layout
    fn render(&self, ctx: &mut RenderContext, layout: &Layout);
    
    /// Handle events
    fn on_event(&mut self, ctx: &mut EventContext, event: &UiEvent) -> EventResponse;
    
    /// Measure function for leaf nodes (e.g., text, images)
    fn measure(
        &self,
        known_dimensions: taffy::Size<Option<f32>>,
        available_space: taffy::Size<taffy::AvailableSpace>,
    ) -> taffy::Size<f32> {
        taffy::Size::ZERO
    }
    
    /// Child widgets
    fn children(&self) -> &[Box<dyn Widget>];
}

// Built-in widgets
pub struct Container {
    style: ContainerStyle,
    children: Vec<Box<dyn Widget>>,
}

pub struct Text {
    content: String,
    font: FontHandle,
    size: f32,
    color: Color,
}

pub struct Button {
    label: String,
    on_click: Option<Box<dyn FnMut()>>,
    state: ButtonState,
    style: ButtonStyle,
}

pub struct Image {
    texture: TextureHandle,
    size: Option<Size<f32>>,
}

pub struct Row {
    gap: f32,
    children: Vec<Box<dyn Widget>>,
}

pub struct Column {
    gap: f32,
    children: Vec<Box<dyn Widget>>,
}
```

### 2. UI Tree & Layout Manager

```rust
// crates/astrelis-ui/src/tree.rs

use taffy::{TaffyTree, NodeId, Style, Layout};

pub struct UiTree {
    /// Taffy tree for layout computation
    taffy: TaffyTree<WidgetData>,
    
    /// Root node
    root: Option<NodeId>,
    
    /// Mapping from NodeId to widget
    widgets: HashMap<NodeId, Box<dyn Widget>>,
    
    /// Cached layouts
    layouts: HashMap<NodeId, Layout>,
    
    /// Dirty tracking
    dirty_nodes: HashSet<NodeId>,
}

impl UiTree {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            root: None,
            widgets: HashMap::new(),
            layouts: HashMap::new(),
            dirty_nodes: HashSet::new(),
        }
    }
    
    /// Build UI tree from declarative description
    pub fn build(&mut self, root_widget: Box<dyn Widget>) {
        self.clear();
        self.root = Some(self.build_node(root_widget, None));
    }
    
    fn build_node(&mut self, widget: Box<dyn Widget>, parent: Option<NodeId>) -> NodeId {
        let style = widget.style();
        let children: Vec<_> = widget.children()
            .iter()
            .map(|child| self.build_node(child.clone(), None))
            .collect();
        
        let node = self.taffy.new_with_children(style, &children).unwrap();
        self.widgets.insert(node, widget);
        
        node
    }
    
    /// Compute layout for given viewport size
    pub fn compute_layout(&mut self, viewport_size: taffy::Size<f32>) {
        if let Some(root) = self.root {
            self.taffy.compute_layout_with_measure(
                root,
                taffy::Size {
                    width: taffy::AvailableSpace::Definite(viewport_size.width),
                    height: taffy::AvailableSpace::Definite(viewport_size.height),
                },
                |known_dimensions, available_space, node_id, node_context, _style| {
                    // Measure function - delegates to widget's measure
                    if let Some(widget) = self.widgets.get(&node_id) {
                        widget.measure(known_dimensions, available_space)
                    } else {
                        taffy::Size::ZERO
                    }
                },
            ).unwrap();
            
            // Cache layouts
            self.cache_layouts(root);
        }
    }
    
    fn cache_layouts(&mut self, node: NodeId) {
        let layout = *self.taffy.layout(node).unwrap();
        self.layouts.insert(node, layout);
        
        for child in self.taffy.children(node).unwrap() {
            self.cache_layouts(child);
        }
    }
    
    /// Mark node as dirty (needs re-layout)
    pub fn mark_dirty(&mut self, node: NodeId) {
        self.dirty_nodes.insert(node);
    }
}
```

### 3. Declarative UI Builder API

```rust
// crates/astrelis-ui/src/builder.rs

pub fn container() -> ContainerBuilder {
    ContainerBuilder::default()
}

pub fn text(content: impl Into<String>) -> TextBuilder {
    TextBuilder::new(content.into())
}

pub fn button(label: impl Into<String>) -> ButtonBuilder {
    ButtonBuilder::new(label.into())
}

pub fn row() -> RowBuilder {
    RowBuilder::default()
}

pub fn column() -> ColumnBuilder {
    ColumnBuilder::default()
}

// Example builder pattern
pub struct ContainerBuilder {
    style: ContainerStyle,
    children: Vec<Box<dyn Widget>>,
}

impl ContainerBuilder {
    pub fn width(mut self, width: Dimension) -> Self {
        self.style.width = width;
        self
    }
    
    pub fn height(mut self, height: Dimension) -> Self {
        self.style.height = height;
        self
    }
    
    pub fn padding(mut self, padding: f32) -> Self {
        self.style.padding = Rect::all(padding);
        self
    }
    
    pub fn background(mut self, color: Color) -> Self {
        self.style.background_color = color;
        self
    }
    
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.style.border_width = width;
        self.style.border_color = color;
        self
    }
    
    pub fn child(mut self, widget: impl Widget + 'static) -> Self {
        self.children.push(Box::new(widget));
        self
    }
    
    pub fn children(mut self, widgets: Vec<Box<dyn Widget>>) -> Self {
        self.children.extend(widgets);
        self
    }
    
    pub fn build(self) -> Container {
        Container {
            style: self.style,
            children: self.children,
        }
    }
}

// Usage example:
fn build_ui() -> Box<dyn Widget> {
    Box::new(
        column()
            .padding(20.0)
            .gap(10.0)
            .child(
                text("Hello World")
                    .size(24.0)
                    .color(Color::WHITE)
                    .build()
            )
            .child(
                row()
                    .gap(10.0)
                    .child(
                        button("Click Me")
                            .on_click(|| println!("Clicked!"))
                            .build()
                    )
                    .child(
                        button("Cancel")
                            .style(ButtonStyle::Secondary)
                            .build()
                    )
                    .build()
            )
            .build()
    )
}
```

### 4. WGPU Renderer Integration

```rust
// crates/astrelis-ui/src/renderer.rs

pub struct UiRenderer {
    /// Quad renderer for boxes, borders, backgrounds
    quad_renderer: QuadRenderer,
    
    /// Text renderer (existing cosmic-text integration)
    text_renderer: TextRenderer,
    
    /// Image/icon renderer
    image_renderer: ImageRenderer,
    
    /// Render pipeline
    pipeline: wgpu::RenderPipeline,
    
    /// Vertex/index buffers
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    
    /// Uniform buffer (viewport transform)
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl UiRenderer {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        // Initialize sub-renderers
        let quad_renderer = QuadRenderer::new(device, config);
        let text_renderer = TextRenderer::new(device, config);
        let image_renderer = ImageRenderer::new(device, config);
        
        // Create pipeline, buffers, etc.
        // ...
        
        Self {
            quad_renderer,
            text_renderer,
            image_renderer,
            // ...
        }
    }
    
    pub fn render(
        &mut self,
        tree: &UiTree,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        viewport_size: (u32, u32),
    ) {
        // Begin render pass
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("UI Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        
        // Render UI tree in depth-first order
        if let Some(root) = tree.root {
            self.render_node(tree, root, &mut pass);
        }
    }
    
    fn render_node(
        &mut self,
        tree: &UiTree,
        node: NodeId,
        pass: &mut wgpu::RenderPass,
    ) {
        let widget = &tree.widgets[&node];
        let layout = &tree.layouts[&node];
        
        // Set up clipping if needed
        self.apply_clipping(layout, pass);
        
        // Render the widget
        match widget.type_id() {
            WidgetTypeId::Container => {
                self.render_container(widget, layout, pass);
            }
            WidgetTypeId::Text => {
                self.render_text(widget, layout, pass);
            }
            WidgetTypeId::Button => {
                self.render_button(widget, layout, pass);
            }
            WidgetTypeId::Image => {
                self.render_image(widget, layout, pass);
            }
            // ...
        }
        
        // Render children
        for child in tree.taffy.children(node).unwrap() {
            self.render_node(tree, child, pass);
        }
    }
    
    fn render_container(
        &mut self,
        widget: &dyn Widget,
        layout: &Layout,
        pass: &mut wgpu::RenderPass,
    ) {
        let container = widget.downcast_ref::<Container>().unwrap();
        
        // Render background
        if container.style.background_color.a > 0.0 {
            self.quad_renderer.draw_rect(
                layout.location.x,
                layout.location.y,
                layout.size.width,
                layout.size.height,
                container.style.background_color,
                pass,
            );
        }
        
        // Render border
        if container.style.border_width > 0.0 {
            self.quad_renderer.draw_border(
                layout.location.x,
                layout.location.y,
                layout.size.width,
                layout.size.height,
                container.style.border_width,
                container.style.border_color,
                pass,
            );
        }
    }
    
    fn render_text(
        &mut self,
        widget: &dyn Widget,
        layout: &Layout,
        pass: &mut wgpu::RenderPass,
    ) {
        let text = widget.downcast_ref::<Text>().unwrap();
        
        self.text_renderer.draw_text(
            &text.content,
            layout.location.x,
            layout.location.y,
            text.size,
            text.color,
            pass,
        );
    }
}

// Quad renderer for rectangles, borders, etc.
pub struct QuadRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: DynamicBuffer<QuadVertex>,
    index_buffer: DynamicBuffer<u32>,
}

impl QuadRenderer {
    pub fn draw_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
        pass: &mut wgpu::RenderPass,
    ) {
        // Generate quad vertices
        let vertices = [
            QuadVertex { position: [x, y], color: color.as_array() },
            QuadVertex { position: [x + width, y], color: color.as_array() },
            QuadVertex { position: [x + width, y + height], color: color.as_array() },
            QuadVertex { position: [x, y + height], color: color.as_array() },
        ];
        
        let indices = [0, 1, 2, 2, 3, 0];
        
        // Upload to GPU and draw
        self.vertex_buffer.write(&vertices);
        self.index_buffer.write(&indices);
        
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice());
        pass.set_index_buffer(self.index_buffer.slice(), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..6, 0, 0..1);
    }
    
    pub fn draw_border(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        border_width: f32,
        color: Color,
        pass: &mut wgpu::RenderPass,
    ) {
        // Draw 4 rectangles for border edges
        self.draw_rect(x, y, width, border_width, color, pass); // Top
        self.draw_rect(x, y + height - border_width, width, border_width, color, pass); // Bottom
        self.draw_rect(x, y, border_width, height, color, pass); // Left
        self.draw_rect(x + width - border_width, y, border_width, height, color, pass); // Right
    }
}
```

### 5. Shader Code

```wgsl
// crates/astrelis-ui/src/shaders/ui_quad.wgsl

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

struct Uniforms {
    screen_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    
    // Convert pixel coordinates to NDC
    let ndc_x = (input.position.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (input.position.y / uniforms.screen_size.y) * 2.0;
    
    output.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    output.color = input.color;
    
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
```

### 6. Event System

```rust
// crates/astrelis-ui/src/events.rs

pub struct UiEventSystem {
    /// Current hovered node
    hovered: Option<NodeId>,
    
    /// Focused node (for keyboard input)
    focused: Option<NodeId>,
    
    /// Mouse state
    mouse_pos: (f32, f32),
    mouse_buttons: MouseButtonState,
}

impl UiEventSystem {
    pub fn handle_event(
        &mut self,
        tree: &mut UiTree,
        event: &Event,
    ) -> HandleStatus {
        match event {
            Event::MouseMoved { x, y } => {
                self.mouse_pos = (*x, *y);
                self.update_hover(tree);
                HandleStatus::Ignored
            }
            Event::MouseButtonPressed { button } => {
                if let Some(hovered) = self.hovered {
                    self.dispatch_click(tree, hovered, *button);
                    HandleStatus::Consumed
                } else {
                    HandleStatus::Ignored
                }
            }
            // ... other events
            _ => HandleStatus::Ignored,
        }
    }
    
    fn update_hover(&mut self, tree: &UiTree) {
        let new_hover = self.hit_test(tree, self.mouse_pos);
        
        if new_hover != self.hovered {
            // Send hover exit event
            if let Some(old) = self.hovered {
                if let Some(widget) = tree.widgets.get(&old) {
                    widget.on_event(&mut EventContext::new(), &UiEvent::HoverExit);
                }
            }
            
            // Send hover enter event
            if let Some(new) = new_hover {
                if let Some(widget) = tree.widgets.get(&new) {
                    widget.on_event(&mut EventContext::new(), &UiEvent::HoverEnter);
                }
            }
            
            self.hovered = new_hover;
        }
    }
    
    fn hit_test(&self, tree: &UiTree, pos: (f32, f32)) -> Option<NodeId> {
        if let Some(root) = tree.root {
            self.hit_test_node(tree, root, pos)
        } else {
            None
        }
    }
    
    fn hit_test_node(
        &self,
        tree: &UiTree,
        node: NodeId,
        pos: (f32, f32),
    ) -> Option<NodeId> {
        let layout = &tree.layouts[&node];
        
        // Check if point is inside this node
        if pos.0 >= layout.location.x
            && pos.0 <= layout.location.x + layout.size.width
            && pos.1 >= layout.location.y
            && pos.1 <= layout.location.y + layout.size.height
        {
            // Check children (reverse order for Z-ordering)
            for child in tree.taffy.children(node).unwrap().iter().rev() {
                if let Some(hit) = self.hit_test_node(tree, *child, pos) {
                    return Some(hit);
                }
            }
            
            // This node is hit
            return Some(node);
        }
        
        None
    }
    
    fn dispatch_click(&mut self, tree: &mut UiTree, node: NodeId, button: MouseButton) {
        if let Some(widget) = tree.widgets.get_mut(&node) {
            widget.on_event(
                &mut EventContext::new(),
                &UiEvent::Click { button },
            );
        }
    }
}
```

### 7. Integration with Engine

```rust
// crates/astrelis-ui/src/lib.rs

pub struct UiSystem {
    tree: UiTree,
    renderer: UiRenderer,
    event_system: UiEventSystem,
}

impl UiSystem {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        Self {
            tree: UiTree::new(),
            renderer: UiRenderer::new(device, config),
            event_system: UiEventSystem::new(),
        }
    }
    
    pub fn build_ui(&mut self, root: Box<dyn Widget>) {
        self.tree.build(root);
    }
    
    pub fn update(&mut self, viewport_size: (f32, f32)) {
        self.tree.compute_layout(taffy::Size {
            width: viewport_size.0,
            height: viewport_size.1,
        });
    }
    
    pub fn handle_event(&mut self, event: &Event) -> HandleStatus {
        self.event_system.handle_event(&mut self.tree, event)
    }
    
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        viewport_size: (u32, u32),
    ) {
        self.renderer.render(&self.tree, encoder, view, viewport_size);
    }
}

// Usage in application:
impl AppHandler for MyApp {
    fn init(ctx: EngineCtx) -> Box<dyn AppHandler> {
        let mut ui = UiSystem::new(ctx.device(), ctx.surface_config());
        
        // Build declarative UI
        ui.build_ui(Box::new(
            column()
                .child(text("FPS: 60").build())
                .child(
                    button("Settings")
                        .on_click(|| println!("Open settings"))
                        .build()
                )
                .build()
        ));
        
        Box::new(Self { ui, /* ... */ })
    }
    
    fn update(&mut self, ctx: EngineCtx) {
        let size = ctx.window().size();
        self.ui.update((size.0 as f32, size.1 as f32));
    }
    
    fn on_event(&mut self, ctx: EngineCtx, event: &Event) -> HandleStatus {
        self.ui.handle_event(event)
    }
}
```

## Implementation Phases

### Phase 1: Foundation (Week 1-2)
- [ ] Add taffy dependency to workspace
- [ ] Create `astrelis-ui` crate structure
- [ ] Implement basic `Widget` trait and `UiTree`
- [ ] Implement simple container and text widgets
- [ ] Basic Taffy integration and layout computation

### Phase 2: Rendering (Week 2-3)
- [ ] Implement `QuadRenderer` for boxes/borders
- [ ] Integrate existing `TextRenderer`
- [ ] Create UI shader pipeline
- [ ] Implement coordinate transforms (pixel → NDC)
- [ ] Basic render pass integration

### Phase 3: Interactivity (Week 3-4)
- [ ] Implement event system
- [ ] Hit testing algorithm
- [ ] Button widget with hover/click states
- [ ] Focus management for keyboard input
- [ ] Event propagation & bubbling

### Phase 4: Advanced Widgets (Week 4-5)
- [ ] Input field widget
- [ ] Checkbox/Radio widgets
- [ ] Scrollable containers
- [ ] Image widget
- [ ] Layout helpers (Row, Column, Stack)

### Phase 5: Styling & Theming (Week 5-6)
- [ ] Style system (colors, fonts, spacing)
- [ ] Theme support
- [ ] CSS-like selectors/states
- [ ] Animation system
- [ ] Responsive design helpers

### Phase 6: Optimization & Polish (Week 6-7)
- [ ] Layout caching & dirty tracking
- [ ] Render batching
- [ ] Culling off-screen widgets
- [ ] Profiling & benchmarks
- [ ] Documentation & examples

## API Usage Examples

### Simple Button
```rust
button("Click Me")
    .width(Dimension::Points(120.0))
    .height(Dimension::Points(40.0))
    .on_click(|| println!("Clicked!"))
    .build()
```

### Flexbox Layout
```rust
row()
    .gap(10.0)
    .justify_content(JustifyContent::SpaceBetween)
    .child(button("Left").build())
    .child(button("Center").build())
    .child(button("Right").build())
    .build()
```

### Grid Layout
```rust
container()
    .display(Display::Grid)
    .grid_template_columns(vec![fr(1.0), fr(2.0)])
    .grid_gap(10.0)
    .child(text("Label").build())
    .child(input("Value").build())
    .build()
```

### Complex UI
```rust
column()
    .width(Dimension::Percent(1.0))
    .padding(20.0)
    .child(
        // Header
        row()
            .height(Dimension::Points(60.0))
            .background(Color::from_hex("#282c34"))
            .child(text("My Game").size(24.0).build())
            .build()
    )
    .child(
        // Content area
        row()
            .flex_grow(1.0)
            .gap(20.0)
            .child(
                // Sidebar
                column()
                    .width(Dimension::Points(200.0))
                    .background(Color::from_hex("#21252b"))
                    .child(button("Inventory").build())
                    .child(button("Skills").build())
                    .child(button("Map").build())
                    .build()
            )
            .child(
                // Main content
                container()
                    .flex_grow(1.0)
                    .background(Color::from_hex("#1e1e1e"))
                    .child(/* game content */)
                    .build()
            )
            .build()
    )
    .build()
```

## Performance Considerations

1. **Layout Caching**: Only recompute layout for dirty subtrees
2. **Render Batching**: Group similar draws (same shader/texture)
3. **Culling**: Skip rendering off-screen widgets
4. **Instancing**: Use instanced rendering for repeated elements
5. **Atlas Management**: Pack textures/icons into atlases
6. **Event Optimization**: Spatial hashing for hit testing

## Dependencies

```toml
[dependencies]
taffy = { version = "0.9", features = ["grid", "flexbox", "block_layout"] }
cosmic-text = "0.12" # Already present
wgpu = "24.0" # Already present
glam = { version = "0.30", features = ["bytemuck"] } # Already present
```

## Alternative Approach: Hybrid with egui

Instead of replacing egui entirely, could use Taffy for specific game UI elements while keeping egui for debug/editor UI:

```rust
struct HybridUiSystem {
    taffy_ui: UiSystem,      // Game UI (HUD, menus, etc.)
    egui_ui: EguiState,       // Debug UI (profiler, editor, etc.)
}
```

This allows leveraging egui's mature widget library for tools while having custom game UI with Taffy.

## Open Questions

1. Should we support retained mode (like egui) or immediate mode API?
2. How to handle animations smoothly?
3. Integration with accessibility features?
4. Support for custom shaders per widget?
5. How to handle text input (IME, selection, etc.)?

## Conclusion

This plan provides a complete path to integrating Taffy for flexible, production-grade UI layout in Astrelis. The modular architecture allows incremental implementation and testing while maintaining compatibility with existing systems.