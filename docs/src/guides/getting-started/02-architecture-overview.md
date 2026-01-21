# Architecture Overview

Astrelis is designed as a **modular Rust game engine** where you pick and choose the components you need. This guide explains Astrelis's architecture, core patterns, and how the pieces fit together.

## Philosophy: Modular Composition

Unlike monolithic engines, Astrelis is built as **independent crates** that you can use à la carte:

```toml
# Minimal: Just rendering
[dependencies]
astrelis-core = "..."
astrelis-render = "..."

# Add UI when you need it
astrelis-ui = "..."

# Add assets when you need them
astrelis-assets = "..."
```

**Benefits**:
- **Smaller binaries**: Only include what you use
- **Faster compile times**: Don't build unused features
- **Flexibility**: Swap out components or use third-party alternatives
- **Clear dependencies**: Each crate has well-defined responsibilities

## Crate Dependency Layers

Astrelis is organized in **dependency layers**. Lower layers don't depend on higher layers:

```
┌─────────────────────────────────────────────────────────┐
│  Main Crate (astrelis)                                  │
│  - Plugin system                                        │
│  - Engine builder                                       │
│  - Resource management                                  │
└─────────────────────────────────────────────────────────┘
                          │
        ┌─────────────────┴─────────────────┐
        │                                   │
┌───────▼──────────┐              ┌────────▼─────────┐
│  Integration     │              │  Content Layer   │
│  - egui          │              │  - UI            │
│  - (future)      │              │  - Text          │
└──────────────────┘              └────────┬─────────┘
                                           │
                           ┌───────────────┴──────────────┐
                           │                              │
                  ┌────────▼────────┐          ┌─────────▼────────┐
                  │  Asset Layer    │          │  Rendering Layer │
                  │  - assets       │          │  - render        │
                  └─────────────────┘          └──────────────────┘
                                                        │
                                          ┌─────────────┴──────────────┐
                                          │     Platform Layer         │
                                          │     - winit (windowing)    │
                                          └────────────────────────────┘
                                                        │
                                          ┌─────────────┴──────────────┐
                                          │     Core Layer             │
                                          │     - math (glam)          │
                                          │     - logging (tracing)    │
                                          │     - profiling (puffin)   │
                                          │     - geometry types       │
                                          └────────────────────────────┘
```

### Core Layer (`astrelis-core`)

**Foundation crate** with no game-specific logic:

- **Math**: Re-exports `glam` types (`Vec2`, `Vec3`, `Mat4`, etc.)
- **Logging**: Initializes `tracing` for structured logging
- **Profiling**: Integration with `puffin` for performance analysis
- **Geometry**: Common types (`Rect`, `Size`, `Point`, `Transform`)
- **Custom allocators**: `ahash` HashMap/HashSet for performance

**Dependencies**: Only external crates (`glam`, `tracing`, `puffin`)

**When to use**: Every Astrelis project needs this.

### Platform Layer (`astrelis-winit`)

**Window and event management**:

- **`App` trait**: Your game loop lifecycle (`on_start`, `update`, `render`, `on_exit`)
- **`run_app()`**: Entry point that runs the event loop
- **`WindowDescriptor`**: Window configuration (title, size, fullscreen, etc.)
- **`EventBatch`**: Batched input events per frame
- **`AppCtx`**: Context with frame timing and system info

**Dependencies**: `astrelis-core`, `winit` 0.30

**When to use**: Every game with a window (which is almost all games).

### Rendering Layer (`astrelis-render`)

**Low-level GPU rendering via WGPU**:

- **`GraphicsContext`**: GPU device, queue, adapter (shared with `Arc`)
- **`WindowContext`**: Per-window surface management
- **`FrameContext`**: Per-frame rendering (RAII - automatically submits)
- **`RenderPass`**: Render pass with automatic lifecycle
- **`Framebuffer`**: Render-to-texture support
- **`Renderer`**: Utility for creating shaders, buffers, bind groups

