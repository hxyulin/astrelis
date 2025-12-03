# Rendering Pipeline

The rendering system in Astrelis is built on WGPU, providing a cross-platform graphics API that targets Vulkan, Metal, DirectX 12, and WebGPU. The `astrelis-render` crate provides a modular foundation for building renderers.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Application Layer                     │
│              (Your game rendering code)                 │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│              High-Level Renderers                       │
│   ┌─────────────┐  ┌─────────────┐  ┌──────────────┐   │
│   │ UI Renderer │  │Text Renderer│  │Scene Renderer│   │
│   │ (batched)   │  │(atlas-based)│  │  (planned)   │   │
│   └─────────────┘  └─────────────┘  └──────────────┘   │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│              astrelis-render Core                       │
│   ┌──────────────────────────────────────────────────┐  │
│   │           GraphicsContext                        │  │
│   │  (WGPU Device, Queue, Adapter)                   │  │
│   └──────────────────────────────────────────────────┘  │
│   ┌──────────────────────────────────────────────────┐  │
│   │           WindowContext                          │  │
│   │  (Surface, Swapchain, Format)                    │  │
│   └──────────────────────────────────────────────────┘  │
│   ┌──────────────────────────────────────────────────┐  │
│   │           Renderer Trait                         │  │
│   │  (Resource management base)                      │  │
│   └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│                      WGPU                               │
│            (Vulkan/Metal/DX12/WebGPU)                   │
└─────────────────────────────────────────────────────────┘
```

## GraphicsContext

The `GraphicsContext` is the central hub for all GPU operations, created once and shared across the application as a static reference.

### Initialization

```rust
use astrelis_render::{GraphicsContext, GraphicsContextDescriptor};

// Synchronous initialization (blocks until GPU is ready)
let context = GraphicsContext::new_sync();

// Asynchronous initialization
let context = GraphicsContext::new().await;

// Custom configuration
let descriptor = GraphicsContextDescriptor {
    backends: wgpu::Backends::VULKAN | wgpu::Backends::METAL,
    power_preference: wgpu::PowerPreference::HighPerformance,
    features: wgpu::Features::PUSH_CONSTANTS,
    ..Default::default()
};
let context = GraphicsContext::new_with_descriptor(descriptor).await;
```

### Structure

```rust
pub struct GraphicsContext {
    pub instance: wgpu::Instance,  // GPU instance (entry point)
    pub adapter: wgpu::Adapter,    // Physical GPU device
    pub device: wgpu::Device,      // Logical device for commands
    pub queue: wgpu::Queue,        // Command submission queue
}
```

### Lifetime Management

`GraphicsContext` uses `&'static` lifetime achieved via `Box::leak`:
- Created once at application start
- Persists for entire program lifetime
- Eliminates lifetime parameters from all rendering APIs
- Simplifies resource ownership

This pattern is standard in game engines where graphics context outlives all other resources.

## WindowContext

Per-window rendering state managing the swapchain and surface.

### Creation

```rust
use astrelis_render::{WindowContext, WindowContextDescriptor};

let window_context = WindowContext::new(
    graphics_context,
    &window,  // winit::window::Window
    WindowContextDescriptor::default()
);
```

### Structure

```rust
pub struct WindowContext {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),
}
```

### Surface Configuration

Automatically configured for optimal performance:
- **Present mode**: Mailbox (low latency) or Fifo (vsync)
- **Format**: Bgra8Unorm or Rgba8Unorm (platform-dependent)
- **Alpha mode**: Opaque (no transparency)
- **Usage**: RenderAttachment (for rendering)

### Frame Acquisition

```rust
let frame = window_context.current_frame()?;
let view = frame.texture.create_view(&Default::default());

// Render to view
// ...

frame.present(); // Submit to display
```

## Renderer Trait

Base trait for building custom renderers with resource management.

### Interface

```rust
pub trait Renderer {
    type Resources;
    
    fn new(context: &'static GraphicsContext) -> Self;
    fn resources(&self) -> &Self::Resources;
    fn resize(&mut self, width: u32, height: u32);
}
```

