# Render Passes

This guide explains how to use render passes in Astrelis to organize your rendering commands. Learn how to create efficient multi-pass rendering pipelines for advanced effects.

## Overview

A **render pass** represents a single rendering operation to a render target. Render passes:

- Clear or load existing content from attachments
- Execute draw commands
- Store or discard results to attachments
- Support color, depth, and stencil attachments

**Key Concept:** Render passes use RAII (Drop) to automatically submit commands. When the pass goes out of scope, commands are recorded to the command encoder.

**Comparison to Unity:** Similar to Unity's `CommandBuffer`, but with automatic scope management instead of manual `ExecuteCommandBuffer()`.

## Basic Render Pass Usage

### The Recommended Pattern: clear_and_render()

The easiest way to create a render pass is with `clear_and_render()`:

```rust
use astrelis_render::{Color, RenderTarget, RenderableWindow};

let mut frame = renderable_window.begin_drawing();

frame.clear_and_render(
    RenderTarget::Surface,
    Color::rgb(0.2, 0.3, 0.4),
    |pass| {
        // Render commands go here
        ui.render(pass.wgpu_pass());
        sprite_renderer.render(pass.wgpu_pass());
    },
);

frame.finish();
```

**What happens:**
1. Creates a render pass targeting the surface
2. Clears with the specified color
3. Executes the closure with the pass
4. Automatically drops the pass (submits commands)
5. `finish()` presents the frame

**Advantage:** The closure ensures the pass is dropped before `finish()`, preventing common errors.

### Manual Pass Management

For more control, create passes manually:

```rust
use astrelis_render::{RenderPassBuilder, RenderTarget, Color};

let mut frame = renderable_window.begin_drawing();

{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .clear_color(Color::BLACK)
        .build(&mut frame);

    // Render commands
    ui.render(pass.wgpu_pass());

} // Pass drops here automatically

frame.finish();
```

**Critical:** The pass MUST be dropped before `frame.finish()`. Use a scope block `{}` to ensure this.

### Common Error: Pass Not Dropped

```rust
// ❌ ERROR: Pass not dropped before finish()
let mut frame = renderable_window.begin_drawing();

let mut pass = RenderPassBuilder::new()
    .target(RenderTarget::Surface)
    .clear_color(Color::BLACK)
    .build(&mut frame);

ui.render(pass.wgpu_pass());

frame.finish(); // PANIC: pass still borrowed
```

**Fix:** Drop the pass explicitly or use a scope:

```rust
// ✅ GOOD: Explicit drop
let mut pass = RenderPassBuilder::new()
    .target(RenderTarget::Surface)
    .clear_color(Color::BLACK)
    .build(&mut frame);

ui.render(pass.wgpu_pass());
drop(pass); // Explicit drop

frame.finish();
```

## RenderPassBuilder API

### Creating a Builder

```rust
use astrelis_render::RenderPassBuilder;

let builder = RenderPassBuilder::new();
```

### Setting the Render Target

```rust
use astrelis_render::RenderTarget;

// Render to window surface (most common)
builder.target(RenderTarget::Surface);

// Render to framebuffer (for render-to-texture)
builder.target(RenderTarget::Framebuffer(framebuffer_id));
```

See [Render Targets](render-targets.md) for more details.

### Clear Color

```rust
use astrelis_render::Color;

// Clear with solid color
builder.clear_color(Color::rgb(0.0, 0.0, 0.0));

// Clear with alpha
builder.clear_color(Color::rgba(0.0, 0.0, 0.0, 0.5));

// Common colors
builder.clear_color(Color::BLACK);
builder.clear_color(Color::WHITE);
builder.clear_color(Color::TRANSPARENT);
```

**Load Operation:** If you don't call `clear_color()`, the pass will **load** existing content instead of clearing.

### Load/Store Operations

Control how attachments are handled:

