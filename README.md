# Astrelis

The `next` branch starts a new rendering API built directly on wgpu, with one
workspace crate. The existing implementation remains on `main`.

This first slice renders indexed, colored triangle meshes into window surfaces.
It provides a shared `GraphicsContext`, uploaded `Mesh` resources, a surface-only
`RenderTarget` enum, scoped `Frame` and `RenderPass` types, and a `MeshRenderer` that
records mesh draws. Device-bound resources are created through the context:
`graphics.create_mesh(vertices, indices)` and `MeshRenderer::new(&graphics)`.

## API

Window creation belongs to the application. An owned window handle such as
`Arc<winit::window::Window>` allows the surface to have a `'static` lifetime:

```rust,no_run
use astrelis::{FrameError, GraphicsContext, MeshRenderer, Vertex, wgpu};

async fn example(window: std::sync::Arc<winit::window::Window>) -> Result<(), Box<dyn std::error::Error>> {
    let size = window.inner_size();
    let (graphics, mut target) = GraphicsContext::with_surface(
        window, size.width, size.height,
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
    frame.present()?;
    Ok(())
}
```

Vertices use clip-space X/Y in `[-1, 1]`, Z in `[0, 1]`, and linear, straight-alpha
RGBA colors. The shader interpolates color and premultiplies it for blending.
Meshes draw in submission order without depth testing or face culling. Indices
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

Passes end when dropped. `frame.present()` consumes the frame, submits once, and
presents, returning a wgpu submission index. Dropping a frame releases its acquired
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
`graphics.create_surface(window, width, height)` for additional windows; each target
has its own size and presentation state. The same renderer and meshes can draw to
all compatible targets. An incompatible surface returns `Error::UnsupportedSurface`
without selecting a different GPU. `GraphicsContext::headless()` initializes
without a window.

`GraphicsContext::from_wgpu` accepts an existing instance/adapter/device/queue, and the
context and mesh expose their wgpu resources for custom GPU work. The context
requests no timestamp features. Normal frames do not wait for GPU completion;
resizing or recovering an outdated surface may synchronize with the GPU.

## Examples

```sh
cargo run -p astrelis --example triangle
cargo run -p astrelis --example meshes
cargo run -p astrelis --example multi_window
```

The triangle demonstrates interpolated vertex color. The meshes example draws
overlapping opaque and translucent quads through two independent renderers sharing
one pass, each mesh using a vertex and index buffer.
The multi-window example shares one context and its rendering resources between
two independently sized windows.

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
configuration validation, device validation, and resource reuse. Compile-fail documentation tests
verify the frame/pass lifetime constraints. The GPU test fails when no adapter is
available. It uses an internal offscreen attachment; the public target API remains
surface-only.

This version deliberately starts with a fixed mesh pipeline. Canvas recording,
materials, cameras, scene APIs, and UI integration will be designed in later slices.

## License

MIT. See [LICENSE-MIT](LICENSE-MIT).
