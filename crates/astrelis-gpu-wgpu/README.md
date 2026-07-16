# astrelis-gpu-wgpu

Native wgpu implementation of `astrelis-gpu`.

Examples:

```sh
cargo run -p astrelis-gpu-wgpu --example clear_window
cargo run -p astrelis-gpu-wgpu --example multi_window_triangle
```

Headless integration tests clear and draw offscreen, read pixels back, and
exercise compute pipelines and bind groups.

## GPU profiling on macOS

Metal 4 timestamp queries currently return zero-filled results through wgpu
([wgpu issue #9414](https://github.com/gfx-rs/wgpu/issues/9414)). Astrelis marks
Metal timestamps unreliable and refuses to create `WgpuGpuProfiler` there.

Use Vulkan through MoltenVK to validate profiling:

```sh
ASTRELIS_REQUIRE_VULKAN_PROFILING=1 \
  cargo test -p astrelis-gpu-wgpu --test vulkan_timestamps -- --nocapture
```
