# astrelis-render

Modular rendering framework for the Astrelis game engine.

## Overview

`astrelis-render` provides a modular, extensible architecture for managing GPU resources and rendering. It wraps WGPU with higher-level abstractions while maintaining low-level control when needed.

## Architecture

### Core Modules

- **`context`** - Graphics context management (device, queue, adapter)
- **`window`** - Window rendering contexts and surface management
- **`frame`** - Frame lifecycle and render pass builders
- **`renderer`** - Low-level extensible renderer for resource management

### Design Philosophy

1. **Modular**: Each component has a clear responsibility
2. **Extensible**: Easy to build higher-level renderers (TextRenderer, SceneRenderer, etc.)
3. **Type-Safe**: Leverages Rust's type system for safety
4. **Zero-Cost**: Minimal overhead over raw WGPU

## Quick Start

### Basic Setup

```rust
use astrelis_render::{GraphicsContext, RenderableWindow, WindowContextDescriptor};
use astrelis_winit::{app::run_app, window::WindowDescriptor};

run_app(|ctx| {
    // Create graphics context
    let graphics_ctx = GraphicsContext::new_sync();
    
    // Create window
    let window = ctx.create_window(WindowDescriptor::default())?;
    let window = RenderableWindow::new(window, graphics_ctx);
    
    Box::new(MyApp { window })
});
```

### Using the Renderer API

```rust
use astrelis_render::Renderer;

let graphics_ctx = GraphicsContext::new_sync();
let renderer = Renderer::new(graphics_ctx);

// Create shader
let shader = renderer.create_shader(Some("My Shader"), shader_source);

// Create vertex buffer
let vertices: &[f32] = &[/* ... */];
let vertex_buffer = renderer.create_vertex_buffer(Some("Vertices"), vertices);

// Create texture
let texture = renderer.create_texture_2d(
    Some("My Texture"),
    width, height,
    wgpu::TextureFormat::Rgba8UnormSrgb,
    wgpu::TextureUsages::TEXTURE_BINDING,
    texture_data,
);

// Create sampler
let sampler = renderer.create_linear_sampler(Some("Sampler"));

// Create bind group
let bind_group_layout = renderer.create_bind_group_layout(
    Some("Layout"),
    &[/* entries */],
);

let bind_group = renderer.create_bind_group(
    Some("Bind Group"),
    &bind_group_layout,
    &[/* entries */],
);
```

### Render Loop

```rust
impl App for MyApp {
    fn update(&mut self, _ctx: &mut AppCtx) {
        // Global logic
    }
    
    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }
        
        // Handle events
        events.dispatch(|event| {
            match event {
                Event::WindowResized(size) => {
                    self.window.resized(*size);
                    HandleStatus::consumed()
                }
                _ => HandleStatus::ignored()
            }
        });
        
        // Begin frame
        let mut frame = self.window.begin_drawing();
        
        {
            // Create render pass
            let mut render_pass = RenderPassBuilder::new()
                .label("Main Pass")
                .color_attachment(
                    None, // Use window surface
                    None,
                    wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                )
                .build(&mut frame);
            
            let pass = render_pass.wgpu_pass();
            // ... render commands
        }
        
        frame.finish();
    }
}
```

## API Reference

### GraphicsContext

Manages the WGPU instance, adapter, device, and queue.

```rust
// Create with defaults
let ctx = GraphicsContext::new_sync();

// Create with custom descriptor
let ctx = GraphicsContext::new_with_descriptor(
    GraphicsContextDescriptor {
        backends: wgpu::Backends::VULKAN | wgpu::Backends::METAL,
        power_preference: wgpu::PowerPreference::HighPerformance,
        features: wgpu::Features::PUSH_CONSTANTS,
        ..Default::default()
    }
).await;

// Query device info
let info = ctx.info();
let limits = ctx.limits();
let features = ctx.features();
```

### WindowContext

Manages a window surface and its configuration.

```rust
let window_ctx = WindowContext::new(
    window,
    graphics_ctx,
    WindowContextDescriptor {
        format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
        present_mode: Some(wgpu::PresentMode::Mailbox),
        alpha_mode: Some(wgpu::CompositeAlphaMode::Opaque),
    },
);

// Handle resize
window_ctx.resized(new_size);

// Reconfigure surface
window_ctx.reconfigure_surface(new_config);

// Access surface
let surface = window_ctx.surface();
let config = window_ctx.surface_config();
```

### Renderer

Low-level API for creating GPU resources.

#### Buffer Creation

