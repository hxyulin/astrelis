# Design: Application Framework, 2D Renderer, and Facade Crate

**Date:** 2026-04-13
**Status:** Draft
**Scope:** `astrelis-app` (Layer 3), `astrelis-render-2d` (Layer 2), `astrelis` facade (Layer 4)

## Context

Astrelis has solid Layer 0–2 infrastructure (GPU wrappers, windowing, input, assets, text, profiling) but no higher-level framework to tie them together. Every example manually wires up `AppHandler`, creates a `Gpu`, manages a `Surface`, and calls subsystem update methods — significant boilerplate that grows with each new system.

These three crates address that gap:

1. **`astrelis-app`** — provides the plugin system, resource container, phase-based scheduling, and event bus that all future systems register into.
2. **`astrelis-render-2d`** — provides structured 2D drawing (sprites, shapes, camera, batching) so users don't need raw wgpu for common 2D games.
3. **`astrelis`** — the facade re-exporting everything, so users write `use astrelis::prelude::*`.

Together they bring Astrelis to the point where someone can write a simple 2D game without touching raw wgpu.

---

## 1. `astrelis-app` — Application Framework

### 1.1 Core Types

#### `App`

The builder and runner. Users configure plugins, resources, and systems, then call `run()` to enter the event loop.

```rust
App::new()
    .add_default_plugins()
    .add_plugin(MyGamePlugin)
    .run();
```

- `add_plugin(impl Plugin)` — registers a plugin
- `add_default_plugins()` — registers all built-in plugins (Window, Gpu, Input, Asset, Time, Profiling)
- `insert_resource(T)` — inserts a value into the type-map
- `add_system(Phase, impl Fn(&Resources))` — registers a system in a phase
- `add_event::<T>()` — registers a typed event channel
- `run()` — enters the event loop, never returns

#### `Resources`

A type-map keyed by `TypeId`. Holds all shared state.

```rust
fn my_system(resources: &Resources) {
    let input = resources.get::<InputState>();       // panics if missing
    let mut player = resources.get_mut::<PlayerState>(); // mutable borrow
    let maybe = resources.try_get::<OptionalThing>(); // returns Option
}
```

- Storage: `HashMap<TypeId, Box<dyn Any>>` (or similar)
- Borrow checking: runtime via `RefCell`-style guards (`Ref<T>`, `RefMut<T>`). Panics on conflicting borrows (two `get_mut` on the same type). This is the standard approach for single-threaded ECS-like resource containers.
- `get::<T>() -> Ref<T>` — immutable borrow, panics if missing
- `get_mut::<T>() -> RefMut<T>` — mutable borrow, panics if missing
- `try_get::<T>() -> Option<Ref<T>>` — non-panicking variant
- `try_get_mut::<T>() -> Option<RefMut<T>>` — non-panicking variant
- `contains::<T>() -> bool` — check existence

#### `Phase`

An enum defining the fixed execution order per frame:

```
Startup          (runs once after all plugins registered)
    ↓
┌─ PreUpdate     (input polling, asset server update, event buffer swap)
│  FixedUpdate   (runs 0..N times per frame at fixed rate, accumulator-based)
│  Update        (main game logic, variable dt)
│  PostUpdate    (cleanup, state transitions)
│  Render        (draw commands, pass encoding)
│  Present       (surface present, profiler frame mark)
└─ loop back to PreUpdate
```

`FixedUpdate` uses an accumulator: while `accumulator >= fixed_dt`, run all FixedUpdate systems and subtract `fixed_dt`. Default fixed rate: 60 Hz, configurable via `Time` resource.

#### `Plugin` Trait

```rust
pub trait Plugin {
    fn build(&self, app: &mut App);
}
```

Plugins can insert resources, add systems, add events, and add sub-plugins. They run their `build()` during `App` construction, before `run()` enters the event loop.

#### `Events<T>`

A double-buffered typed event queue, stored as a resource.

- `send(event: T)` — push to the current-frame buffer
- `read() -> impl Iterator<Item = &T>` — iterate events from current + previous frame
- At the start of `PreUpdate`, the framework swaps buffers: last frame's current becomes previous (still readable), write buffer is cleared

Double-buffering ensures events are readable for up to 2 frames, avoiding system-ordering-dependent event visibility.

### 1.2 Built-in Plugins

| Plugin | Resources Inserted | Phase Work |
|--------|--------------------|------------|
| `WindowPlugin` | `Window` (primary window handle) | Handles lifecycle events, feeds resize/close |
| `GpuPlugin` | `Gpu`, `Surface` | Initializes GPU, acquires/presents frames in `Present` phase |
| `InputPlugin` | `InputState` | Calls `begin_frame()` in `PreUpdate`, feeds events |
| `AssetPlugin` | `AssetServer` | Calls `update()` in `PreUpdate`, drains asset events |
| `TimePlugin` | `Time` | Updates delta time, elapsed, manages fixed-timestep accumulator |
| `ProfilingPlugin` | — | Calls `new_frame()` at frame boundaries |