### Example: Custom Renderer

```rust
use astrelis_render::{Renderer, GraphicsContext};

pub struct MyRenderer {
    context: &'static GraphicsContext,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
}

impl Renderer for MyRenderer {
    type Resources = ();
    
    fn new(context: &'static GraphicsContext) -> Self {
        let pipeline = create_pipeline(context);
        let vertex_buffer = create_vertex_buffer(context);
        let uniform_buffer = create_uniform_buffer(context);
        
        Self {
            context,
            pipeline,
            vertex_buffer,
            uniform_buffer,
        }
    }
    
    fn resources(&self) -> &Self::Resources {
        &()
    }
    
    fn resize(&mut self, width: u32, height: u32) {
        // Update viewport-dependent resources
    }
}

impl MyRenderer {
    pub fn render(&mut self, view: &wgpu::TextureView) {
        let mut encoder = self.context.device.create_command_encoder(&Default::default());
        
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..3, 0..1);
        }
        
        self.context.queue.submit(Some(encoder.finish()));
    }
}
```

## Frame Management

### Frame Lifecycle

```
1. Acquire frame from swapchain
   window_context.current_frame()
   ↓
2. Create texture view
   frame.texture.create_view()
   ↓
3. Create command encoder
   device.create_command_encoder()
   ↓
4. Begin render pass
   encoder.begin_render_pass()
   ↓
5. Execute render commands
   pass.set_pipeline(), pass.draw()
   ↓
6. End render pass (implicit drop)
   ↓
7. Finish command buffer
   encoder.finish()
   ↓
8. Submit to queue
   queue.submit()
   ↓
9. Present frame
   frame.present()
```

### Error Handling

Frame acquisition can fail:
```rust
match window_context.current_frame() {
    Ok(frame) => {
        // Render frame
    }
    Err(wgpu::SurfaceError::Lost) => {
        // Surface lost, recreate
        window_context.recreate_surface();
    }
    Err(wgpu::SurfaceError::OutOfMemory) => {
        // Fatal error, exit
        panic!("Out of GPU memory");
    }
    Err(_) => {
        // Transient error, skip frame
    }
}
```

## Color Management

### Color Type

```rust
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
```

Implements `From<wgpu::Color>` and `Into<wgpu::Color>` for seamless conversion.

### Predefined Colors

```rust
Color::WHITE
Color::BLACK
Color::RED
Color::GREEN
Color::BLUE
Color::TRANSPARENT
```

### Construction

```rust
// From components
let color = Color::rgba(1.0, 0.5, 0.0, 1.0);

// From hex
let color = Color::from_hex(0xFF8000FF);

// From u8 components
let color = Color::from_rgba8(255, 128, 0, 255);
```

## Resource Management

### Buffer Creation

```rust
let vertex_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("Vertex Buffer"),
    size: vertex_data.len() as u64,
    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    mapped_at_creation: false,
});

context.queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertex_data));
```

### Texture Creation

```rust
let texture = context.device.create_texture(&wgpu::TextureDescriptor {
    label: Some("Atlas Texture"),
    size: wgpu::Extent3d {
        width: 1024,
        height: 1024,
        depth_or_array_layers: 1,
    },
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format: wgpu::TextureFormat::Rgba8Unorm,
    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    view_formats: &[],
});
```

### Pipeline Creation

```rust
let shader = context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("Shader"),
    source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
});

let pipeline = context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    label: Some("Pipeline"),
    layout: Some(&pipeline_layout),
    vertex: wgpu::VertexState {
        module: &shader,
        entry_point: "vs_main",
        buffers: &[vertex_buffer_layout],
        compilation_options: Default::default(),
    },
    fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: "fs_main",
        targets: &[Some(wgpu::ColorTargetState {
            format: surface_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })],
        compilation_options: Default::default(),
    }),
    primitive: wgpu::PrimitiveState::default(),
    depth_stencil: None,
    multisample: wgpu::MultisampleState::default(),
    multiview: None,
    cache: None,
});
```

