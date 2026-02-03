# Rendering Fundamentals

In this guide, you'll learn Astrelis's rendering patterns and understand how to control the GPU rendering pipeline. Unlike engines like Unity that handle rendering automatically, Astrelis gives you explicit control over render passes, frame contexts, and GPU commands.

## Prerequisites

- Completed [Hello Window](03-hello-window.md)
- Basic understanding of graphics concepts (vertices, shaders, textures)

## Core Rendering Concepts

### The Three-Context Pattern

Astrelis uses three contexts for rendering:

```
GraphicsContext (Arc)
    ↓ owns
WindowContext (per window)
    ↓ creates
FrameContext (per frame, RAII)
    ↓ creates
RenderPass (scoped, RAII)
```

#### 1. GraphicsContext

**Owns GPU resources**: device, queue, adapter

```rust
let graphics = GraphicsContext::new_owned_sync();  // Creates Arc<GraphicsContext>
```

**Shared ownership** with `Arc` - clone it cheaply:

```rust
let ui_system = UiSystem::new(graphics.clone(), window_manager.clone());
let renderer = CustomRenderer::new(graphics.clone());
```

**Why Arc?**: Multiple systems need GPU access. Arc provides automatic cleanup when the last owner drops.

#### 2. WindowContext

**Per-window surface** (the actual screen buffer)

Created automatically by `RenderableWindow`:

```rust
let renderable = RenderableWindow::new(window, graphics.clone());
```

Handles surface configuration, format, present mode (VSync).

#### 3. FrameContext

**Per-frame rendering state** - created by `begin_drawing()`:

```rust
let mut frame = renderable.begin_drawing();  // Acquires surface texture, creates encoder
```

**RAII lifecycle**:
- **Creation**: Acquires surface texture, creates command encoder
- **Usage**: Build render passes, record commands
- **Drop** (`finish()`): Submits commands to GPU, presents to screen

**Important**: Must call `finish()` or commands are never submitted!

#### 4. RenderPass

**Scoped rendering context** for drawing:

```rust
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        // pass: &mut RenderPass
        // Draw commands go here
    }, // pass is automatically dropped here
);
```

**Why closures?**: Ensures render pass is dropped before `frame.finish()`.

### Render Targets

You can render to two targets:

#### Surface (the window)

```rust
RenderTarget::Surface
```

Renders directly to the screen. This is what users see.

#### Framebuffer (texture)

```rust
RenderTarget::Framebuffer(&my_framebuffer)
```

Renders to an offscreen texture for:
- **Post-processing**: Apply effects like blur, bloom
- **Render-to-texture**: Use rendered content as a texture
- **Multiple passes**: Render scene, then composite

Example:
```rust
// Create framebuffer
let framebuffer = Framebuffer::builder(1920, 1080)
    .format(wgpu::TextureFormat::Rgba8UnormSrgb)
    .label("My Framebuffer")
    .build(&graphics);

// Render to it
frame.clear_and_render(
    RenderTarget::Framebuffer(&framebuffer),
    Color::BLACK,
    |pass| {
        // Render scene to texture
    },
);

// Later, use framebuffer.color_view() as a texture
```

## The Render Pass Pattern

### Automatic Pass Management (Recommended)

Use `clear_and_render()` for automatic scoping:

```rust
frame.clear_and_render(
    RenderTarget::Surface,
    Color::rgb(0.1, 0.1, 0.1),  // Clear color
    |pass| {
        // pass is a &mut RenderPass
        let render_pass = pass.wgpu_pass();  // Get wgpu::RenderPass

        // Record draw commands
        render_pass.set_pipeline(&pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..6, 0..1);
    }, // pass automatically dropped here
);
```

**Benefits**:
- Automatic render pass lifecycle
- No manual Drop required
- Guaranteed correct ordering

### Manual Pass Management (Advanced)

For complex scenarios, create passes manually:

```rust
use astrelis_render::RenderPassBuilder;

{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .clear_color(Color::BLACK)
        .build(&mut frame);

    let render_pass = pass.wgpu_pass();
    render_pass.set_pipeline(&pipeline);
    render_pass.draw(0..3, 0..1);
} // MUST drop pass before finish()

frame.finish();
```

**Critical**: Pass must be dropped before `frame.finish()` or you'll get a runtime error.

## Comparison to Other Engines

### Unity's Rendering

Unity handles rendering automatically:

```csharp
// Unity: No manual rendering
public class MyBehaviour : MonoBehaviour {
    void Update() {
        // Game logic
    }
    // Rendering happens automatically via Camera
}
```

**SRP (Scriptable Render Pipeline)** for custom rendering:

```csharp
void Render(ScriptableRenderContext context, Camera camera) {
    CommandBuffer cmd = CommandBufferPool.Get();
    cmd.ClearRenderTarget(true, true, Color.black);
    cmd.Blit(source, destination);
    context.ExecuteCommandBuffer(cmd);
    context.Submit();
}
```

