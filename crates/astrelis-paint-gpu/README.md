# astrelis-paint-gpu

`Renderer::register_external_image` associates application-owned, same-device,
single-sampled filterable 2D texture views with paint tokens. Registration may
be replaced or removed explicitly; allocation and texture lifetime remain the
application's responsibility.

GPU renderer for `astrelis-paint` display lists.

The renderer uses only `astrelis-gpu`, performs scale-aware CPU path
tessellation, caches meshes and uploaded images per device, uses scissor and
stencil clipping, renders mask and color glyph atlases, and supports 1x or 4x
MSAA.

The wgpu-backed demo is:

```sh
cargo run -p astrelis-paint-gpu --example vector_demo
cargo run -p astrelis-paint-gpu --example text_demo
```
