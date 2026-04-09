# Astrelis

A modular Rust game engine built on wgpu.

> **Status:** Early development (v0.3.0) — architecture rewrite in progress.

## Design Principles

- **Layered architecture** — each crate lives at a specific layer and may
  only depend on crates in lower layers. No circular dependencies.
- **Backend agnosticism** — windowing and rendering are abstracted behind
  trait crates with separate implementation crates (e.g., `astrelis-window`
  defines traits, `astrelis-window-winit` implements them).
- **One concern per crate** — the engine is split into small, focused crates
  rather than monolithic libraries. Feature flags are used for optional
  dependencies, not as architectural boundaries.
- **Zero-cost profiling** — profiling macros compile to no-ops when no
  backend is enabled.

## Architecture

```
Layer 4: astrelis                 facade crate, re-exports
Layer 3: astrelis-app             game framework, lifecycle, plugins
         astrelis-ui-*            UI system (layout, render, events)
         astrelis-scene           scene graph
Layer 2: astrelis-gpu             GPU abstraction traits
         astrelis-gpu-wgpu        wgpu backend
         astrelis-window          windowing traits
         astrelis-window-winit    winit backend
         astrelis-render-2d       high-level 2D renderers
         astrelis-assets          asset loading, hot-reload
         astrelis-input           input mapping
         astrelis-text-shaping    text shaping (CPU)
         astrelis-text-render     glyph atlas (GPU)
         astrelis-ecs             entity-component-system
Layer 1: astrelis-profiling       backend-agnostic profiling
Layer 0: astrelis-core            math, types, traits
```

## Crates

| Crate | Layer | Status | Description |
|-------|-------|--------|-------------|
| `astrelis-core` | 0 | Implemented | Math (glam), color, geometry, typed IDs |
| `astrelis-profiling` | 1 | Implemented | CPU/GPU profiling macros (puffin backend) |
| `astrelis-window` | 2 | Planned | Windowing abstraction traits |
| `astrelis-window-winit` | 2 | Planned | winit backend |
| `astrelis-gpu` | 2 | Planned | GPU abstraction traits |
| `astrelis-gpu-wgpu` | 2 | Planned | wgpu backend |
| `astrelis-render-2d` | 2 | Planned | 2D renderers (quad, sprite, line) |
| `astrelis-assets` | 2 | Planned | Async asset loading and caching |
| `astrelis-input` | 2 | Planned | Backend-agnostic input mapping |
| `astrelis-text-shaping` | 2 | Planned | Text shaping via cosmic-text |
| `astrelis-text-render` | 2 | Planned | GPU glyph atlas and text rendering |
| `astrelis-ecs` | 2 | Planned | Entity-component-system |
| `astrelis-app` | 3 | Planned | Application framework and plugin system |
| `astrelis-ui-*` | 3 | Planned | Retained-mode UI system |
| `astrelis-scene` | 3 | Planned | Scene graph and transforms |
| `astrelis` | 4 | Planned | Facade crate |

## Building

```sh
cargo build
cargo test
cargo clippy --workspace
```

Enable profiling with the puffin backend:

```sh
cargo build --features astrelis-profiling/puffin
```

## License

Licensed under the MIT license. See [LICENSE-MIT](LICENSE-MIT) for details.
