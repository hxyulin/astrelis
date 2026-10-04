# Astrelis

The `next` branch starts a new rendering API built directly on wgpu, with one
workspace crate. The existing implementation remains on `main`.

This version renders indexed triangle meshes and sampled texture rectangles into window surfaces and
offscreen framebuffers. It provides a shared `GraphicsContext`, uploaded `Mesh`
resources, a `RenderTarget` enum with surface/framebuffer variants, scoped `Frame`
and `RenderPass` types, and an independent `MeshRenderer`. Create GPU resources
with `graphics.create_mesh(...)` and `graphics.create_framebuffer(...)`; construct
the built-in renderer with `MeshRenderer::new(&graphics)`.

## API

Window creation belongs to the application. An owned window handle such as
`Arc<winit::window::Window>` allows the surface to have a `'static` lifetime:

```rust,no_run
use astrelis::{FrameError, GraphicsContext, MeshRenderer, SurfaceOptions, Vertex, wgpu};

async fn example(window: std::sync::Arc<winit::window::Window>) -> Result<(), Box<dyn std::error::Error>> {
    let size = window.inner_size();
    let (graphics, mut target) = GraphicsContext::with_surface(
        window, SurfaceOptions::new(size.width, size.height),
    ).await?;
    let mut renderer = MeshRenderer::new(&graphics);

    let triangle = graphics.create_mesh(
        &[
            Vertex::new([ 0.0,  0.7, 0.0], [1.0, 0.0, 0.0, 1.0]),
            Vertex::new([-0.7, -0.7, 0.0], [0.0, 1.0, 0.0, 1.0]),
            Vertex::new([ 0.7, -0.7, 0.0], [0.0, 0.0, 1.0, 1.0]),
        ],
        &[0, 1, 2],
    )?;

    // In the window's redraw handler; upload the mesh only once.
    let mut frame = match target.begin_frame() {
        Ok(frame) => frame,
        Err(FrameError::Retry) => { /* Schedule a later redraw. */ return Ok(()); }
        Err(FrameError::Suspended) => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    {
        let mut pass = frame.render_pass().clear_color(wgpu::Color::BLACK).begin()?;
        renderer.draw(&mut pass, &triangle)?;
    }
    frame.finish()?;
    Ok(())
}
```

Vertices use clip-space X/Y in `[-1, 1]`, Z in `[0, 1]`, and linear, straight-alpha
RGBA colors. The shader interpolates color and premultiplies it for blending.
Default shading draws in submission order without depth testing or face culling. Indices
are `u32`; a clear pass with no draws is valid.

`target.begin_frame()` returns `Result<Frame, FrameError>` independently of any
renderer. Multiple renderers on the same device can draw into its passes.
`frame.render_pass().begin()?` defaults to clearing transparent black, storing the
result, and using the full viewport and scissor. Each pass has the same defaults;
use `.load()` to preserve earlier drawing. Builders support `.label(...)`,
`.clear_color(...)`, `.load()`, `.viewport(...)`, and `.scissor_rect(...)`. The last
clear/load selection wins. Dropping a builder records nothing; invalid configuration
leaves the frame usable.

```rust
let mut pass = frame.render_pass()
    .label("scene")
    .clear_color(wgpu::Color::BLACK)
    .scissor_rect(0, 0, width, height)
    .begin()?;
renderer.draw(&mut pass, &mesh)?;
pass.set_scissor_rect(10, 10, 100, 100)?;
renderer.draw(&mut pass, &overlay)?;
```

The builder and active pass use physical pixel dimensions. Viewport methods also
take a depth range, following wgpu. Scissor rectangles must fit the attachment;
zero dimensions clip all drawing. `MeshRenderer` uses the pass's selected viewport
and scissor instead of forcing full-target drawing.

Passes end when dropped. `frame.finish()` consumes the frame, submits once, and
presents only for surface frames, returning a wgpu submission index. Dropping a frame releases its acquired
image and discards recorded commands without submitting or presenting. Presentation
without a clear pass returns `Error::UninitializedFrame`.

`FrameError::Retry` requires a later redraw after a transient acquisition
failure. `FrameError::Suspended` means the target is zero-sized or occluded. Resize
with physical dimensions using `target.resize` when no frame is active. Frame/pass
borrows prevent
resizing during recording or presenting while a pass is active. Outdated surfaces
are reconfigured once; a lost surface must be recreated by the application. The
examples show this lifecycle and sleep while idle.