Users can opt out of any default plugin and provide their own replacement.

### 1.3 `Time` Resource

```rust
pub struct Time {
    delta: Duration,         // time since last frame
    elapsed: Duration,       // total time since app start
    fixed_delta: Duration,   // fixed timestep interval (default 1/60s)
    frame_count: u64,        // total frames rendered
}
```

- `delta_secs() -> f32` — variable delta as seconds
- `fixed_delta_secs() -> f32` — fixed delta as seconds
- `elapsed_secs() -> f64` — total elapsed as seconds (f64 for precision)

### 1.4 Event Loop Integration

`App::run()` internally implements `AppHandler` from `astrelis-window`. The mapping:

- `on_lifecycle(Resumed)` → run `Startup` systems (once)
- `on_window_event(event)` → feed to `InputState`, `EguiIntegration` if present, buffer window events into `Events<WindowEvent>`
- `on_events_cleared()` → run phases `PreUpdate` → `FixedUpdate` (0..N) → `Update` → `PostUpdate` → `Render` → `Present`

Control flow defaults to `Poll` (continuous rendering). Users can switch to `Wait` via a resource or system call for desktop-app-style idle rendering.

---

## 2. `astrelis-render-2d` — 2D Rendering Pipeline

### 2.1 Overview

A Layer 2 crate depending only on `astrelis-gpu` and `astrelis-core`. Provides an immediate-mode 2D drawing API with automatic batching. App framework integration comes via a `Render2DPlugin` (lives in `astrelis-app` or a thin glue module).

### 2.2 Camera

```rust
pub struct Camera2D {
    pub position: Vec2,
    pub zoom: f32,
    pub rotation: f32,
    pub viewport: Size<Physical>,
}
```

Produces an orthographic view-projection matrix. All draw commands are in world space; the camera transform is applied in the vertex shader.

### 2.3 Drawing API

Begin/end pattern — accumulates draw commands, then sorts, batches, and submits:

```rust
renderer.begin(&camera);

// Sprites
renderer.draw_sprite(&texture, transform, sprite_opts);
renderer.draw_sprite_region(&texture, region, transform, sprite_opts); // atlas sub-rect

// Shapes
renderer.draw_rect(position, size, color);
renderer.draw_rect_filled(position, size, color);
renderer.draw_circle(center, radius, color);
renderer.draw_circle_filled(center, radius, segments, color);
renderer.draw_line(start, end, thickness, color);

// Z-index control
renderer.set_z_index(5);
renderer.draw_sprite(...); // draws at z=5

renderer.end(&gpu, &mut encoder, &view); // sort → batch → submit
```

### 2.4 Instance Format

Sprites and shapes share a unified instance format (inspired by the legacy `UnifiedInstance2D`), designed to be tier-agnostic for future indirect/bindless upgrades:

```rust
#[repr(C)]
struct Instance2D {
    position: [f32; 2],     // world-space position
    size: [f32; 2],         // quad size
    uv_min: [f32; 2],       // texture coords min (0,0 for shapes)
    uv_max: [f32; 2],       // texture coords max (0,0 for shapes)
    color: [f32; 4],        // tint / shape color (premultiplied alpha)
    rotation: f32,          // radians
    z_depth: f32,           // normalized depth for sorting
    texture_index: u32,     // index into bound textures (0 = white pixel for shapes)
    draw_type: u32,         // 0=sprite, 1=rect, 2=circle, 3=line
}
```

A single shader handles all draw types via the `draw_type` discriminant. Circles use SDF in the fragment shader (smooth at any resolution). Lines are expanded to quads on the CPU.

### 2.5 Batch Pipeline

When `end()` is called:

1. **Sort** by `(z_index, draw_type, texture_id)` — z for correctness, type to avoid pipeline switches, texture for batch merging.
2. **Batch** consecutive instances sharing the same texture into a single draw call. Write instance data into a dynamic vertex/instance buffer.
3. **Render** — one render pass, iterate batches: bind texture, draw instanced.

### 2.6 Tiered Dispatch (Future)

V1 ships as **Tier 1 (Direct)** only — per-texture-group `draw_indexed()` calls. The `Instance2D` format and `DrawBatch2D` structure are designed so that Tier 2 (indirect) and Tier 3 (bindless) can be added later as alternative dispatch backends behind a `BatchRenderer2D` trait, matching the legacy pattern:

```rust
pub trait BatchRenderer2D: Send {
    fn tier(&self) -> RenderTier;
    fn prepare(&mut self, batch: &DrawBatch2D);
    fn render(&self, pass: &mut RenderPass<'_>);
    fn stats(&self) -> BatchRenderStats2D;
}
```

