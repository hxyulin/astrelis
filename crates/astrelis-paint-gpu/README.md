# astrelis-paint-gpu

GPU renderer for `astrelis-paint` display lists.

The renderer uses only `astrelis-gpu`, performs scale-aware CPU path
tessellation, caches meshes and uploaded images per device, uses scissor and
stencil clipping, and supports 1x or 4x MSAA.

The wgpu-backed demo is:

```sh
cargo run -p astrelis-paint-gpu --example vector_demo
```