`pass.as_wgpu()` supports custom drawing within the same pass. Custom renderers
must set the GPU state they rely on. The built-in mesh renderer restores its
pipeline, geometry bindings, and the pass's chosen viewport and scissor for every
draw. Raw viewport/scissor changes do not update the wrapper's settings; use the
wrapped setters to control mesh drawing.

`frame.encoder()` permits copies, compute, and custom commands before, between,
and after passes, using resources from the same device. Commands are submitted
with the frame or discarded when the frame is dropped. The frame owns submission
and encoder lifetime. A wrapped clear pass is still required to initialize the
surface for presentation. Frame borrows prevent encoder access while a pass is in use.

A context owns one wgpu instance, adapter, device, and queue. Use
`graphics.create_surface(window, SurfaceOptions::new(width, height))` for additional windows; each target
has its own size and presentation state. The same renderer and meshes can draw to
all compatible targets. An incompatible surface returns `Error::UnsupportedSurface`
without selecting a different GPU. `GraphicsContext::headless()` initializes
without a window.

`SurfaceOptions::new(width, height)` defaults to one sample per pixel. Select
initial MSAA when creating the target:

```rust
let options = SurfaceOptions::new(width, height).sample_count(4);
let (graphics, mut target) = GraphicsContext::with_surface(window, options).await?;

// For a settings change during gameplay, between frames:
let supported = target.supported_sample_counts(); // Borrowed &[u32], no allocation.
assert!(supported.contains(&4));
target.set_sample_count(1)?;
```

Creation validates the initial count and caches counts usable for the selected
surface format on the current device, including render and resolve support.
`supported_sample_counts()` borrows that cache; the runtime setter uses it without
repeating capability queries. Unsupported creation requests return
`Error::UnsupportedSampleCount` before surface configuration or attachment allocation;
unsupported runtime changes leave the target unchanged. No fallback is selected
implicitly. Zero-sized targets validate MSAA at creation and defer allocation until
resized. Native counts beyond the portable set may require
`wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` enabled on an
application-created device adopted through `GraphicsContext::from_wgpu`.

The surface image stays single-sampled. The target reuses a multisampled color
attachment, recreating it on resize or sample-count changes and releasing it when
suspended. Each pass resolves to the surface image and retains its multisampled
contents for later `.load()` passes. This preserves edge coverage across passes;
every new frame still starts with a clear. `MeshRenderer` selects pipelines by
format and sample count, so one renderer can draw to targets with different counts.
Custom pipelines must match `pass.sample_count()`. Surface recreation creates a
new target; include the desired count in its creation options as shown in the examples.
At 1x there is no multisampled attachment or resolve. Ordinary frame/pass recording
does not query capabilities or allocate MSAA attachments. Changing to a new count
may allocate an attachment, and its first mesh draw may create a pipeline.

`GraphicsContext::from_wgpu` accepts an existing instance/adapter/device/queue, and the
context and mesh expose their wgpu resources for custom GPU work. The context
requests no timestamp features. Normal frames do not wait for GPU completion;
resizing or recovering an outdated surface may synchronize with the GPU.

## Textures and compositing

Create textures through the context, upload pixels, and prepare reusable draws
through an independent `TextureRenderer`:

```rust
let texture = graphics.create_texture(TextureOptions::new(width, height))?;
texture.write(&rgba_pixels)?; // Tightly packed sRGB RGBA8; no 256-byte row padding.

let mut renderer = TextureRenderer::new(&graphics);
let image = renderer.create_binding(texture.view(), TextureDrawOptions::new()
    .destination([0.1, 0.1, 0.8, 0.8])
    .source([0.0, 0.0, 0.5, 0.5])
    .filter(TextureFilter::Nearest)
    .tint([1.0, 0.8, 0.8, 0.75]))?;
renderer.prepare_for_target(&image, &target)?;

let mut frame = target.begin_frame()?;
{
    let mut pass = frame.render_pass().begin()?;
    renderer.draw(&mut pass, &image)?;
}
frame.finish()?;
```

Rectangles are `[x, y, width, height]`. Destination coordinates are relative to
the active viewport, from top-left `(0, 0)` to bottom-right `(1, 1)`. Source
coordinates are normalized texture UVs with the same orientation. Defaults draw
the complete source into the full viewport, providing a full-target blit.
Sampling clamps to texture edges. Nearest and linear filtering, tint/opacity,
and `TextureBlend::Alpha` / `Replace` are supported. Tint is linear RGB with straight
opacity. Draw output is premultiplied in both blend modes. Texture draws restore
wrapped viewport/scissor state and disable depth/stencil tests and writes.

