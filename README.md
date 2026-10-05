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

## Shared 2D drawing semantics

`Rect`, `DrawSpace`, and `Transform2D` describe geometry relative to the current
viewport: X right, Y down, physical pixels by default. Normalized units scale each
axis by the viewport size. Applications apply DPI scaling explicitly. Transforms
act in the selected units before viewport conversion; `a.then(b)` applies `a`,
then `b`. `TextureDraw::space` and `transform_2d` use these same conventions.

`ShapeDraw` describes filled or outlined rectangles, uniformly rounded rectangles,
and ellipses. `.stroke(Stroke::new(width))` selects a centered outline; `.inside()`
and `.outside()` place all of its width on the chosen side of the boundary.
Rectangle outlines have sharp corners; rounded rectangles offset their corner
radii, clamping collapsed inner radii to zero. Ellipse outlines use a distance
offset of the original curve, with finite-precision shader calculations.
Width uses the selected draw units and transforms with geometry. A stroke that
consumes the interior becomes solid; zero width or zero extents draw nothing.
`LineDraw` describes independent segments with width and butt, square, or round caps.
Widths/radii transform with geometry. Inputs use finite linear, straight RGBA
colors with alpha in `0..=1`; rendering outputs premultiplied source-over color.
Zero-area geometry draws nothing. Paths and connected stroke joins
are outside these primitive types. Shader edge coverage is independent of
attachment MSAA, selected with `EdgeAntialiasing`.

`ShapeRenderer` and `LineRenderer` record these primitives directly into existing
passes. Both provide `draw`, `draw_many`, and `bind` scopes. Batches validate every
item before drawing, preserve order, and split by upload capacity. Warmed workloads
reuse buffers, pipeline variants, and CPU scratch. Defaults disable depth/stencil
tests and writes so primitives can overlay a 3D pass, including read-only aspects.
Custom stencil-tested primitives can use mesh materials.

```rust
let mut shapes = ShapeRenderer::new(&graphics);
let mut lines = LineRenderer::new(&graphics);
shapes.prepare(&target.render_format())?;
lines.prepare(&target.render_format())?;

let mut pass = frame.render_pass().begin()?;
shapes.draw(&mut pass,
    ShapeDraw::rounded_rect(Rect::new(20., 20., 120., 60.), 12., [0.1, 0.3, 0.6, 1.]))?;
let mut strokes = lines.bind(&mut pass)?;
strokes.draw(LineDraw::new([30., 50.], [130., 50.], [1.; 4])
    .width(3.).cap(LineCap::Round))?;
```

## Painter

`Painter` retains a shape, line, image, and text renderer and lends an immediate painting
session on an existing pass. It preserves call order and exposes explicit batches;
it does not buffer a display list. Borrowed transform scopes apply local geometry
transforms without changing the parent's transform. They leave viewport, clipping,
and application bindings under pass control. Colors and coordinate units are the
same as the lower-level draw types.

```rust
let mut painter = Painter::new(&graphics);
painter.prepare(&target.render_format())?;

let mut frame = target.begin_frame()?;
{
    let mut pass = frame.render_pass().begin()?;
    let mut paint = painter.begin(&mut pass)?;
    paint.fill_rounded_rect(Rect::new(20., 20., 120., 60.), 12., [0.1, 0.3, 0.6, 1.])?;
    paint.stroke_rounded_rect(Rect::new(20., 20., 120., 60.), 12.,
        Stroke::new(2.).inside(), [0.3, 0.6, 0.9, 1.])?;
    {
        let mut local = paint.transformed(Transform2D::translation(30., 30.))?;
        local.draw_line(LineDraw::new([0., 0.], [80., 0.], [1.; 4])
            .width(3.).cap(LineCap::Round))?;
    }
    // Interleave custom renderer calls through paint.pass() in this same pass.
    paint.fill_ellipse(Rect::new(20., 20., 40., 40.), [1., 0., 0., 0.5])?;
}
frame.finish()?;
```

Image bindings use `create_image_binding` or `create_sampled_binding`, with
`prepare_image` for the binding/attachment variant. A session offers `draw_image`,
`draw_shapes`, `draw_lines`, and `draw_images`. Transformed batches reuse CPU scratch;
identity-transform batches delegate directly. `stroke_rect`, `stroke_rounded_rect`,
and `stroke_ellipse` provide outline conveniences; fills and outlines can share
one explicit `draw_shapes` batch. `shapes()`, `lines()`, `textures()`, and `text()`
expose the owned renderers outside an active session for direct scopes, preparation,
custom image materials/samplers, and immutable prepared image data. Painter owns no
window, frame, scene, or UI layout. There is no session flush or finish operation.

## Scoped drawing

For consecutive draws that keep the same mesh/material or image/material, bind a
scope to amortize immutable compatibility checks and pipeline selection:

```rust
let mut draws = meshes.bind(&mut pass, &mesh)?;
draws.draw();
draws.pass().set_scissor_rect(x, y, width, height)?;
draws.draw_range(&mesh.full_draw().instances(0..instance_count))?;
```

Texture scopes support changing rectangles with `textures.bind(&mut pass, &image)?`
and `.draw(draw)` / `.draw_many(draws)`. For immutable instance data:

```rust
let mut draws = textures.bind_prepared(&mut pass, &image, &prepared)?;
draws.draw()?;
```

Scopes keep the pass open and preserve explicit draw order. `pass()` gives access
to clipping, application bindings, other renderers, and raw wgpu operations; the
next scoped draw restores renderer-owned state. Prepared pixel-space data is
revalidated after pass access if the viewport changes. Application-owned bind
groups remain caller-controlled.

Mesh and directly prepared texture scopes retain pipeline handles, so the same
renderer can be reused through their pass access. Dynamic texture scopes borrow
renderer scratch storage for batches. Live framebuffer images are snapshots within
a scope; a new scope follows later storage replacement. Individual draw methods
remain available for simple and mixed rendering.

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

## CPU text layout

`TextSystem` owns fonts and shaping caches independently of a graphics context.
Font loading and system-font discovery are explicit. `TextBuffer` retains text,
style, width, wrapping, and alignment; evaluation returns an immutable
`Arc<TextLayout>` with local-unit measurement, positioned glyphs, whole-buffer UTF-8
clusters, bidi levels, line boxes, selected fonts, and missing-glyph ranges.

```rust
let mut text = TextSystem::new();
text.load_font(include_bytes!("fonts/MyFont.ttf"))?;
let mut label = TextBuffer::new();
label.set_text("Hello, office!", TextStyle::new().family("My Font"))?;
label.set_width(Some(320.))?;
let layout = label.layout(&mut text)?;
let measured_size = layout.size();
```

Advanced shaping uses cosmic-text with font fallback, kerning, ligatures, and
complex-script support. Unchanged evaluation reuses its snapshot; width changes
reuse shaped runs. Snapshots retain text and font data across edits and font loading.
Measurement requires no window/GPU and does not apply DPI or compute pixel ink bounds.
This milestone provides one style per buffer; rich spans and text editing/hit testing
follow separately. See the
[text architecture and plan](docs/text.md).

`TextRenderer` explicitly prepares coverage/color glyphs or outline distance fields, with immutable geometry:

```rust
let mut renderer = TextRenderer::new(&graphics);
renderer.prepare(&target.render_format())?;
let prepared = renderer.prepare_text(&layout,
    TextRasterOptions::new().scale_factor(dpi_scale))?;
// Reuse prepared text across frames; origin, color and opacity remain per draw.
renderer.draw(&mut pass, &prepared,
    TextDraw::new([20., 30.]).color([0.8, 0.9, 1., 1.]))?;
```

Raster scale applies DPI once to glyphs and layout positions. Draw placement uses
viewport-relative physical pixels. Coverage is colored with linear RGBA; intrinsic
color glyphs retain their RGB and receive draw opacity. Batches preserve layout
order and current clipping. A draw uploads 48 bytes of placement/color parameters,
with no shaping, rasterization, or recurring glyph-geometry upload. Atlas budgets
include prepared texts, recordings, and GPU completion leases. Cache exhaustion
returns `AtlasFull`; callers control resource release and polling. Painter exposes
this same explicit preparation and retained drawing:

```rust
painter.prepare(&target.render_format())?; // Primitive and text pipelines.
let prepared = painter.prepare_text(&layout,
    TextRasterOptions::new().scale_factor(dpi_scale))?;
// During a later painting session:
let mut paint = painter.begin(&mut pass)?;
let mut local = paint.transformed(Transform2D::translation(20., 30.))?;
local.draw_text(&prepared, TextDraw::default().color([0.8, 0.9, 1., 1.]))?;
```

Text origins and session transforms are physical pixels relative to the viewport.
DPI is already applied during preparation, so a session DPI scale would scale the
glyphs again. Text preserves call order with shapes, lines, images, and custom work
through `paint.pass()`. `painter.text()` exposes atlas statistics and cache control;
replace it with `TextRenderer::with_options(...)` to select custom budgets. Distance
fields remain a separate milestone.

## Standalone examples and development