### Astrelis's Rendering

Astrelis requires explicit control:

```rust
fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, _events: &mut EventBatch) {
    // You control everything
    let mut frame = self.window.begin_drawing();

    frame.clear_and_render(
        RenderTarget::Surface,
        Color::BLACK,
        |pass| {
            // Your rendering code
        },
    );

    frame.finish();  // You must submit
}
```

**Similarity**: Astrelis's `FrameContext` ≈ Unity's `CommandBuffer`

**Difference**: Astrelis is lower-level. You control pass creation, not just commands.

## Multiple Render Passes

You can have multiple passes per frame:

```rust
fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, _events: &mut EventBatch) {
    let mut frame = self.window.begin_drawing();

    // Pass 1: Render scene to framebuffer
    frame.clear_and_render(
        RenderTarget::Framebuffer(&self.scene_fb),
        Color::rgb(0.0, 0.0, 0.0),
        |pass| {
            self.render_scene(pass.wgpu_pass());
        },
    );

    // Pass 2: Apply post-processing
    frame.clear_and_render(
        RenderTarget::Framebuffer(&self.post_fb),
        Color::BLACK,
        |pass| {
            self.render_post_process(pass.wgpu_pass(), &self.scene_fb);
        },
    );

    // Pass 3: Render to screen
    frame.clear_and_render(
        RenderTarget::Surface,
        Color::BLACK,
        |pass| {
            // Draw final result
            self.render_final(pass.wgpu_pass(), &self.post_fb);
            // Draw UI on top
            self.ui.render(pass.wgpu_pass());
        },
    );

    frame.finish();
}
```

**Pass ordering**: Passes execute in the order you call them.

## GraphicsContext Deep Dive

### Creation

```rust
// Synchronous creation (blocks until GPU is ready)
let graphics = GraphicsContext::new_owned_sync();  // Returns Arc<GraphicsContext>

// Or panic if creation fails
let graphics = GraphicsContext::new_owned_sync_or_panic();

// Async creation (non-blocking)
let graphics = GraphicsContext::new_owned_async().await;
```

**Recommendation**: Use `new_owned_sync_or_panic()` for simplicity.

### Backend Selection

By default, Astrelis picks the best backend for your platform:

- **macOS/iOS**: Metal
- **Windows**: DirectX 12 (with Vulkan fallback)
- **Linux**: Vulkan
- **Web**: WebGPU

**Force a backend** (rarely needed):

```rust
use astrelis_render::Backend;

let graphics = GraphicsContext::new_with_backend(Backend::Vulkan, true).await;
```

### GPU Information

Query GPU capabilities:

```rust
let info = graphics.info();
println!("Backend: {:?}", info.backend);
println!("Adapter: {}", info.name);
println!("Driver: {}", info.driver);

// Check features
if graphics.device().features().contains(wgpu::Features::TIMESTAMP_QUERY) {
    println!("Timestamp queries supported");
}
```

### Limits

Check GPU limits:

```rust
let limits = graphics.device().limits();
println!("Max texture size: {}x{}", limits.max_texture_dimension_2d, limits.max_texture_dimension_2d);
println!("Max bind groups: {}", limits.max_bind_groups);
```

## Frame Lifecycle

Understanding the complete frame lifecycle:

```rust
fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    // 1. Begin frame (acquires surface texture, creates encoder)
    let mut frame = self.window.begin_drawing();

    // 2. Build render passes and record commands
    frame.clear_and_render(
        RenderTarget::Surface,
        Color::BLACK,
        |pass| {
            // Commands are recorded here (not executed yet!)
        },
    ); // Render pass ends here

    // 3. Finish frame (submits to GPU, presents to screen)
    frame.finish();
}
```

**Execution timing**:
1. Commands are **recorded** during render passes
2. Commands are **submitted** during `finish()`
3. GPU **executes** commands asynchronously
4. **Present** swaps buffers (shows result on screen)

This is why you must drop passes before `finish()` - commands must be fully recorded before submission.

## RenderableWindow Configuration

Customize surface configuration:

```rust
use astrelis_render::WindowContextDescriptor;
use wgpu::{PresentMode, TextureFormat};

let renderable = RenderableWindow::new_with_descriptor(
    window,
    graphics.clone(),
    WindowContextDescriptor {
        format: Some(TextureFormat::Bgra8UnormSrgb),  // Surface format
        present_mode: PresentMode::Mailbox,           // Triple buffering
        alpha_mode: wgpu::CompositeAlphaMode::Opaque,
        desired_maximum_frame_latency: None,
    },
);
```

### Present Modes

- **`Fifo` (default)**: VSync on, smooth but may have latency
- **`FifoRelaxed`**: VSync on, but allows tearing if GPU can't keep up
- **`Immediate`**: VSync off, lowest latency but may tear
- **`Mailbox`**: Triple buffering, smooth and responsive (if supported)