**Dependencies**: `astrelis-core`, `wgpu` 27.0.1

**When to use**: For custom rendering, 2D/3D graphics, compute shaders.

### Asset Layer (`astrelis-assets`)

**Type-safe async asset loading**:

- **`AssetServer`**: Central asset management
- **`Handle<T>`**: Type-safe generational handle (prevents use-after-free)
- **`AssetLoader` trait**: Custom asset type loaders
- **`AssetEvent`**: Loaded, Modified (hot-reload), Unloaded events
- **SparseSet storage**: O(1) handle lookups

**Dependencies**: `astrelis-core`, `notify` (file watching)

**When to use**: Loading textures, fonts, shaders, configs, audio, etc.

### Content Layer

#### Text Rendering (`astrelis-text`)

**GPU-accelerated text rendering**:

- **`cosmic-text` integration**: Modern text shaping (Unicode, bidirectional, etc.)
- **GPU texture atlas**: Glyphs cached in GPU texture
- **Font fallback**: Multiple fonts for missing glyphs
- **Rich text**: Multiple styles in one text block
- **Text effects**: Outlines, shadows, gradients (future)

**Dependencies**: `astrelis-core`, `astrelis-render`, `cosmic-text` 0.12

**When to use**: Any text rendering (UI labels, buttons, text editors, etc.)

#### UI System (`astrelis-ui`)

**Retained-mode declarative UI**:

- **`UiSystem`**: Complete UI solution with layout and rendering
- **`UiCore`**: Render-agnostic UI tree and event handling
- **`UiRenderer`**: GPU instanced rendering (single draw call per widget type)
- **Widget system**: Text, Button, Container, Image, etc.
- **Flexbox layout**: Powered by `taffy` (CSS Flexbox engine)
- **Dirty flags**: Fine-grained updates (color-only, text-shaping, layout, geometry)
- **Event system**: Click, hover, keyboard navigation, focus

**Dependencies**: `astrelis-core`, `astrelis-render`, `astrelis-text`, `taffy` 0.5

**When to use**: Building in-game UI, menus, HUDs, editors.

### Integration Layer

#### egui Integration (`astrelis-egui`)

**Immediate-mode UI via egui**:

- **`egui-wgpu` backend**: Renders egui with WGPU
- **Hybrid UI**: Use egui for tools/debug UI, astrelis-ui for game UI

**Dependencies**: `astrelis-render`, `egui` 0.33, `egui-wgpu` 0.33

**When to use**: Quick debug UI, dev tools, prototyping.

### Main Crate (`astrelis`)

**Engine facade and plugin system**:

- **`Engine`**: Main engine struct with resource storage
- **`EngineBuilder`**: Fluent API for engine configuration
- **`Plugin` trait**: Extensibility via plugins
- **`Resources`**: Type-erased resource storage (like Bevy)
- **`DefaultPlugins`**: Common plugin bundle

**Dependencies**: All other astrelis crates

**When to use**: For plugin-based architecture and complex games.

### WIP Crates (Not Yet Stable)

These crates are in development and not ready for production:

- **`astrelis-input`**: Input state management (keyboard, mouse, gamepad)
- **`astrelis-audio`**: Audio playback and spatialization
- **`astrelis-ecs`**: Entity Component System (ECS)
- **`astrelis-scene`**: Scene graph and hierarchy management

## Core Architectural Patterns

### 1. Arc-Based Shared Ownership

Astrelis uses `Arc<T>` for shared ownership of expensive resources:

```rust
// Create once, clone the Arc (cheap - just increments reference count)
let graphics = GraphicsContext::new_owned_sync();

// Share with multiple owners
let ui = UiSystem::new(graphics.clone(), window_manager.clone());
let renderer = CustomRenderer::new(graphics.clone());
let sprite_batch = SpriteBatch::new(graphics.clone());
```

