# Astrelis for the web

Astrelis renders to HTML canvases through wgpu's **WebGPU** backend on
`wasm32-unknown-unknown`. The core rendering API is shared with desktop: meshes,
Painter, paths, textures, text, offscreen framebuffers and MSAA. The browser owns
the canvas, input and scheduling. There is no WebGL fallback in the default build.

The [web example](../examples/web) uses a small Rust `cdylib` and plain JavaScript.
It draws shapes, a curved vector path, an uploaded image and embedded-font text;
pointer movement updates a marker, and a button switches between 1× and 4× MSAA.
It requires no frontend bundler or JavaScript framework.

## Run the example

From the repository root, with the repository's Rust toolchain and Python 3:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
sh examples/web/build.sh
cd examples/web
python3 -m http.server 8080 --bind 127.0.0.1
```

Open <http://127.0.0.1:8080> in a browser with WebGPU enabled and a usable GPU
adapter. WebGPU needs a secure context: localhost works for development; deployed
pages need HTTPS. `navigator.gpu` alone does not guarantee that adapter/device
creation will succeed. The example reports initialization and GPU failures on
the page.

The build script prefers `mbx` when installed, otherwise Cargo. It produces
`examples/web/pkg/` with the wasm binary and JavaScript bindings, and copies the
embedded font's license alongside the page. The CLI's version must match the
`wasm-bindgen` version in `Cargo.lock`; update the CLI when updating that dependency.
Serve `.wasm` files as `application/wasm`. Opening `index.html` through `file://`
does not work. Deploy `index.html`, `main.js`, `pkg/` and `SourceSans3-OFL.md`
together; generated outputs are ignored by Git.

## Initialize asynchronously

Depend on `astrelis`, `wasm-bindgen`, `wasm-bindgen-futures`, and `web-sys` with
its `HtmlCanvasElement` feature. Build your library as a `cdylib`. See the
example's [manifest](../examples/web/Cargo.toml) for exact workspace dependencies.

```rust,ignore
use astrelis::{GraphicsContext, Painter, SurfaceOptions, wgpu};

// In an async wasm-bindgen export, called from JavaScript with an HTML canvas:
let (graphics, mut target) = GraphicsContext::with_surface(
    wgpu::SurfaceTarget::Canvas(canvas.clone()),
    SurfaceOptions::new(canvas.width(), canvas.height()),
).await?;
let mut painter = Painter::new(&graphics);
painter.prepare_for_target(&target)?;
```

Retain the context, target and renderers between frames. Adapter/device requests
must yield to the browser. Never use `pollster::block_on`, a blocking channel
receive, or a GPU completion wait on the browser thread. Async wasm exports return
JavaScript promises. After generating bindings with `wasm-bindgen --target web`,
await the module initializer before calling an export:

```js
import init, { WebApp } from './pkg/astrelis_web_example.js';
await init();
const app = await WebApp.create(document.querySelector('canvas'));
requestAnimationFrame(() => app.draw(devicePixelRatio, 100, 100));
```

Use the normal `begin_frame` → render pass → `frame.finish()` sequence inside
the callback. Submission does not block for GPU completion. The example uses
on-demand animation frames, invalidated by input, resize and density changes.
For animation, request the next frame after each successful draw. Pause drawing
when the document is hidden.

## Canvas dimensions and input

CSS controls the displayed size; `canvas.width` and `canvas.height` control the
physical backing resolution. Multiply the canvas's CSS size by
`devicePixelRatio`, round to integer pixels, and call `target.resize(width,
height)` whenever those dimensions change. Use `ResizeObserver` for layout
changes and also observe density changes when moving between displays or zooming.
Do not derive CSS size from the backing dimensions: this can create resize loops.

Draw in CSS pixels by applying `Transform2D::scale(dpr, dpr)` once to the
Painter session. Prepare text with `TextRasterOptions::raster_scale(dpr)` and
prepare it again when density changes. Pointer positions are CSS coordinates
relative to `canvas.getBoundingClientRect()`; keep them in the same logical units
as your drawing. The example avoids CSS borders/padding/transforms on the canvas.

Zero-sized targets suspend acquisition. Retry `FrameError::Retry` on a later
animation frame; wait for a resize when suspended. Recreate a lost surface with
`graphics.create_surface(SurfaceTarget::Canvas(...), options)`, preserving the
sample count and preparing pipelines again if attachment formats change. Device
loss requires a new graphics context and rebuilding all GPU resources. The
example stops with a reload message for device loss, and drains
`take_gpu_events()` periodically so asynchronous validation failures are visible.

## Text, color and MSAA

Browsers do not expose installed fonts to `TextSystem::load_system_fonts()`.
Embed licensed TTF/OTF bytes using `include_bytes!`, or fetch bytes in JavaScript
and pass them to Rust, then call `TextSystem::load_font`. CSS font loading does
not populate Astrelis's font database. The example embeds Source Sans 3; its OFL
license is included in the served output.

Built-in draw colors are linear RGB. Surface targets select an sRGB render view
even when WebGPU canvas storage offers only a linear format. Multisample storage
and resolve storage use matching formats with matching sRGB views, preserving
display colors when toggling MSAA. Select a supported count from
`target.supported_sample_counts()` and call `painter.prepare_for_target()` after
changing it. Image pipelines also need `painter.prepare_image()`.

## Integration boundaries

- `astrelis-winit::WindowContext` and configuration helpers compile for wasm.
  Its `Runner`, `Handler` and desktop scheduling API remain desktop-only. For a
  winit browser loop, use its web extensions (`spawn_app`, canvas attachment),
  asynchronously initialize a `WindowContext`, and deliver completion back to
  your application. Desktop examples that use `pollster` are not browser entry
  points. The supplied web example uses the DOM directly and needs no winit.
- `Framebuffer::read_rgba8()` is synchronous and requires a native backend that
  can wait. For browser readback, use raw wgpu copy/map operations and await
  completion while yielding to the browser.
- Optional device features and limits remain application choices. A successful
  wasm build does not establish support on every browser/device. This example
  and CI test WebGPU in Chromium; Firefox, Safari, mobile browsers, worker
  canvases and WebGL are not covered by this browser suite.

## Browser tests

After building the wasm example, Node.js 22+ can run the regression suite:

```sh
cd examples/web
npm ci
npx playwright install chromium
npm test
```

On Linux, use `npx playwright install --with-deps chromium` to install browser
system dependencies too. The suite starts its own local server and uses
Chromium's software WebGPU adapter. It checks rendered pixels (including sRGB
color before and after MSAA), embedded text, images and paths, pointer updates,
resize, device pixel ratios 1 and 2, zero-size suspension/resume, asynchronous
GPU diagnostics, and the visible error when WebGPU is unavailable. CI runs the
same suite as well as compiling both libraries and the example for wasm.

Upstream references: [wgpu web platform notes](https://wgpu.rs/doc/wgpu/documentation/platforms/web/index.html)
and [winit web integration](https://docs.rs/winit/0.30.13/winit/platform/web/index.html).

For the tested RXUI integration path and remaining platform gaps, see the
[RXUI browser validation report](web-rxui-validation.md).
