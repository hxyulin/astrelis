# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Development Commands

**Building the project:**
```bash
cargo build                           # Build all workspace crates
cargo build --release                 # Release build with optimizations
cargo build -p astrelis-ui            # Build specific crate
```

**Running examples:**
```bash
# UI examples (most commonly used)
cargo run -p astrelis-ui --example counter
cargo run -p astrelis-ui --example simple_ui
cargo run -p astrelis-ui --example ui_dashboard
cargo run -p astrelis-ui --example image_widget

# Render examples
cargo run -p astrelis-render --example image_blitting
cargo run -p astrelis-render --example sprite_sheet

# egui integration
cargo run -p astrelis-egui --example egui_demo
```

**Testing:**
```bash
cargo test                            # Run all tests
cargo test -p astrelis-ui             # Test specific crate
cargo test --workspace                # Test entire workspace
```

**Linting:**
```bash
cargo clippy                          # Run clippy lints
cargo clippy --workspace              # Lint all crates
cargo fmt                             # Format code
cargo fmt --check                     # Check formatting without writing
```

**Profiling:**
The project uses puffin for profiling. Most examples initialize profiling with:
```rust
init_profiling(ProfilingBackend::PuffinHttp);
```
Access profiler at `http://127.0.0.1:8585` when running.

## Architecture Overview

Astrelis is a modular 2D/3D Rust game engine with a layered architecture:

### Crate Dependency Layers

**Core Layer:**
- `astrelis-core`: Foundation (math via glam, logging via tracing, profiling via puffin, custom allocators)

**Platform Layer:**
- `astrelis-winit`: Window/event management, event batching, `App` trait for game loop

**Rendering Layer:**
- `astrelis-render`: WGPU-based GPU rendering
  - `GraphicsContext`: Static lifetime GPU context (device, queue, adapter) - created once via `Box::leak`
  - `WindowContext`: Per-window surface management
  - `FrameContext`: Per-frame rendering state (RAII - Drop submits commands and presents)
  - `Renderer`: Low-level utility for shader/buffer/bind group creation

**Asset Layer:**
- `astrelis-assets`: Async asset loading with generational handles
  - Type-safe `Handle<T>` with generation counter prevents use-after-free
  - SparseSet storage for O(1) access
  - Event system for hot-reload
  - Multiple sources: disk, memory, raw bytes

**Content Layer:**
- `astrelis-text`: Text rendering via cosmic-text with GPU-accelerated atlas and caching
- `astrelis-ui`: Retained-mode declarative UI with Taffy layout engine (Flexbox/Grid)

**Integration/WIP:**
- `astrelis-egui`: egui immediate-mode UI integration
- `astrelis-input`: Input state management (WIP)
- `astrelis-audio`: Audio playback (WIP)
- `astrelis-ecs`: Entity Component System (WIP)
- `astrelis-scene`: Scene management (WIP)

**Main Crate:**
- `astrelis`: Facade with plugin system, `Engine`/`EngineBuilder`, type-erased resource storage

### Key Architectural Patterns

**Static Lifetime Pattern:**
`GraphicsContext` uses `Box::leak` to achieve `'static` lifetime, simplifying API by eliminating lifetime parameters. It's created once at startup and lives for the program lifetime.

**RAII Resource Management:**
`FrameContext::Drop` automatically submits commands and presents the surface. Always ensure frame contexts go out of scope properly.

**Plugin System:**
Engine uses topologically-sorted plugins with dependencies:
```rust
let engine = Engine::builder()
    .add_plugin(MyPlugin)
    .build();
```
Resources are type-erased (HashMap<TypeId, ResourceEntry>) with type-safe generic access.

**Generational Handles:**
Asset system uses `Handle<T>` with generation counters to prevent use-after-free and provide type safety.

## UI System Architecture

The UI system is performance-critical with sophisticated optimization:

### Two-Layer Design
- `UiCore`: Render-agnostic tree management, layout, events
- `UiSystem`: Adds `UiRenderer` for GPU rendering