## Performance Considerations

### Batching

Minimize draw calls by batching similar geometry:
- UI quads batched into single draw call
- Text glyphs from same atlas rendered together
- Sort by material/texture to reduce state changes

### Buffer Updates

Strategies for updating dynamic buffers:
- **Small updates**: Use `write_buffer()` for < 1KB
- **Large updates**: Use staging buffer and copy commands
- **Frequent updates**: Use ring buffer with multiple frames in flight

### Memory Management

Resource lifetime best practices:
- **Static resources**: Created once, live forever (pipelines, shaders)
- **Per-frame resources**: Command buffers, dropped after submit
- **Dynamic resources**: Reuse buffers, resize when needed
- **Texture atlases**: Grow incrementally, avoid frequent reallocations

### Synchronization

WGPU handles synchronization automatically:
- No explicit fences needed for single queue
- Submit order guarantees execution order
- `write_buffer` is implicitly synchronized

## Integration with Other Systems

### Text Rendering

`astrelis-text::FontRenderer` builds on `astrelis-render`:
- Uses `GraphicsContext` for resource creation
- Manages texture atlas for glyph caching
- Batches text rendering into single draw call per font size

### UI Rendering

`astrelis-ui::UiRenderer` builds on `astrelis-render`:
- Uses `GraphicsContext` for pipeline creation
- Batches UI quads by texture
- Integrates `FontRenderer` for text widgets

### Future: Scene Rendering

Planned scene renderer will include:
- Camera management
- Transform hierarchy
- Material system
- Lighting and shadows
- Post-processing pipeline

## Platform Differences

### Backend Selection

WGPU selects backend based on platform:
- **Windows**: DX12 (primary), Vulkan (fallback)
- **macOS/iOS**: Metal
- **Linux**: Vulkan
- **Web**: WebGPU (browsers with support)

### Surface Formats

Platform-specific preferred formats:
- **Metal**: Bgra8Unorm
- **Vulkan/DX12**: Often Rgba8Unorm or Bgra8Unorm
- **WebGPU**: Rgba8Unorm

The engine automatically selects compatible formats.

### Coordinate Systems

WGPU uses standard conventions:
- **NDC**: [-1, 1] for X and Y, [0, 1] for Z
- **Texture coords**: [0, 1] with (0, 0) at top-left
- **Clip space**: Left-handed (positive Z forward)

## Debugging Tools

### Validation Layers

Enable for development:
```rust
let descriptor = GraphicsContextDescriptor {
    backends: wgpu::Backends::VULKAN,
    // Validation automatically enabled in debug builds
    ..Default::default()
};
```

### Labels

Use labels for GPU debugging:
```rust
let buffer = device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("Vertex Buffer - UI Quads"),
    // ...
});
```

Labels appear in:
- RenderDoc captures
- PIX captures
- Metal Frame Debugger
- Browser DevTools (WebGPU)

### Profiling

GPU profiling via puffin:
```rust
use astrelis_core::profiling::profile_scope;

{
    profile_scope!("render_ui");
    ui_renderer.render(&mut pass);
}
```

## Best Practices

1. **Minimize state changes**: Sort draw calls by pipeline, texture, buffer
2. **Batch geometry**: Combine multiple objects into single draw call
3. **Reuse resources**: Pools for buffers, textures avoid allocations
4. **Update smartly**: Only upload changed data, use staging buffers
5. **Profile regularly**: Use puffin to identify bottlenecks
6. **Test on target**: Performance varies significantly across GPUs
7. **Handle errors**: Surface loss, OOM can occur at runtime
8. **Label resources**: Helps debugging with GPU tools

## Future Enhancements

1. **Compute shaders** - GPU-accelerated calculations
2. **Multi-pass rendering** - Deferred rendering, post-processing
3. **Instancing** - Efficient rendering of repeated objects
4. **Indirect drawing** - GPU-driven rendering
5. **Ray tracing** - Advanced lighting (when WGPU supports)
6. **Mesh shaders** - Next-gen geometry processing