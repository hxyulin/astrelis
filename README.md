# Astrelis

The `next` branch develops a rendering API directly over wgpu in one workspace
crate. The earlier implementation remains on `main`.

Applications own windows, event loops, draw order, and scheduling. `GraphicsContext`
handles GPU initialization and resource creation. Independent renderers record
into scoped passes; a frame submits once and presents only when it owns a surface.

## Ownership and a first draw

```rust,no_run
use astrelis::{GraphicsContext, MeshRenderer, SurfaceOptions, Vertex, wgpu};

async fn draw(window: std::sync::Arc<winit::window::Window>)
    -> Result<(), Box<dyn std::error::Error>> {
    let size = window.inner_size();
    let (graphics, mut target) = GraphicsContext::with_surface(
        window, SurfaceOptions::new(size.width, size.height),
    ).await?;
    let mesh = graphics.create_mesh(&[
        Vertex::new([0., 0.7, 0.], [1., 0., 0., 1.]),
        Vertex::new([-0.7, -0.7, 0.], [0., 1., 0., 1.]),
        Vertex::new([0.7, -0.7, 0.], [0., 0., 1., 1.]),
    ], &[0, 1, 2])?;
    let mut renderer = MeshRenderer::new(&graphics);
    renderer.prepare_for_target(&target)?;
    // Keep resources between redraws in a real application.
    let mut frame = target.begin_frame()?;
    {
        let mut pass = frame.render_pass().clear_color(wgpu::Color::BLACK).begin()?;
        renderer.draw(&mut pass, &mesh)?;
    }
    frame.finish()?;
    Ok(())
}
```

A context owns a wgpu instance, adapter, device, and queue. Clones share existing
handles, upload storage, and managed submission ordering. Add windows with
`graphics.create_surface(window, options)`; every surface must support the selected
adapter. `GraphicsContext::headless()` initializes without a window, and
`GraphicsContext::from_wgpu(...)` adopts custom features and limits.

`begin_frame()` returns `Result<Frame, FrameError>`. Retry schedules a later redraw;
suspension waits for size/visibility changes. A lost surface is recreated by the
application. Frame borrows prevent resize or overlapping acquisition of its default
target. Scoped passes prevent simultaneous encoder access. Dropping a frame abandons
commands and uploads; `finish()` returns a submission index without waiting for GPU
completion. Surface output must be initialized, including an MSAA resolve, before
presentation. Reconfiguration can synchronize with the GPU.

## Pass operations

Every default pass clears transparent color, depth to 1, and stencil to 0, and stores
available aspects. `load_color()` preserves only color; `load_all()` preserves all
available aspects. Individual clear/load selectors and `color_ops`, `depth_ops`, and
`stencil_ops` provide exact control. `depth_ops(None)` / `stencil_ops(None)` make an
aspect read-only. `without_depth_stencil()` omits the attachment entirely.

```rust
let mut pass = frame.render_pass()
    .load_all()
    .stencil_reference(1)
    .scissor_rect(0, 0, width, height)
    .begin()?;
renderer.draw(&mut pass, &mesh)?;
pass.set_scissor_rect(10, 10, 100, 100)?; // Applies to subsequent draws.
renderer.draw(&mut pass, &overlay)?;
```

MSAA and depth/stencil storage belong to target creation:

```rust
let options = SurfaceOptions::new(width, height)
    .sample_count(4)
    .depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8);
```

Targets cache supported sample counts and reuse attachments until size or sample
count changes. Unsupported counts return an error. Zero size suspends storage.
Passes resolve MSAA by default; `resolve(false)` skips intermediate resolves.
Keep intermediate samples with `Store`. A final resolving pass can use color
`Discard` to discard samples while retaining resolved output. Resolve and sample
initialization are tracked separately. Depth/stencil storage is never resolved.

## Images, placement, and materials

Image bindings retain source and sampler resources. Placement, UVs, tint, and affine
transform are supplied per draw, so moving or fading an image does not rebuild a
sampler, binding, or uniform buffer.

```rust
let texture = graphics.create_texture(TextureOptions::new(1, 1))?;
texture.write(&[255, 255, 255, 255])?;
let mut textures = TextureRenderer::new(&graphics);
let image = textures.create_binding(texture.view(), TextureBindingOptions::new())?;
textures.prepare(&image, &target.render_format())?;

let mut frame = target.begin_frame()?;
{
    let mut pass = frame.render_pass().begin()?;
    textures.draw(&mut pass, &image,
        TextureDraw::new(Rect::new(x, y, 80., 60.))
            .uv(UvRect::new(0., 0., 1., 1.))
            .tint([1., 1., 1., opacity]))?;
    textures.draw_many(&mut pass, &image, &rectangles)?;
}
frame.finish()?;
```

`TextureDraw::new(Rect)` uses physical pixels relative to the viewport's top-left;
`TextureDraw::normalized(Rect)` uses viewport fractions. `UvRect` uses normalized
texture coordinates. Default drawing fills the viewport. Tint RGB is linear and
opacity is straight alpha. Instance parameters occupy dense 64-byte records in
recording-owned pages. CPU and GPU page storage are pooled; finish uploads each
used page once. Pages remain leased until submit/drop, so overlapping recordings
retain distinct parameters. First use or increased capacity can allocate storage.
`draw_many` instances compatible consecutive draws in input order, splitting large
batches by page capacity. No automatic sorting occurs. Static content can use `prepare_draws(draws, viewport_size)`
and `draw_prepared` to retain immutable instance data with no recurring parameter
uploads. Pixel rectangles require the preparation viewport size; normalized rectangles
adapt to any viewport. Image bindings and materials remain separate.