```rust
// Vertex buffer
let vertex_buffer = renderer.create_vertex_buffer(label, vertices);

// Index buffer
let index_buffer = renderer.create_index_buffer(label, indices);

// Uniform buffer
let uniform_buffer = renderer.create_uniform_buffer(label, &uniforms);

// Update uniform
renderer.update_uniform_buffer(&uniform_buffer, &new_uniforms);
```

#### Texture Creation

```rust
// 2D texture with data
let texture = renderer.create_texture_2d(
    label,
    width, height,
    format,
    usage,
    data,
);

// Custom texture descriptor
let texture = renderer.create_texture(&descriptor);
```

#### Sampler Creation

```rust
// Linear sampler
let sampler = renderer.create_linear_sampler(label);

// Nearest sampler
let sampler = renderer.create_nearest_sampler(label);

// Custom sampler
let sampler = renderer.create_sampler(&descriptor);
```

#### Bind Groups

```rust
// Create layout
let layout = renderer.create_bind_group_layout(label, &entries);

// Create bind group
let bind_group = renderer.create_bind_group(label, &layout, &entries);
```

#### Pipeline Creation

```rust
// Create shader
let shader = renderer.create_shader(label, source);

// Create pipeline layout
let pipeline_layout = renderer.create_pipeline_layout(
    label,
    &[&bind_group_layout],
    &push_constant_ranges,
);

// Create render pipeline
let pipeline = renderer.create_render_pipeline(&descriptor);

// Create compute pipeline
let compute_pipeline = renderer.create_compute_pipeline(&descriptor);
```

#### Command Submission

```rust
// Create encoder
let mut encoder = renderer.create_command_encoder(label);

// ... record commands

// Submit
renderer.submit(std::iter::once(encoder.finish()));
```

### RenderPassBuilder

Builder for creating render passes with automatic encoder management.

```rust
let mut render_pass = RenderPassBuilder::new()
    .label("My Pass")
    .color_attachment(
        Some(&texture_view),
        None, // No resolve target
        wgpu::Operations {
            load: wgpu::LoadOp::Clear(color),
            store: wgpu::StoreOp::Store,
        },
    )
    .depth_stencil_attachment(
        &depth_view,
        Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(1.0),
            store: wgpu::StoreOp::Store,
        }),
        None, // No stencil
    )
    .build(&mut frame);

// Use the render pass
let pass = render_pass.wgpu_pass();
pass.set_pipeline(&pipeline);
// ... render commands

// Encoder automatically returned to frame when dropped
```

## WGPU Re-export

All WGPU types are re-exported for convenience:

```rust
use astrelis_render::wgpu;

// Instead of:
// use wgpu::TextureFormat;

// You can use:
use astrelis_render::wgpu::TextureFormat;
```

## Building Higher-Level Renderers

The `Renderer` is designed as a foundation for specialized renderers:

```rust
pub struct TextRenderer {
    renderer: Renderer,
    pipeline: wgpu::RenderPipeline,
    font_atlas: wgpu::Texture,
    // ...
}

impl TextRenderer {
    pub fn new(context: &'static GraphicsContext) -> Self {
        let renderer = Renderer::new(context);
        
        // Use renderer API to create resources
        let shader = renderer.create_shader(/* ... */);
        let font_atlas = renderer.create_texture_2d(/* ... */);
        
        Self { renderer, pipeline, font_atlas }
    }
    
    pub fn draw_text(&mut self, text: &str, position: Vec2) {
        // High-level text rendering API
    }
}
```

### Example Renderer Types

- **TextRenderer** - Text rendering with font atlases
- **SpriteRenderer** - 2D sprite batching
- **SceneRenderer** - 3D scene rendering with lighting
- **UIRenderer** - Immediate-mode UI rendering
- **ParticleRenderer** - GPU particle systems
- **DebugRenderer** - Debug shapes and lines

## Examples

See the `examples/` directory for complete examples:

- **`renderer_api.rs`** - Demonstrates the low-level Renderer API
- **`textured_window.rs`** - Basic textured quad rendering
- **`multi_window.rs`** - Multiple windows with different content

Run an example:

```bash
cargo run --package astrelis-render --example renderer_api
```

## Features

- **Modular Architecture** - Clean separation of concerns
- **Type-Safe** - Compile-time safety with Rust
- **Zero-Cost Abstractions** - Minimal overhead
- **Multi-Window Support** - Render to multiple windows
- **Extensible** - Easy to build specialized renderers
- **WGPU Integration** - Full access to WGPU features
- **Profiling Integration** - Built-in profiling support

## License

Part of the Astrelis game engine.