**Why?**: Rust doesn't have garbage collection. `Arc` provides automatic cleanup when the last owner drops.

**Comparison**:
- **Unity/C#**: Garbage collector handles this automatically
- **Bevy**: Uses ECS World - resources are stored centrally
- **Astrelis**: Explicit `Arc` cloning

### 2. RAII Resource Management

**RAII (Resource Acquisition Is Initialization)** means resources are cleaned up when they go out of scope:

```rust
fn render(&mut self, _ctx: &mut AppCtx, _window_id: WindowId, _events: &mut EventBatch) {
    let mut frame = self.window.begin_drawing();

    frame.clear_and_render(
        RenderTarget::Surface,
        Color::BLACK,
        |pass| {
            self.ui.render(pass.descriptor());
        }, // pass is dropped here - render pass ends
    );

    frame.finish(); // frame is dropped here - commands submitted, surface presented
}
```

**Key RAII types**:
- **`FrameContext`**: `Drop` impl submits GPU commands and presents
- **`RenderPass`**: `Drop` impl ends render pass
- **`Guard` types**: Lock guards, file handles, etc.

**Why?**: Prevents resource leaks and ensures correct cleanup order.

### 3. Type-Safe Handles with Generation Counters

Asset handles use **generational indices** to prevent use-after-free:

```rust
let handle: Handle<Texture> = assets.load("player.png");
// Handle = { index: 42, generation: 1 }

// Later, asset is unloaded and slot is reused
assets.unload(handle);
let new_handle = assets.load("enemy.png");
// new_handle = { index: 42, generation: 2 }  <- different generation!

// Old handle is now invalid - get() returns None
assert!(assets.get(handle).is_none());  // Safe - no use-after-free!
```

**Benefits**:
- **Type safety**: `Handle<Texture>` ≠ `Handle<Font>`
- **No dangling pointers**: Invalid handles return `None`, not crashes
- **No manual IDs**: Automatic generation tracking

### 4. Dirty Flags for Incremental Updates

UI system uses **bitflags** to minimize work:

```rust
// Just changing text color (0.1ms)
ui.update_color("label", Color::RED);  // Sets COLOR_ONLY flag

// Changing text content (5-10ms)
ui.update_text("label", "New text");   // Sets TEXT_SHAPING flag

// Changing size (10-20ms)
ui.update_size("label", Size::new(200, 100));  // Sets LAYOUT flag
```

**Dirty flags**:
- `COLOR_ONLY`: Skip layout, text shaping, geometry
- `TEXT_SHAPING`: Needs text reshaping, but not layout
- `LAYOUT`: Needs layout recomputation
- `GEOMETRY`: Needs geometry rebuild (borders, radius)

**Performance**: Color updates are ~200x faster than full rebuilds.

### 5. Plugin System with Dependencies

Plugins extend the engine with automatic dependency resolution:

```rust
struct RenderPlugin;
impl Plugin for RenderPlugin {
    fn dependencies(&self) -> Vec<TypeId> {
        vec![typeid!(GraphicsPlugin)]  // Requires GraphicsPlugin first
    }

    fn build(&self, engine: &mut Engine) {
        engine.insert(RenderSystem::new(/* ... */));
    }
}

let engine = Engine::builder()
    .add_plugin(GraphicsPlugin)  // Added first
    .add_plugin(RenderPlugin)    // Automatically scheduled after GraphicsPlugin
    .build();
```

**Benefits**:
- **Topological sorting**: Dependencies run in correct order
- **Type-safe**: No magic strings
- **Composable**: Combine plugins from different sources

## Data Flow and Ownership

### Typical Frame Flow

```
1. App::update()
   - Update game state
   - Handle input
   - Update physics/AI/etc.
   - Mutate state directly

2. App::render()
   - begin_drawing() -> FrameContext
   - Create render passes
   - Render UI, sprites, meshes
   - finish() -> Submit and present

3. Repeat next frame
```

### Ownership Patterns