```rust
use wgpu::{LoadOp, StoreOp};

// Clear on load (default with clear_color)
builder.color_load_op(LoadOp::Clear(wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
}));

// Load existing content (for multi-pass rendering)
builder.color_load_op(LoadOp::Load);

// Store results (default)
builder.color_store_op(StoreOp::Store);

// Discard results (for temporary rendering)
builder.color_store_op(StoreOp::Discard);
```

**Use Cases:**
- **Clear:** First pass that needs a blank canvas
- **Load:** Second pass that renders on top of first pass
- **Discard:** Depth/stencil buffers you don't need after rendering

### Depth and Stencil Attachments

```rust
// Enable depth testing
builder.depth_stencil_attachment(depth_texture_view);

// Configure depth operations
builder.depth_load_op(LoadOp::Clear(1.0)); // Clear to far plane
builder.depth_store_op(StoreOp::Store);

// Configure stencil operations
builder.stencil_load_op(LoadOp::Clear(0));
builder.stencil_store_op(StoreOp::Discard);
```

**Typical 3D Setup:**
```rust
let mut pass = RenderPassBuilder::new()
    .target(RenderTarget::Surface)
    .clear_color(Color::BLACK)
    .depth_stencil_attachment(depth_view)
    .depth_load_op(LoadOp::Clear(1.0))
    .depth_store_op(StoreOp::Store)
    .build(&mut frame);
```

### Building the Pass

```rust
// Build the pass (consumes builder)
let pass = builder.build(&mut frame);
```

The returned `RenderPass` is a thin wrapper around `wgpu::RenderPass` with automatic Drop handling.

## Multiple Render Passes

### Sequential Passes

Render multiple passes in sequence:

```rust
let mut frame = renderable_window.begin_drawing();

// Pass 1: Render scene to framebuffer
{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Framebuffer(scene_fb))
        .clear_color(Color::BLACK)
        .build(&mut frame);

    scene_renderer.render(pass.wgpu_pass());
} // Pass 1 drops here

// Pass 2: Apply post-processing and render to surface
{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .clear_color(Color::BLACK)
        .build(&mut frame);

    post_process_renderer.render(pass.wgpu_pass(), scene_texture);
} // Pass 2 drops here

frame.finish();
```

**Execution Order:** Passes execute in the order they're created.

### Accumulative Rendering (Load Previous Content)

Build up an image across multiple passes:

```rust
let mut frame = renderable_window.begin_drawing();

// Pass 1: Clear and render background
{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .clear_color(Color::BLACK)
        .build(&mut frame);

    background_renderer.render(pass.wgpu_pass());
}

// Pass 2: Load and render foreground on top
{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .color_load_op(LoadOp::Load) // Don't clear, load existing
        .build(&mut frame);

    foreground_renderer.render(pass.wgpu_pass());
}

// Pass 3: Load and render UI on top
{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .color_load_op(LoadOp::Load)
        .build(&mut frame);

    ui.render(pass.wgpu_pass());
}

frame.finish();
```

**Use Case:** Layering effects without blending complexity.

### Conditional Passes

Only create passes when needed:

```rust
let mut frame = renderable_window.begin_drawing();

// Main scene pass
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        scene_renderer.render(pass.wgpu_pass());
    },
);

// Debug overlay pass (only if debugging)
if debug_mode {
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .color_load_op(LoadOp::Load) // Load scene
        .build(&mut frame);

    debug_overlay.render(pass.wgpu_pass());
}

frame.finish();
```

## Pass Descriptor

The `pass.wgpu_pass()` method returns a `&mut wgpu::RenderPass` for executing draw commands:

```rust
let mut pass = RenderPassBuilder::new()
    .target(RenderTarget::Surface)
    .clear_color(Color::BLACK)
    .build(&mut frame);

let render_pass = pass.wgpu_pass();

// Use wgpu RenderPass API
render_pass.set_pipeline(&pipeline);
render_pass.set_bind_group(0, &bind_group, &[]);
render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
render_pass.draw(0..3, 0..1);
```