### Fine-Grained Dirty Flags
The system uses bitflags for selective updates:
- `COLOR_ONLY`: Color changes skip layout/text shaping (~20ms → <1ms optimization)
- `TEXT_SHAPING`: Text content changed, needs reshaping
- `LAYOUT`: Size/position changed
- `GEOMETRY`: Border/radius changed

**Key optimization:** Text shaping results are cached in `Arc<ShapedTextData>` and only invalidated when necessary via version counters.

### Declarative API with Incremental Updates
```rust
// Initial build
ui.build(|root| {
    root.text("Hello").id("greeting").build();
});

// Fast incremental update (marks TEXT_SHAPING dirty only)
ui.update_text("greeting", "New text");

// Color-only update (marks COLOR_ONLY dirty)
ui.update_color("greeting", Color::RED);
```

### Rendering Pipeline
1. **Layout:** Taffy computes Flexbox/Grid layout only for dirty subtrees
2. **Draw list generation:** Converts widgets to `QuadCommand`, `TextCommand`, `ImageCommand`
3. **GPU instanced rendering:** Single draw call per type using unit quad + instance buffers

## Rendering Pipeline Flow

**Typical frame:**
```rust
// 1. Begin frame (acquires surface texture, creates encoder)
let mut frame = renderable_window.begin_drawing();

// 2. Create render pass
{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .clear_color(Color::BLACK)
        .build(&mut frame);

    // 3. Render (UI, sprites, etc.)
    ui.render(pass.descriptor());
}

// 4. Finish frame (Drop impl submits and presents)
frame.finish();
```

**IMPORTANT:** Render passes must be dropped before `frame.finish()`. Use block scoping `{ }` to ensure pass drops first.

## Application Entry Points

**Low-level approach (using `App` trait):**
```rust
use astrelis_winit::{run_app, App, AppCtx};

fn main() {
    run_app(|ctx| {
        let graphics = GraphicsContext::new_sync();
        let window = ctx.create_window(descriptor).unwrap();
        let renderable = RenderableWindow::new(window, graphics);
        Box::new(MyApp { renderable })
    });
}

impl App for MyApp {
    fn update(&mut self, ctx: &mut AppCtx) { /* game logic */ }
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        /* rendering */
    }
}
```

**Engine approach (using plugins):**
```rust
let engine = Engine::builder()
    .add_plugins(DefaultPlugins)
    .add_plugin(MyPlugin)
    .build();

let assets = engine.get::<AssetServer>().unwrap();
```

## Important Development Notes

**Rust version:** Stable (defined in `rust-toolchain.toml`)

**Edition:** Rust 2024 (Cargo.toml workspace.package.edition)

**Key external dependencies:**
- WGPU 27.0.1 (pinned version)
- winit 0.30.12
- cosmic-text 0.12
- taffy 0.5
- egui/egui-wgpu 0.33

**Performance-critical areas:**
- UI dirty flag propagation (see `crates/astrelis-ui/src/dirty.rs`)
- Text shaping and caching (see `crates/astrelis-text/src/shaping.rs`)
- Asset handle lookups (SparseSet in `crates/astrelis-assets`)

**Common pitfalls:**
- Forgetting to drop render pass before `frame.finish()`
- Not marking UI nodes dirty after state changes
- Creating GraphicsContext multiple times (should be singleton)
- Mixing up generational handle generations (always use AssetServer API)

## File Locations for Common Tasks

**Adding a new widget:** `crates/astrelis-ui/src/widget/`
**Modifying render pipeline:** `crates/astrelis-render/src/context.rs`
**Asset loading logic:** `crates/astrelis-assets/src/server.rs`
**Plugin system:** `crates/astrelis/src/plugin.rs`
**UI dirty flag logic:** `crates/astrelis-ui/src/dirty.rs`
**Text shaping:** `crates/astrelis-text/src/shaping.rs`

## Code Style

- Use workspace-level dependencies (defined in root Cargo.toml)
- Prefer `ahash` HashMap/HashSet over std for performance
- Use `glam` types (Vec2, Vec3, Mat4) for math
- Document public APIs with `///` doc comments
- Use `tracing` macros (trace!, debug!, info!, warn!, error!) not println!
- Initialize profiling in examples with `init_profiling(ProfilingBackend::PuffinHttp)`