A `TextureBinding` retains its source, sampler, and immutable GPU parameters.
Reuse it across frames and renderers on the same device. Prepared repeated draws
create no GPU resources or uploads. Create another binding for different settings.
Pipeline variants depend on formats, MSAA, filtering, alpha mode, and blending;
rectangles, tint, and image dimensions do not create additional pipelines.

For framebuffer compositing, bind `framebuffer.color_view()` with
`TextureDrawOptions::new().alpha(TextureAlpha::Premultiplied)`. The framebuffer's
resolved color is always single-sampled. Record the offscreen pass with
`frame.render_to(&mut framebuffer)` first, then draw its binding into the surface
pass and finish once. Sampling an active color/resolve attachment is rejected as
`Error::TextureFeedback`.

Framebuffer resize and MSAA changes replace storage. Refresh the binding explicitly
with `renderer.rebind(&mut binding, framebuffer.color_view()?)?` after allocation.
The sampler and parameter buffer are reused; the same view needs no new bind group.
Previously recorded draws retain their original bindings. Uploaded images normally
use the default `TextureAlpha::Straight`; mesh-rendered transparent framebuffer
contents use `Premultiplied` to avoid applying alpha twice.

Texture creation supports uncompressed 2D color formats, one mip, and one layer.
Defaults are sRGB RGBA8 with `TEXTURE_BINDING | COPY_DST`; format and usages are
configurable. Upload bytes must use the texture's encoding. `write_region` updates
an in-bounds rectangle with tightly packed rows. Uploads execute before commands
in the next queue submission; submit older draws before changing pixels they
should observe. Texture drawing also accepts application-created sampled 2D
float-color views, including unfilterable float formats with nearest sampling.
Custom view/device mismatches follow wgpu validation. File decoding and mipmap
generation remain application-controlled.

## Materials and custom mesh shaders

The default `renderer.draw(&mut pass, &mesh)` still interpolates vertex color.
Choose custom shading with a material built from an application-created wgpu
shader module:

```rust
let shader = graphics.device().create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("Application mesh shader"),
    source: wgpu::ShaderSource::Wgsl(source.into()),
});
let material = graphics.create_material(
    MaterialOptions::new(&shader)
        .bind_group_layouts(&[Some(&uniform_layout)])
        .blend(Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING)),
);

// Optional: create this target's pipeline during loading, before the first draw.
renderer.prepare_material(&material, target.format(), target.sample_count())?;

let mut frame = target.begin_frame()?;
{
    let mut pass = frame.render_pass().begin()?;
    pass.as_wgpu().set_bind_group(0, &uniform_group, &[dynamic_offset]);
    renderer.draw_with_material(&mut pass, &mesh, &material)?;
}
frame.finish()?;
```

The vertex shader accepts position at location 0 (`vec3<f32>`) and color at
location 1 (`vec4<f32>`); unused attributes can be omitted. It can transform
positions using application-owned uniforms or storage resources. Fragment output
at location 0 must match the destination format. Default entry points are
`vertex_main` / `fragment_main`; `.entry_points(vertex, fragment)` overrides them.
Materials select blending, color write masks, front-face winding, and face culling.
Default blending requires premultiplied shader output; `.blend(None)` selects
replacement writes and allows matching integer outputs on integer framebuffers.
Depth/stencil testing is opt-in through material state and target attachments.

Materials are immutable GPU resources. Buffers, textures, and bind groups remain
application-owned; updates and per-draw dynamic offsets do not require a new
material. Rebind the required groups when switching resource bindings. The renderer
restores the material pipeline, mesh buffers, and the wrapped pass's raster settings.
One material can be used by independent renderers and compatible targets.

Each renderer caches pipelines by material identity, color format, sample count,
and optional depth/stencil format.
Cloned materials reuse entries; creating another material produces another identity.
The cache lasts for the renderer's lifetime. `prepare(format, samples)` prebuilds
the default material's pipeline, and `prepare_material(material, format, samples)`
prebuilds a custom material's pipeline. Cache hits do not create pipelines or query
capabilities. Preparation records/submits no commands and waits for no GPU work;
GPU drivers may still defer some compilation work until use.

Astrelis returns device, target-format, and sample-count compatibility errors.
Shader compilation, entry points, interfaces, and raw binding layouts use wgpu's
error scopes or uncaptured error handler. A successful preparation `Result` alone
does not certify shader validity. The crate documentation shows checking a wgpu
validation scope around preparation; after a shader failure, discard that material
and create a corrected one.

## Offscreen framebuffers