### Texture Formats

Common surface formats:
- `Bgra8UnormSrgb` (Windows preferred)
- `Rgba8UnormSrgb` (cross-platform)
- `Bgra8Unorm` (no sRGB correction)

## Handling Surface Loss

Windows can lose their surface (minimize, resize, GPU reset). Handle gracefully:

```rust
fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    // Handle resize events
    events.dispatch(|event| {
        use astrelis_winit::event::{Event, HandleStatus};
        if let Event::WindowResized(size) = event {
            self.window.resized(*size);
            HandleStatus::consumed()
        } else {
            HandleStatus::ignored()
        }
    });

    // Check window isn't minimized
    let size = self.window.size();
    if size.width == 0 || size.height == 0 {
        return;  // Skip rendering
    }

    // Try to begin drawing (may fail if surface lost)
    let mut frame = match self.window.try_begin_drawing() {
        Ok(frame) => frame,
        Err(e) => {
            tracing::warn!("Failed to begin drawing: {:?}", e);
            return;  // Skip this frame
        }
    };

    frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |_| {});
    frame.finish();
}
```

## Performance Considerations

### Frame Budget

At 60 FPS, you have ~16.67ms per frame:

```
Frame budget (60 FPS):
- Update logic: ~5ms
- Rendering: ~10ms
- GPU execution: ~1-2ms
```

**Profile with puffin**:

```rust
use astrelis_core::profiling::{init_profiling, ProfilingBackend};

fn main() {
    init_profiling(ProfilingBackend::PuffinHttp);
    // ... run app
}
```

Access profiler at `http://127.0.0.1:8585`.

### Minimize Pass Count

Each render pass has overhead. Combine when possible:

```rust
// Bad: Too many passes
frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
    self.render_background(pass.wgpu_pass());
});
frame.clear_and_render(RenderTarget::Surface, Color::TRANSPARENT, |pass| {
    self.render_sprites(pass.wgpu_pass());  // Clears again!
});

// Good: One pass
frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
    self.render_background(pass.wgpu_pass());
    self.render_sprites(pass.wgpu_pass());
});
```

**Exception**: Render-to-texture requires separate passes.

### Batch Draw Calls

Minimize draw calls by batching similar objects:

```rust
// Bad: One draw call per sprite (1000 sprites = 1000 calls)
for sprite in &self.sprites {
    pass.draw(sprite.vertices, sprite.indices);
}

// Good: Instance buffer (1000 sprites = 1 call)
pass.draw_instanced(0..6, 0..self.sprites.len() as u32);
```

Astrelis's UI system uses instancing automatically.

## Common Patterns

### Render-to-Texture for Post-Processing

```rust
// Setup (once)
let scene_fb = Framebuffer::builder(1920, 1080)
    .format(wgpu::TextureFormat::Rgba8UnormSrgb)
    .build(&graphics);

// Render (every frame)
fn render(&mut self, ...) {
    let mut frame = self.window.begin_drawing();

    // Render scene to texture
    frame.clear_and_render(
        RenderTarget::Framebuffer(&scene_fb),
        Color::BLACK,
        |pass| { self.render_3d_scene(pass); },
    );

    // Apply blur effect and render to screen
    frame.clear_and_render(
        RenderTarget::Surface,
        Color::BLACK,
        |pass| {
            self.render_fullscreen_quad(pass, scene_fb.color_view(), &self.blur_shader);
        },
    );

    frame.finish();
}
```

### UI Overlay on 3D Scene

```rust
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        // 3D scene first
        self.render_3d_scene(pass.wgpu_pass());

        // UI on top (drawn after, appears on top)
        self.ui.render(pass.wgpu_pass());
    },
);
```

### Multi-Window Rendering

```rust
fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    // Render different content per window
    if window_id == self.main_window_id {
        self.render_game_view();
    } else if window_id == self.editor_window_id {
        self.render_editor_view();
    }
}
```

Each window has its own `RenderableWindow`, but they share the `Arc<GraphicsContext>`.

## Next Steps

Now that you understand rendering fundamentals:

1. **[First UI](05-first-ui.md)** - Add UI to your app
2. **[Custom Shaders Guide](../../rendering/custom-shaders.md)** (Phase 3) - Write custom shaders
3. **[Render Passes Guide](../../rendering/render-passes.md)** (Phase 3) - Advanced pass management

## Summary

**Key takeaways**:
- **GraphicsContext**: GPU access (shared with Arc)
- **FrameContext**: Per-frame rendering (RAII, call finish())
- **RenderPass**: Scoped drawing context (use closures)
- **RenderTarget**: Surface (screen) or Framebuffer (texture)
- **Multiple passes**: Allowed, execute in order
- **Handle surface loss**: Check size, handle resize events

You now have the foundation for rendering in Astrelis. The next guide will show you how to add UI to your application!
