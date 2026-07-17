# Astrelis

A low-level native application and graphics runtime for Rust.

Astrelis is being rebuilt methodically from a small foundation. The current
workspace contains:

- `astrelis-app`: shared timers, wakeups, invalidation, and frame scheduling;
- `astrelis-core`: shared math, color, geometry, IDs, and logging helpers;
- `astrelis-gpu`: backend-neutral GPU resources, commands, and surfaces;
- `astrelis-gpu-wgpu`: native and browser-WebGPU implementation plus the
  native GPU profiling bridge;
- `astrelis-platform`: backend-neutral windows, lifecycle, and input;
- `astrelis-platform-winit`: desktop and browser-canvas winit implementation;
- `astrelis-platform-test`: deterministic display-free scripted backend;
- `astrelis-profiling`: dependency-free CPU/GPU timeline profiling.
- `astrelis-render`: shared scene target, antialiasing, and frame statistics.
- `astrelis-render-2d`: batched sprites, atlases, cameras, and chunked tilemaps.
- `astrelis-render-3d`: reverse-Z lit meshes, materials, culling, and debug geometry.
- `astrelis-ui-core`: extensible retained widgets, Taffy layout, typed messages,
  capture/target/bubble input, semantics, controls, and display-list generation.
- `astrelis-ui-widgets`: reusable drag/drop, split, navigation, virtualization,
  and texture-backed render-view compositions.
- `astrelis-ui-docking`: serializable editor docking trees, retained panel
  hosts, tab/split drop policy, and in-window floating groups.

Painting, text, and the first retained UI vertical slice are available as
separately testable layers above the shared application runtime. The Painter
supports solid and gradient brushes, vector paths and UI shapes, nested
opacity, images, text, transforms, and clipping.

## Development

```sh
cargo fmt --all --check
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Profiling can be compiled without instrumentation:

```sh
cargo check -p astrelis-profiling --no-default-features
```

The retained settings UI also has a WebGPU/WASM build. See
[`crates/astrelis-ui-core/README.md`](crates/astrelis-ui-core/README.md) for
the no-bundler `wasm-bindgen` workflow and current browser limitations.

## License

Licensed under the MIT license. See [LICENSE-MIT](LICENSE-MIT) for details.