**Most systems accept `&mut wgpu::RenderPass`:**
```rust
ui.render(pass.wgpu_pass());
sprite_renderer.render(pass.wgpu_pass());
mesh_renderer.render(pass.wgpu_pass());
```

## Advanced Techniques

### Multi-Sample Anti-Aliasing (MSAA)

Enable MSAA for smoother edges:

```rust
// Create MSAA texture (done once)
let msaa_texture = device.create_texture(&wgpu::TextureDescriptor {
    label: Some("MSAA Texture"),
    size: wgpu::Extent3d {
        width: window_width,
        height: window_height,
        depth_or_array_layers: 1,
    },
    mip_level_count: 1,
    sample_count: 4, // 4x MSAA
    dimension: wgpu::TextureDimension::D2,
    format: surface_format,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
    view_formats: &[],
});

let msaa_view = msaa_texture.create_view(&Default::default());

// Render with MSAA
let mut pass = RenderPassBuilder::new()
    .target(RenderTarget::Surface)
    .clear_color(Color::BLACK)
    .msaa_view(msaa_view) // Add MSAA
    .build(&mut frame);

scene_renderer.render(pass.wgpu_pass());
```

**Result:** GPU automatically resolves MSAA to surface.

### Depth Pre-Pass

Optimize complex scenes with depth pre-pass:

```rust
let mut frame = renderable_window.begin_drawing();

// Pass 1: Depth pre-pass (no color output)
{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Framebuffer(depth_only_fb))
        .depth_stencil_attachment(depth_view)
        .depth_load_op(LoadOp::Clear(1.0))
        .depth_store_op(StoreOp::Store)
        .build(&mut frame);

    scene_renderer.render_depth_only(pass.wgpu_pass());
}

// Pass 2: Color pass with early-Z culling
{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .clear_color(Color::BLACK)
        .depth_stencil_attachment(depth_view)
        .depth_load_op(LoadOp::Load) // Use pre-pass depth
        .depth_store_op(StoreOp::Discard)
        .build(&mut frame);

    scene_renderer.render_color(pass.wgpu_pass());
}

frame.finish();
```

**Benefit:** Expensive fragment shaders only run for visible pixels.

### Shadow Map Pass

Render shadow maps in separate passes:

```rust
let mut frame = renderable_window.begin_drawing();

// Pass 1: Render shadow map from light's perspective
{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Framebuffer(shadow_map_fb))
        .depth_stencil_attachment(shadow_depth_view)
        .depth_load_op(LoadOp::Clear(1.0))
        .depth_store_op(StoreOp::Store)
        .build(&mut frame);

    scene_renderer.render_from_light(pass.wgpu_pass(), light_view_proj);
}

// Pass 2: Render scene with shadows
{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .clear_color(Color::BLACK)
        .depth_stencil_attachment(scene_depth_view)
        .depth_load_op(LoadOp::Clear(1.0))
        .build(&mut frame);

    scene_renderer.render_with_shadows(pass.wgpu_pass(), shadow_texture);
}

frame.finish();
```

### Post-Processing Chain

Chain multiple post-processing effects:

```rust
let mut frame = renderable_window.begin_drawing();

// Pass 1: Render scene to texture
frame.clear_and_render(
    RenderTarget::Framebuffer(scene_fb),
    Color::BLACK,
    |pass| {
        scene_renderer.render(pass.wgpu_pass());
    },
);

// Pass 2: Blur pass
frame.clear_and_render(
    RenderTarget::Framebuffer(blur_fb),
    Color::BLACK,
    |pass| {
        blur_renderer.render(pass.wgpu_pass(), scene_texture);
    },
);

// Pass 3: Bloom pass
frame.clear_and_render(
    RenderTarget::Framebuffer(bloom_fb),
    Color::BLACK,
    |pass| {
        bloom_renderer.render(pass.wgpu_pass(), blur_texture);
    },
);

// Pass 4: Final composite to surface
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        composite_renderer.render(pass.wgpu_pass(), scene_texture, bloom_texture);
    },
);

frame.finish();
```