V1's direct renderer implements this trait. Adding tiers later is a backend swap, not a rewrite.

### 2.7 Textures

The renderer does not own texture loading. It accepts `TextureView` handles from `astrelis-gpu`. A sprite is a texture view + optional UV region (for atlases). The asset system handles loading.

A 1x1 white pixel texture is created at init for shape rendering (shapes sample this with their vertex color).

### 2.8 Out of Scope (v1)

- Texture atlas packing (users provide regions, or a future utility crate automates it)
- Render-to-texture / offscreen targets
- Post-processing
- Particle systems
- Tilemaps

---

## 3. `astrelis` — Facade Crate

### 3.1 Purpose

Layer 4 convenience crate. Contains zero logic — only re-exports.

### 3.2 Structure

```rust
// Namespaced re-exports for specific access
pub use astrelis_core as core;
pub use astrelis_app as app;
pub use astrelis_gpu as gpu;
pub use astrelis_window as window;
pub use astrelis_input as input;
pub use astrelis_assets as assets;
pub use astrelis_render_2d as render_2d;
pub use astrelis_text as text;
pub use astrelis_profiling as profiling;

// Curated prelude for the 80% case
pub mod prelude {
    pub use astrelis_app::{App, Plugin, Phase, Resources, Events, Time};
    pub use astrelis_core::math::*;
    pub use astrelis_core::color::Color;
    pub use astrelis_render_2d::{Renderer2D, Camera2D, SpriteOptions};
    pub use astrelis_input::InputState;
    pub use astrelis_assets::{AssetServer, Handle, Asset, AssetLoader};
    pub use astrelis_window::keyboard::KeyCode;
    pub use astrelis_window::mouse::MouseButton;
}
```

### 3.3 Principles

- **Namespaced** (`astrelis::gpu::Buffer`) for specific types
- **Flat prelude** for common types only — not exhaustive
- **No new code** — pure re-exports
- The prelude includes only types that appear in typical game code. Specialized types (pipeline descriptors, texture formats) stay in their namespaced modules.

---

## 4. File Plan

### New Crates

```
crates/astrelis-app/
├── Cargo.toml
└── src/
    ├── lib.rs          # pub mod + re-exports
    ├── app.rs          # App builder and runner
    ├── plugin.rs       # Plugin trait
    ├── resources.rs    # Type-map container with runtime borrow checking
    ├── phase.rs        # Phase enum and system registry
    ├── events.rs       # Events<T> double-buffered event queue
    ├── time.rs         # Time resource and fixed-timestep accumulator
    └── plugins/
        ├── mod.rs
        ├── window.rs   # WindowPlugin
        ├── gpu.rs      # GpuPlugin
        ├── input.rs    # InputPlugin
        ├── asset.rs    # AssetPlugin
        ├── time.rs     # TimePlugin
        └── profiling.rs # ProfilingPlugin

crates/astrelis-render-2d/
├── Cargo.toml
└── src/
    ├── lib.rs          # pub mod + re-exports
    ├── renderer.rs     # Renderer2D begin/end API
    ├── camera.rs       # Camera2D + projection math
    ├── batch.rs        # Sort + batch logic, DrawBatch2D
    ├── instance.rs     # Instance2D format
    ├── pipeline.rs     # Render pipeline creation, shaders
    ├── shapes.rs       # Shape primitive → instance conversion
    ├── sprite.rs       # SpriteOptions, region handling
    └── shader.wgsl     # Unified 2D shader (vertex + fragment)

crates/astrelis/
├── Cargo.toml
└── src/
    └── lib.rs          # Re-exports + prelude
```

### Modified Files

- **Root `Cargo.toml`** — add three new workspace members and their dependencies to `[workspace.dependencies]`

---

## 5. Verification

### `astrelis-app`

- Create an example that uses `App::new().add_default_plugins().add_plugin(MyPlugin).run()` and verify:
  - Window opens, GPU initializes, surface presents frames
  - `InputState` is accessible and responds to keyboard/mouse
  - `Time` resource reports correct delta/elapsed
  - Systems run in phase order (log from each phase to verify)
  - `Events<T>` can send/read across systems within the same frame
  - FixedUpdate runs at the correct rate (log tick count vs wall time)
- Run `cargo clippy` and `cargo test` across the workspace

### `astrelis-render-2d`

- Create an example that draws sprites and shapes with a movable camera:
  - Verify sprites render with correct texture, position, scale, rotation
  - Verify shapes render (filled rect, filled circle, line)
  - Verify z-ordering is correct (overlapping sprites at different z)
  - Verify camera pan/zoom works
  - Check draw call count via stats (batching working = fewer calls than sprites)

### `astrelis` facade

- Create a minimal example using only `use astrelis::prelude::*` that draws a moving sprite
- Verify it compiles and runs without importing any sub-crate directly