The workspace requires Rust 1.98.1.

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
cargo run -p astrelis --example ui_workload
cargo run -p astrelis --example scene_3d
cargo run -p astrelis --example mixed_2d
cargo run -p astrelis --example painter
cargo run -p astrelis --example text_layout
cargo run -p astrelis --example text
cargo run -p astrelis --example painter_text
cargo run -p astrelis --example text_quality
```

Every windowed example is one copyable file with its own windows, event handling, resize,
redraw scheduling, and surface-loss recovery. There is no support module or smoke
test mode. `winit` and `pollster` are development dependencies. The texture example
combines immutable prepared placement with a shared image scope for cropping and
filtering. Geometry and the 3D scene use scoped mesh draws with custom streams,
Uint16 indices, and instancing; materials shows application bindings through the
scope's pass access. The UI workload combines batched cards with scoped controls.
Compositing follows framebuffer replacement without explicit rebinding; passes
demonstrates a managed custom surface pass with a final resolve and discarded MSAA
samples.

`text_layout` is a standalone CPU example using bundled licensed fonts, with
multilingual fallback, glyph/cluster inspection, headless measurement, snapshot
reuse, and reflow. Run `cargo bench -p astrelis --bench text` to measure CPU stages;
see the [CPU text baseline](docs/performance/text.md) for results and boundaries.
`text` adds a standalone window with DPI preparation, clipping, multilingual fallback,
COLR/PNG color glyph fixtures, resizing/reflow, and Space-controlled MSAA. The
`text_rendering` benchmark separates GPU-text CPU stages, with completion outside timing;
see the [GPU-text CPU baseline](docs/performance/text-rendering.md).
`painter_text` combines retained multilingual text, primitive layers, scoped transforms and pass
access, DPI-aware resize/reflow, and a custom mesh in a standalone window. Run
`cargo bench -p astrelis --bench painter_text` for a matched CPU recording comparison
against direct text rendering; see the [Painter text baseline](docs/performance/painter-text.md).

`mixed_2d` combines filled primitives, line caps, transformed ellipses, translucent
images, clipping, and custom mesh viewports in one offscreen pass, then composites
its live output. Space changes layer MSAA and P pauses animation. The primitive
benchmark compares individual, scoped, and explicit batch APIs and checks ordered
mixed-renderer output before timing. `painter` expresses the same layer through the
facade, including borrowed transform scopes and custom mesh interleaving. Painter
benchmark variants compare against equivalent direct renderer calls and batches:

```sh
cargo bench -p astrelis --bench primitives -- --counts 100,1000,10000 --samples 40 --warmup 8
```

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

The foundation acceptance examples exercise animated clipped layer compositing and
indexed, instanced 3D with camera uniforms and depth. Both own their complete window
lifecycle. Space toggles supported MSAA and P pauses animation.

The [foundation acceptance criteria](docs/foundation.md) describe the API and
performance gate, measurement boundaries, and remaining work. Run the independent
headless benchmark with:

```sh
cargo bench -p astrelis --bench rendering -- \
  --counts 100,1000,10000 --samples 40 --warmup 8 \
  > rendering.csv 2> rendering.log
```

The benchmark compares equivalent direct-wgpu and Astrelis workloads, checks pixel
output before timing, and emits median/p95 CPU metrics. It has no window or example
smoke-test mode. Completion waits are reported separately from CPU work and are not
GPU execution measurements. See the [initial baseline](docs/performance/baseline.md)
and [scoped drawing results](docs/performance/scoped-drawing.md).

MIT. See [LICENSE-MIT](LICENSE-MIT).

`text_quality` compares whole/fractional pixel placement, small sizes, hinting,
DPI preparation, magnification, rotation, multilingual fallback, and color glyphs.
Space toggles MSAA; Z toggles 2x/4x magnification. See the
[quality captures](docs/performance/text-quality.md). The
[text workload benchmark](docs/performance/text-workloads.md) measures visible
1920×1080 text, 600 labels, mixed color pages, content updates, many unique glyphs,
and explicit recovery under a small atlas budget:

```sh
cargo bench -p astrelis --bench text_workloads
cargo bench -p astrelis --bench text_workloads -- --no-timestamps
```

GPU pass timing is optional and withheld when timestamp samples are incomplete.
Opt into scalable outline fill using the same retained draw API:

```rust
let prepared = painter.prepare_text(&layout,
    MtsdfOptions::new().pixels_per_em(64).range_em(0.25).scale_factor(dpi_scale))?;
paint.draw_text(&prepared, TextDraw::new([20., 30.]))?;
```

Coverage remains the default; intrinsic color glyphs retain their image path.
Fields reuse atlas images across font sizes and DPI, but cold generation is
substantial: prepare ahead of drawing. `PreparedText::preparation()` describes the
chosen representation and replaces `raster_options()`. See the
[distance-field API](docs/text-distance-fields.md) and
[quality/performance report](docs/performance/text-distance-fields.md). Fill is
implemented; outline/shadow effects remain separate.

```sh
cargo run -p astrelis --example text_distance_fields
cargo bench -p astrelis --bench text_distance_fields
```
