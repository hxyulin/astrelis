# Astrelis

The `next` branch starts a new rendering API built directly on wgpu, with one
workspace crate. The existing implementation remains on `main`.

This first slice renders indexed, colored triangle meshes into window surfaces.
It provides a shared `GraphicsContext`, uploaded `Mesh` resources, a surface-only
`RenderTarget` enum, and a `Renderer` that submits and presents frames.

## API

Window creation belongs to the application. An owned window handle such as
`Arc<winit::window::Window>` allows the surface to have a `'static` lifetime:

```rust,no_run
use astrelis::{GraphicsContext, Mesh, Renderer, Vertex, wgpu};

async fn example(window: std::sync::Arc<winit::window::Window>) -> Result<(), astrelis::Error> {
    let size = window.inner_size();
    let (graphics, mut target) = GraphicsContext::with_surface(
        window, size.width, size.height,
    ).await?;
    let mut renderer = Renderer::new(&graphics);

    let triangle = Mesh::new(
        &graphics,
        &[
            Vertex::new([ 0.0,  0.7, 0.0], [1.0, 0.0, 0.0, 1.0]),
            Vertex::new([-0.7, -0.7, 0.0], [0.0, 1.0, 0.0, 1.0]),
            Vertex::new([ 0.7, -0.7, 0.0], [0.0, 0.0, 1.0, 1.0]),
        ],
        &[0, 1, 2],
    )?;

    // In the window's redraw handler; upload the mesh only once.
    let _status = renderer.render(&mut target, wgpu::Color::BLACK, &[&triangle])?;
    Ok(())
}
```

Vertices use clip-space X/Y in `[-1, 1]`, Z in `[0, 1]`, and linear, straight-alpha
RGBA colors. The shader interpolates color and premultiplies it for blending.
Meshes draw in submission order without depth testing or face culling. Indices
are `u32`; an empty mesh submission clears the surface.

`FrameStatus::Presented` includes the wgpu submission index. `Retry` requests a
later redraw after a transient acquisition failure. `Suspended` means the target
is zero-sized or occluded. Resize with physical dimensions using `target.resize`.
Outdated surfaces are reconfigured once; a lost surface must be recreated by the
application. The examples show this lifecycle and sleep while idle.

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
overlapping opaque and translucent quads, each using a vertex and index buffer.
The multi-window example shares one context, renderer, and mesh between two windows.
All native examples support `--smoke` to verify zero-size suspension, render at
least three frames, confirm a resize to 640×480, and exit. winit and pollster are
development dependencies, not library dependencies.

## Development

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p astrelis --example triangle -- --smoke
cargo run -p astrelis --example meshes -- --smoke
cargo run -p astrelis --example multi_window -- --smoke
```

The GPU test reads pixels back to verify indexed drawing, submission order,
alpha blending, clear-only rendering, and resource reuse. It fails when no GPU
adapter is available. It uses an internal offscreen attachment; the public target
API remains surface-only.

This version deliberately starts with a fixed mesh pipeline. Canvas recording,
materials, cameras, scene APIs, and UI integration will be designed in later slices.

## License

MIT. See [LICENSE-MIT](LICENSE-MIT).