`TextureBindingOptions` selects source alpha and filtering. Convenience samplers
are reused; `create_binding_with_sampler` accepts custom addressing/filtering/mip
settings. A `TextureMaterial` holds immutable shader, blending, and depth/stencil
settings, allowing textured stencil clips. Create one through
`graphics.create_texture_material(TextureMaterialOptions::new())`, then use
`draw_with_material` or `draw_many_with_material`. Additional shader resource groups
can be bound with `pass.set_bind_group(...)`.

## Framebuffers and live sampling

```rust
let mut layer = graphics.create_framebuffer(
    FramebufferOptions::new(width, height).sample_count(4),
)?;
let image = textures.create_sampled_binding(&layer.sampled_color())?;
let mut frame = target.begin_frame()?;
{
    let mut pass = frame.render_to(&mut layer).begin()?;
    meshes.draw(&mut pass, &mesh)?;
}
{
    let mut pass = frame.render_pass().begin()?;
    textures.draw(&mut pass, &image, TextureDraw::default())?;
}
frame.finish()?;
```

Framebuffer output is single-sampled and persists across submissions. A live
`SampledColor` follows resize/MSAA replacements; its binding refreshes only when
the output view changes. Suspended sources reject drawing. Live sources default
to premultiplied alpha, matching built-in framebuffer shading. Custom shaders must
declare the correct encoding through `create_sampled_binding_with_options`.
Snapshot bindings remain available through `create_binding` and explicit `rebind`.

Managed attachment operations commit in submission order. Initial loads require a
clear/store or explicitly recorded full write. External load dependencies are
checked again at finish; an intervening discard can reject submission. Dropping a
recording does not commit its effects. Resizing creates a fresh storage generation;
previous descriptors and GPU commands retain old storage. Applications still choose
shared-resource ordering; there is no automatic dependency scheduler.

## Custom passes, geometry, and shaders

`Frame::begin_render_pass` takes a `RenderPassDescriptor` with zero or multiple color
outputs, optional depth/stencil, selected mip/layer views, and optional queries.
Obtain managed snapshots with `frame.color_attachment()`,
`frame.depth_stencil_attachment()`, or the corresponding framebuffer methods.
Use `RenderColorAttachment::new(view, format, mip_size)` and
`RenderDepthStencilAttachment::new(...)` for imported views. Both custom and default
passes produce the same wrapped `RenderPass`. Built-in renderers require one color
output at slot zero; application renderers can use other configurations.

```rust
let colors = [Some(frame.color_attachment()?)];
let depth = frame.depth_stencil_attachment()?;
let mut pass = frame.begin_render_pass(&RenderPassDescriptor {
    colors: &colors,
    depth_stencil: depth.as_ref(),
    ..Default::default()
})?;
application_renderer.draw(pass.as_wgpu());
```

`pass.as_wgpu()` supports custom drawing and invalidates tracked state. The next
wrapped draw restores its required raster, pipeline, geometry, and image bindings.
Wrapped setters affect subsequent draws. `pass.set_bind_group(...)` supports
application bindings without invalidating raster state. `frame.encoder()` supports
copies/compute between passes. Raw writes do not automatically update managed
initialization; use managed custom passes or `frame.write_framebuffer_color(...)`
for an explicitly recorded full-output copy/compute write. That callback must
initialize every output texel and does not initialize separate MSAA samples.
Independently submitted raw commands do not participate in managed tracking.

The simple `create_mesh(vertices, indices)` path remains. `create_mesh_with_options`
accepts `VertexStream` layouts, optional `MeshIndices::U16/U32`, topology, and dynamic
update usages. `create_mesh_from_buffers` retains application-owned ranges.
`MeshDraw` selects geometry ranges, instances, and base vertex. Explicit vertex/index
updates keep capacity and layout fixed. Queue writes execute before the next
submission, not between draws; encoder copies give precise command ordering.

Materials declare matching `vertex_layouts`, topology, and optional strip index
format. `RenderFormat` describes attachment formats and samples without acquiring a
frame. `MeshRenderer::try_prepare_material` and
`TextureRenderer::try_prepare_material` return pipeline diagnostics and cache only
validated pipelines. Raw shader-module/layout creation still uses wgpu error scopes.
Convenience preparation/drawing can create pipelines lazily. Prepared cache hits
create no pipelines and perform no format capability queries.

## Standalone examples and development

```sh
cargo run -p astrelis --example triangle
cargo run -p astrelis --example meshes
cargo run -p astrelis --example materials
cargo run -p astrelis --example depth
cargo run -p astrelis --example stencil
cargo run -p astrelis --example msaa
cargo run -p astrelis --example framebuffer
cargo run -p astrelis --example geometry
cargo run -p astrelis --example passes
cargo run -p astrelis --example textures
cargo run -p astrelis --example compositing
cargo run -p astrelis --example multi_window
```

Every example is one copyable file with its own windows, event handling, resize,
redraw scheduling, and surface-loss recovery. There is no support module or smoke
test mode. `winit` and `pollster` are development dependencies. The texture example
reuses image bindings across placements; compositing follows framebuffer replacement
without explicit rebinding. The geometry example demonstrates custom streams,
Uint16 indices, and instancing; passes demonstrates a managed custom surface pass
with a final resolve and discarded MSAA samples.

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

GPU tests read back pixels for drawing order, blending, clipping, MSAA, depth/stencil,
custom geometry, updates, source replacement, and distinct parameters across
independent recordings. Regression tests cover submission order, abandonment,
load dependencies, custom attachment tracking, and failed shader preparation.
Documentation includes borrowing examples and compile-fail lifetime checks.

MIT. See [LICENSE-MIT](LICENSE-MIT).