```rust
let mut framebuffer = graphics.create_framebuffer(
    FramebufferOptions::new(1024, 1024)
        .format(wgpu::TextureFormat::Rgba8Unorm)
        .sample_count(4),
)?;

let mut frame = window_target.begin_frame()?;
{
    let mut pass = frame.render_to(&mut framebuffer).begin()?;
    mesh_renderer.draw(&mut pass, &mesh)?;
}
{
    let mut pass = frame.render_pass().begin()?;
    // Application-defined drawing using framebuffer.color_view()? goes here.
}
frame.finish()?; // One submission, then surface presentation.
```

For headless rendering, start with `framebuffer.begin_frame()` and use the same
pass API and `finish()`. This submits without presentation. Framebuffers can also
be moved into `RenderTarget::Framebuffer` for generic destination handling.

Defaults are linear RGBA8, 1x samples, and `RENDER_ATTACHMENT | TEXTURE_BINDING`.
Use `.usage(...)` to add `COPY_SRC` for copying/readback. The output texture is
always single-sampled. `color_texture()` and `color_view()` return `Result` because
zero-sized framebuffers have no attachments. Zero size suspends standalone frames;
`render_to(...).begin()` returns `Error::TargetSuspended`. A foreign device is
rejected before recording commands.

Framebuffer `.load()` preserves contents across submitted recordings. An initial
load requires a submitted clear or an earlier clear in the current frame.
Dropping the recording does not initialize the framebuffer or change prior contents.
Surface frames still need a clear for their own default destination; clearing an
additional framebuffer does not satisfy this requirement.

`resize()` and `set_sample_count()` replace attachments, discard their contents,
and invalidate previous output bindings. Rebuild bind groups using the new view
and clear before loading again. Unchanged settings reuse attachments, and sample
counts are validated against the cache. Make attachment changes between recordings.
Old cloned views and pending GPU commands continue to reference old textures.

This implementation has one color attachment, optional MSAA, and optional
depth/stencil. Multiple color attachments and imported attachments remain future work. Integer
color formats can be used with custom shaders; the default floating-point mesh
material returns `Error::UnsupportedMeshFormat` for incompatible formats. A custom
material can write matching integer outputs with blending disabled.

## Depth and stencil

Configure attachments at creation; the target manages storage, size, and MSAA:

```rust
let options = SurfaceOptions::new(width, height)
    .sample_count(4)
    .depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8);
let (graphics, mut target) = GraphicsContext::with_surface(window, options).await?;
```

`FramebufferOptions` has the same depth/stencil selectors. `Depth24Plus` provides
only depth, `Stencil8` only stencil, and `Depth24PlusStencil8` both. Disabled
attachments allocate no storage. Creation validates formats/features/usages and
caches sample counts supported by all attachments. Depth/stencil has the target's
sample count without resolving; resize/MSAA changes replace storage and contents.

Materials select tests and writes independently of attachment allocation. The
renderer exposes its default shader so ordinary colored geometry needs no new WGSL:

```rust
let material = graphics.create_material(
    MaterialOptions::new(renderer.default_material().shader())
        .depth_stencil(Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        })),
);
renderer.prepare_material_for_target(&material, &target)?;
```

Default material drawing disables depth/stencil testing and writes, even on targets
with these attachments, so overlays can share a pass. Explicit material formats
must match the pass. `prepare_for_target` and `prepare_material_for_target` prepare
against all target formats. Scalar `prepare` assumes no depth/stencil, and scalar
`prepare_material` uses the material's explicit format.

Passes default to clearing depth to `1.0` and stencil to `0`, storing both. Color,
depth, and stencil operations are independent:

```rust
let mut pass = frame.render_pass()
    .load()          // Preserve color only.
    .load_depth()
    .load_stencil()
    .stencil_reference(1)
    .begin()?;
pass.set_stencil_reference(2); // Applies to subsequent draws; creates no pipeline.
```

Use `clear_depth` / `clear_stencil` to select clear values. Raw `depth_ops` and
`stencil_ops` accept load/store operations, or `None` for a read-only aspect.
Materials cannot write read-only aspects. Loads/read-only access need a stored
clear from a submitted recording or an earlier pass in this frame. `Discard`
invalidates that aspect until a later clear/store. Dropped or rejected frames do
not commit initialization changes; resize/MSAA creates fresh state. Record and
submit work sharing an attachment in order; tracking does not coordinate
independent simultaneous recordings of the same attachment.

Stencil stores an 8-bit mask per sample. A mask draw can disable color writes,
replace stencil with a reference, and then restrict content using `Equal` against
that reference. Read/write masks allow separate bits for independent flags. Stencil
is useful for nonrectangular UI clipping, object outlines, and portal masks.
Its values are updated through pipeline stencil operations; it is not a general
shader-writable buffer. The standalone stencil example demonstrates diamond clipping.

