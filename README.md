<h1 align="center">Astrelis</h1>
<p align="center">A rendering API for meshes, images, vector paths and text, built directly on wgpu.</p>
<p align="center">
  <a href="https://github.com/hxyulin/astrelis/actions/workflows/ci.yml"><img alt="CI status" src="https://github.com/hxyulin/astrelis/actions/workflows/ci.yml/badge.svg?branch=main"></a>
  <a href="Cargo.toml"><img alt="Rust 1.98.1 or newer" src="https://img.shields.io/badge/rustc-1.98.1%2B-dea584?logo=rust"></a>
  <a href="Cargo.toml"><img alt="wgpu 30" src="https://img.shields.io/badge/wgpu-30-4b6bfb"></a>
  <a href="#license"><img alt="MIT or Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue"></a>
</p>
<p align="center">
  <a href="docs/guide.md">Guide</a> ·
  <a href="crates/astrelis/examples">Examples</a> ·
  <a href="docs/performance">Performance</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

Astrelis gives you GPU contexts, render targets, frames and passes, and a set of
independent renderers that record into them. Your application keeps its windows,
event loop, draw order and scheduling. Astrelis owns no scene graph, retained UI
tree or hidden display list. Draws go into the pass you're holding, in the order
you call them, and you can drop to raw wgpu in the same pass at any point.

```rust,ignore
let mut painter = Painter::new(&graphics);
painter.prepare(&target.render_format())?;

let mut frame = target.begin_frame()?;
{
    let mut pass = frame.render_pass().begin()?;
    let mut paint = painter.begin(&mut pass)?;
    paint.fill_rounded_rect(Rect::new(20., 20., 120., 60.), 12., [0.1, 0.3, 0.6, 1.])?;
    {
        let mut local = paint.transformed(Transform2D::translation(30., 30.))?;
        local.draw_line(LineDraw::new([0., 0.], [80., 0.], [1.; 4]).width(3.).cap(LineCap::Round))?;
    }
    paint.draw_text(&prepared_text, TextDraw::new([20., 120.]))?;
}
frame.finish()?;
```

| Area | What Astrelis provides |
| --- | --- |
| Foundation | Shared GPU context, window and headless targets, MSAA, depth/stencil, framebuffers with live sampling |
| Geometry | Meshes with custom vertex streams, instancing, application bind groups and custom WGSL materials |
| 2D drawing | Rectangles, rounded rectangles, ellipses, lines, filled and stroked vector paths, gradient brushes |
| Dense data | Polylines and markers for charts that stream and pan large point series |
| Images | Prepared and dynamic texture draws with cropping, filtering and placement |
| Text | Multilingual shaping and fallback, color glyphs, coverage or MTSDF rendering, hit testing and selection |
| Painter | One immediate drawing session over all of the renderers above, with transform and clip scopes, including anti-aliased rounded clips |
| Windows | `astrelis-winit`: a window context for your own loop, or a desktop runner with redraw scheduling |

Each feature ships with a benchmark, and the core draw paths are measured against
equivalent direct wgpu code. The [performance notes](docs/performance) record the results and how they were measured.

## Status

Astrelis is a rewrite. The workspace is at `0.4.0-dev` and nothing from it has
been released yet. Expect API changes between commits. The previous
implementation is tagged [`v0.3`](https://github.com/hxyulin/astrelis/tree/v0.3)
and kept on the `legacy-v0.3` branch.

Supported backends are Metal, Vulkan, DX12 and WebGPU. The `astrelis-winit`
runner targets macOS, Windows and Linux; browser and mobile event loops are not
implemented yet.

## Getting started

Astrelis requires Rust 1.98.1 or newer. Until the first release, depend on the
repository:

```toml
[dependencies]
astrelis = { git = "https://github.com/hxyulin/astrelis" }
astrelis-winit = { git = "https://github.com/hxyulin/astrelis" } # optional
```

Each example is a single file you can copy. It handles its own window, resize,
redraw scheduling and surface loss, with no shared support module:

```sh
cargo run -p astrelis-winit --example runner_triangle  # smallest windowed program
cargo run -p astrelis --example painter                # shapes, lines and transforms
cargo run -p astrelis --example painter_text           # text mixed with primitives and a custom mesh
cargo run -p astrelis --example paths                  # filled and stroked vector paths
cargo run -p astrelis --example streaming_chart        # dense point series with pan and zoom
cargo run -p astrelis --example selectable_text        # hit testing and selection
cargo run -p astrelis --example scene_3d               # instanced 3D with depth
```

[`crates/astrelis/examples`](crates/astrelis/examples) and
[`crates/astrelis-winit/examples`](crates/astrelis-winit/examples) contain the
complete set.

## Documentation

| Read | For |
| --- | --- |
| [Guide](docs/guide.md) | A tour of contexts, passes, renderers, Painter and text, with code |
| [Renderer API](docs/renderer-api.md) | Pass ownership, preparation and batching contracts |
| [Paths](docs/paths.md) and [brushes](docs/brushes.md) | Vector geometry, strokes and gradients |
| [Points](docs/points.md) | Dense chart contracts |
| [Text](docs/text.md) and [distance fields](docs/text-distance-fields.md) | Layout, rendering and scalable text |
| [Window lifecycle](docs/winit.md) | `astrelis-winit` contexts, the runner and redraw scheduling |
| [Foundation criteria](docs/foundation.md) | The API and performance bar each feature has to meet |

`cargo doc --workspace --open` builds the API reference.

## Contributing

Bug reports, examples and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md)
covers setup, the checks to run and what to include in a pull request. For a
rendering bug, include the smallest reproducing code, your GPU and backend, and
a screenshot.

## License

Astrelis is available under either the [MIT license](LICENSE-MIT) or the
[Apache License 2.0](LICENSE-APACHE), at your option.

Unless you explicitly state otherwise, any contribution you intentionally submit
for inclusion in this project, as defined in the Apache-2.0 license, is dual
licensed as above, without any additional terms or conditions.

The Source Sans 3 and Noto Sans Arabic test fonts use the SIL Open Font License
1.1; see the [font notes](crates/astrelis/tests/fonts/README.md).