## Performance Considerations

### Pass Overhead

Each render pass has overhead:
- Attachment load/store operations
- GPU state changes
- Command buffer recording

**Guideline:** Use 1-5 passes per frame for typical games. Avoid hundreds of tiny passes.

### Load vs Clear

```rust
// Expensive: Clear overwrites all pixels
builder.clear_color(Color::BLACK);

// Cheap: Load existing pixels (if you'll overwrite them anyway)
builder.color_load_op(LoadOp::Load);
```

**When to use Load:**
- Second pass that renders on top of first
- Depth buffer with pre-pass
- Full-screen effects that overwrite every pixel

**When to use Clear:**
- First pass of the frame
- Need deterministic initial state
- Rendering to only part of the target

### Store vs Discard

```rust
// Store: Write results to memory (needed for later passes)
builder.depth_store_op(StoreOp::Store);

// Discard: Don't write (faster on mobile GPUs)
builder.depth_store_op(StoreOp::Discard);
```

**When to discard:**
- Depth buffer not needed after rendering
- MSAA resolve texture (intermediate result)
- Temporary render targets

**Mobile Optimization:** Tile-based GPUs (mobile) benefit greatly from Discard operations.

## Comparison to Other Engines

### vs Unity CommandBuffer

**Unity:**
```csharp
var cmd = new CommandBuffer();
cmd.SetRenderTarget(renderTexture);
cmd.ClearRenderTarget(true, true, Color.black);
// ... add commands
Graphics.ExecuteCommandBuffer(cmd);
cmd.Release();
```

**Astrelis:**
```rust
frame.clear_and_render(
    RenderTarget::Framebuffer(render_texture),
    Color::BLACK,
    |pass| {
        // Commands automatically executed on drop
    },
);
```

**Key Difference:** Astrelis uses RAII (Drop) for automatic command submission instead of manual `ExecuteCommandBuffer()`.

### vs Bevy RenderGraph

**Bevy:** Uses a declarative render graph with dependency tracking.

**Astrelis:** Uses imperative render passes with manual ordering.

**Trade-off:**
- Bevy: More flexible, complex setup
- Astrelis: Simpler, more explicit control

## Troubleshooting

### Error: "Pass not dropped before finish()"

**Cause:** The render pass is still borrowed when `frame.finish()` is called.

**Fix:** Use a scope block or `drop(pass)`:
```rust
{
    let mut pass = builder.build(&mut frame);
    // ...
} // Drop here

frame.finish();
```

### Error: "Surface texture is outdated"

**Cause:** Window was resized but surface wasn't recreated.

**Fix:** Handle resize events:
```rust
events.dispatch(|event| {
    if let Event::WindowResized(size) = event {
        renderable_window.resized(*size);
        HandleStatus::consumed()
    } else {
        HandleStatus::ignored()
    }
});
```

### Black Screen

**Common causes:**
1. No draw commands in pass
2. Rendering with incorrect blend mode
3. Depth test failing (all pixels culled)
4. Viewport/scissor misconfigured

**Debug:** Add puffin profiling:
```rust
puffin::profile_scope!("render_pass");
let mut pass = builder.build(&mut frame);
// ... render commands
```

Check if draw commands are actually executing.

## Next Steps

- **Practice:** Try the `render_graph_demo` example to see multi-pass rendering
- **Learn More:** [Render Targets](render-targets.md) for framebuffer setup
- **Advanced:** [Custom Shaders](custom-shaders.md) for custom rendering
- **Optimize:** [Performance](../architecture/performance.md) for render pass optimization

## See Also

- [Render Targets](render-targets.md) - Rendering to textures
- [Rendering Fundamentals](../getting-started/04-rendering-fundamentals.md) - Basic rendering concepts
- API Reference: [`RenderPassBuilder`](../../api/astrelis-render/struct.RenderPassBuilder.html)
- API Reference: [`FrameContext`](../../api/astrelis-render/struct.FrameContext.html)