Raw depth/stencil texture/view getters expose storage for custom GPU work.
Creation defaults to rendering usages; opt into supported sampling/copy usages
with `depth_stencil_usage`. Combined formats need aspect-compatible sampling views,
and multisampled depth is not resolved. Getter results distinguish disabled
attachments (`None`) from enabled suspended attachments (`TargetSuspended`).
Raw writes do not update managed initialization, and resize/MSAA invalidates old bindings.

## Examples

```sh
cargo run -p astrelis --example triangle
cargo run -p astrelis --example meshes
cargo run -p astrelis --example materials
cargo run -p astrelis --example depth
cargo run -p astrelis --example stencil
cargo run -p astrelis --example msaa
cargo run -p astrelis --example framebuffer
cargo run -p astrelis --example textures
cargo run -p astrelis --example compositing
cargo run -p astrelis --example multi_window
```

The triangle demonstrates interpolated vertex color. The meshes example draws
overlapping opaque and translucent quads through two independent renderers sharing
one pass, each mesh using a vertex and index buffer.
The materials example compares default shading with a custom mesh shader; press
Space to change an application-owned tint uniform without rebuilding the material
or pipeline. Both draws share a pass.
The depth example shows nearer geometry occluding later draws; Space toggles
depth testing. The stencil example clips a colored quad to a diamond; Space
toggles clipping. Both use 4x MSAA and target-owned attachments.
The MSAA example creates its target with 4x MSAA; press Space to cycle usable counts.
The framebuffer example renders a 4x MSAA triangle offscreen, then samples its
resolved texture into the window through an application-defined shader that swaps
red and blue. Both passes share one submission. Its cached bind group is rebuilt
when resizing changes the output view.
The multi-window example shares one context and its rendering resources between
two independently sized windows using 1x and 4x samples.
The textures example uploads an image and compares nearest/linear sampling with
a cropped, tinted translucent overlay. The compositing example draws a cropped
MSAA framebuffer into a clipped region of a depth-enabled surface; Space changes
offscreen MSAA and refreshes the prepared source binding.

Each example is a standalone file with its own window creation, application state,
event handling, redraw scheduling, resize handling, and surface-loss recovery.
Copy one into a binary that depends on `astrelis`, `winit = "0.30"`, and
`pollster = "0.4"`; no shared support module or extra source files are needed.

The examples contain application code only. For automated native checks, copy an
example to a temporary binary and add presentation counters, resize checks, and
an exit deadline there. winit and pollster are development dependencies, not
library dependencies.

## Development

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The GPU test reads pixels back to verify multiple renderers sharing a pass, indexed
drawing, draw order, alpha blending, sequential load/clear passes, restored GPU
state, default builder behavior, initial and dynamic viewport/scissor settings,
configuration validation, device validation, and resource reuse. MSAA pixel tests
verify partial edge coverage, resolves, and equivalent blending across one pass
and sequential load passes. Compile-fail documentation tests
verify the frame/pass lifetime constraints. The GPU test fails when no adapter is
available. Public framebuffer tests verify persisted contents, abandoned recordings,
attachment replacement, suspended destinations, validation, and resolving/sampling
across multiple targets in one submission.

Material GPU tests verify custom vertex/fragment entry points, dynamic uniform
offsets, updates without pipeline recreation, default/custom switching, MSAA,
cloned material reuse, independent renderers, culling, color masks, integer output,
device rejection, and wgpu validation diagnostics. Depth/stencil pixel tests verify
occlusion, reversed-Z, persistent loads, MSAA, read-only rejection, independent
stencil flags, dynamic references, discard/abandonment, and attachment replacement.
Texture pixel tests cover region uploads, byte-count validation, source orientation,
crop/placement, filtering, sRGB decoding, alpha modes, tint, replacement, bind-group
snapshots, framebuffer resize/MSAA rebinding, feedback rejection, viewport/scissor,
prepared pipeline reuse, and resources from separate instances/devices.

Wrapped passes currently provide one color attachment. Custom renderers can use
raw pass access for instancing or indirect draws, and encoder access for compute,
copies, or more elaborate passes on application-owned textures. Acquired surface
attachments remain private, and raw writes do not update wrapped initialization
tracking. Multiple color attachments, colorless depth-only passes, imported attachments,
custom vertex layouts, and
externally batched frame submission are future extensions. Complex passes can
render offscreen and composite into the surface today.

Canvas recording, cameras, scene APIs, and UI integration will be designed in
later slices.

## License

MIT. See [LICENSE-MIT](LICENSE-MIT).