**Owned State**:
```rust
struct MyGame {
    player: Player,               // Owned
    enemies: Vec<Enemy>,          // Owned collection
    config: GameConfig,           // Owned
}
```

**Shared State (Arc)**:
```rust
struct MyGame {
    graphics: Arc<GraphicsContext>,    // Shared (cheap clone)
    window_manager: Arc<WindowManager>, // Shared
}
```

**Borrowed State**:
```rust
impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx) {  // Borrowed context
        let duration = ctx.last_frame_duration();  // Borrow fields
    }
}
```

## Comparison to Other Engines

### Astrelis vs Unity

| Aspect | Unity | Astrelis |
|--------|-------|----------|
| **Language** | C# (GC) | Rust (no GC) |
| **Architecture** | GameObject + Components | Manual state management |
| **Memory** | Garbage collected | Arc + RAII |
| **Rendering** | Automatic cameras | Manual render passes |
| **UI** | UGUI/UI Toolkit | Retained-mode Flexbox |
| **Assets** | AssetDatabase | `AssetServer` with handles |
| **Extensibility** | Packages | Plugins |

### Astrelis vs Bevy

| Aspect | Bevy | Astrelis |
|--------|------|----------|
| **Architecture** | ECS-first | Traditional + optional ECS |
| **Game Loop** | Automatic systems | Manual methods |
| **Parallelism** | Automatic | Manual (Rayon, async) |
| **Rendering** | High-level (sprites) | Low-level (render passes) |
| **UI** | ECS-based | Retained-mode |
| **Flexibility** | Opinionated | Maximum freedom |

### Astrelis vs Godot

| Aspect | Godot | Astrelis |
|--------|-------|----------|
| **Language** | GDScript/C# | Rust |
| **Architecture** | Scene tree (nodes) | Manual state |
| **UI** | Control nodes | Declarative widgets |
| **Rendering** | Scene-based | Manual passes |
| **Assets** | Resource system | `AssetServer` |
| **Editor** | Full editor | Code-first (editor future) |

## Design Principles

1. **Modularity**: Use only what you need
2. **Explicitness**: No magic - you control the game loop
3. **Type Safety**: Leverage Rust's type system
4. **Performance**: Zero-cost abstractions where possible
5. **Flexibility**: Not opinionated about architecture
6. **Modern Rust**: Use Rust 2024 features
7. **WGPU**: Modern GPU API (Vulkan/Metal/DX12)

## Project Structure Recommendations

### Small Project (Single Crate)

```
my_game/
├── Cargo.toml
├── src/
│   ├── main.rs        # Entry point + App impl
│   ├── player.rs      # Player logic
│   ├── enemy.rs       # Enemy logic
│   └── ui.rs          # UI setup
└── assets/
    └── textures/
```

### Medium Project (Workspace)

```
my_game/
├── Cargo.toml         # Workspace root
├── game/              # Main binary
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── game_logic/        # Shared library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── player.rs
│       └── enemy.rs
└── assets/
```

### Large Project (Plugin-Based)

```
my_game/
├── Cargo.toml
├── game/              # Main binary
├── core/              # Core game systems
├── plugins/           # Game plugins
│   ├── gameplay/
│   ├── rendering/
│   └── ui/
├── editor/            # Editor (future)
└── assets/
```

## Next Steps

Now that you understand Astrelis's architecture:

1. **[Hello Window](03-hello-window.md)** - Build your first app
2. **[Rendering Fundamentals](04-rendering-fundamentals.md)** - Learn rendering patterns
3. **[First UI](05-first-ui.md)** - Create interactive UI
4. **[Plugin System Guide](../plugin-system/creating-plugins.md)** - Build extensible engines

## Further Reading

- [Crate Reference](../../crates/) - Detailed crate documentation
- [Architecture Decisions](../../architecture/) - Design rationale
- [Examples Index](../../examples-index.md) - Code examples

You're now ready to start building with Astrelis!
