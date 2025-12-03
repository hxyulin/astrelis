# Architecture Overview

Astrelis is structured as a modular, layered game engine with clear separation of concerns between subsystems. The architecture emphasizes flexibility, performance, and composability.

## System Layers

```
┌─────────────────────────────────────────────────────────┐
│              Application Layer (Your Game)              │
└─────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────┐
│         High-Level Systems (UI, Scene, ECS)             │
│  ┌──────────────┐  ┌──────────┐  ┌──────────────────┐  │
│  │ astrelis-ui  │  │  Scene   │  │  ECS (planned)   │  │
│  └──────────────┘  └──────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────┐
│              Rendering & Media Layer                    │
│  ┌──────────────────┐  ┌────────────────────────────┐  │
│  │  astrelis-text   │  │  astrelis-render           │  │
│  │  (cosmic-text)   │  │  (WGPU abstraction)        │  │
│  └──────────────────┘  └────────────────────────────┘  │
│  ┌──────────────────┐  ┌────────────────────────────┐  │
│  │ astrelis-assets  │  │  astrelis-audio (planned)  │  │
│  └──────────────────┘  └────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────┐
│               Platform Abstraction                      │
│              ┌──────────────────┐                       │
│              │ astrelis-winit   │                       │
│              │ (event handling) │                       │
│              └──────────────────┘                       │
└─────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────┐
│                  Foundation Layer                       │
│              ┌──────────────────┐                       │
│              │  astrelis-core   │                       │
│              │  (math, alloc,   │                       │
│              │   profiling)     │                       │
│              └──────────────────┘                       │
└─────────────────────────────────────────────────────────┘
```

## Core Principles

### 1. Dependency Flow

Dependencies flow downward through layers:
- High-level systems depend on rendering and platform layers
- Rendering layer depends on platform and foundation
- Foundation layer has minimal external dependencies

No circular dependencies exist between crates. Each crate can be used independently if needed.

### 2. Static Lifetime Pattern

Critical shared resources use static lifetimes to simplify the API:
- **GraphicsContext**: Created once, leaked as `&'static`, accessible globally
- **FontSystem**: Global font database for text rendering
- Eliminates lifetime parameters from most public APIs
- Simplifies resource sharing across systems

Trade-off: Resources persist for program lifetime (acceptable for game engines).

### 3. Workspace-Level Dependency Management

All external dependencies declared in root `Cargo.toml`:
- Ensures version consistency across all crates
- Simplifies dependency updates
- Prevents version conflicts
- Centralizes feature flag management

### 4. Type Consistency

`astrelis-core::math` provides canonical math types:
- Re-exports `glam` vector/matrix types
- All crates use these instead of direct `glam` imports
- Provides `packed` variants (Pod + Zeroable) for GPU uploads
- `mint` integration for interoperability

## Data Flow

### Frame Lifecycle

```
1. Event Collection (winit)
   ↓
2. Event Distribution (App::render per window)
   ↓
3. Update Logic (App::update once per frame)
   ↓
4. UI Updates (incremental, dirty-tracking)
   ↓
5. Layout Computation (Taffy, lazy)
   ↓
6. Rendering
   - Text rendering (cosmic-text → atlas)
   - UI rendering (batched quads)
   - Scene rendering (planned)
   ↓
7. Frame Presentation (WGPU)
```

### Event Flow

```
OS/Platform Events (winit)
   ↓
EventQueue (per window)
   ↓
EventBatch (consumed by app)
   ↓
UiEventSystem (hit testing, dispatching)
   ↓
Widget Callbacks (button clicks, text input, etc.)
```

## Key Design Decisions

### 1. Taffy for Layout

- Industry-standard flexbox/grid implementation
- Proven in production (used by Dioxus, Bevy UI)
- Separates layout logic from rendering
- Allows incremental layout updates

### 2. Cosmic-text for Typography

- Modern Rust text layout engine
- Handles complex scripts, ligatures, shaping
- GPU texture atlas for efficient rendering
- System font integration

### 3. WGPU for Graphics

- Cross-platform graphics API (Vulkan/Metal/DX12/WebGPU)
- Safe Rust wrapper around native graphics
- Future-proof (WebGPU standard)
- Excellent tooling and ecosystem

### 4. Puffin for Profiling

- Zero-cost when disabled
- Frame-based profiling perfect for games
- Visual profiler (puffin_viewer)
- Scoped profiling via macros

### 5. Optimized Collections

- **AHash** instead of default hasher (2-3x faster, non-cryptographic)
- **SparseSet** for generational indices (entity-like storage)
- **IndexMap** for stable iteration order with O(1) access
- Future: SmallVec, SmolStr for small-size optimizations

## Memory Management

### Allocation Strategy

- Minimize allocations in hot paths (profiled)
- Reuse buffers where possible (text rendering, UI batching)
- Cache computed data (text measurements, layout results)
- Use stack allocation for small fixed-size data

### Resource Lifetimes

- **Static**: GraphicsContext, font systems (leaked)
- **Per-Window**: Surfaces, swapchains, render contexts
- **Per-Frame**: Command buffers, render passes (dropped)
- **Cached**: Text atlas, UI vertex buffers (resized as needed)

### Generational Indices

`SparseSet<T>` provides safe handle-based access:
- `IndexSlot` = 32-bit generation + 32-bit index in u64
- Detects use-after-free (generation mismatch panics)
- O(1) insert/remove/lookup
- Niche optimization: `Option<IndexSlot>` = 8 bytes

## Concurrency Model

Current: **Single-threaded** (game loop on main thread)
- Graphics APIs require main thread access
- Simplifies state management
- No data races by design

Future considerations:
- Asset loading on background threads
- Audio mixing on dedicated thread
- Physics simulation off main thread
- Job system for parallel ECS queries

## Extension Points

The engine provides several ways to extend functionality:

### 1. Custom Widgets

Implement `Widget` trait for new UI elements:
```rust
pub trait Widget {
    fn style(&self) -> &Style;
    fn render(&self, ctx: &mut RenderContext);
    fn handle_event(&mut self, event: &UiEvent) -> bool;
    // ... more methods
}
```

### 2. Custom Renderers

Build on `Renderer` base for new rendering systems:
- Text renderer example in `astrelis-text`
- UI renderer example in `astrelis-ui`
- Future: 2D/3D scene renderers

### 3. Event Handlers

Hook into event system at multiple levels:
- Per-widget callbacks
- Global event handlers in `App::render`
- Custom event types via `Event` enum extension

### 4. Layout Measure Functions

Provide custom measurement for complex widgets:
```rust
fn measure(&self, constraints: Size) -> Size {
    // Custom measurement logic
}
```

## Performance Characteristics

### UI System

- **Full rebuild**: O(n) where n = widget count
- **Incremental update**: O(m) where m = dirty widget count
- **Layout**: O(n) with Taffy, but lazy (only dirty subtrees)
- **Rendering**: O(n) with batching (constant draw calls)
- **Hit testing**: O(log n) with spatial optimization (future)

### Text Rendering

- **First render**: Layout + atlas upload = ~1-2ms per text node
- **Cached render**: Atlas lookup only = ~0.1ms per text node
- **Measurement**: Requires layout = ~0.5ms average
- Measurements cached per-node in UI tree

### Collection Performance

- **AHashMap**: 2-3x faster than std HashMap for small keys
- **SparseSet**: O(1) all operations, cache-friendly iteration
- **IndexMap**: Maintains insertion order, slightly slower than HashMap

## Testing Strategy

### Current

- Unit tests in individual crates
- Integration via examples
- Visual testing (manual)
- Profiling with puffin

### Future

- Snapshot testing for rendering
- Property-based testing for layout
- Benchmarks for hot paths
- Automated visual regression tests

## Next Steps

Areas planned for development:

1. **ECS System** - Entity-component-system architecture
2. **Asset Pipeline** - Loading, hot-reloading, asset management
3. **Scene Graph** - Hierarchical transforms, cameras, rendering
4. **Audio System** - Playback, mixing, spatial audio
5. **Input System** - Unified input handling abstraction
6. **Scripting** - Lua/WASM integration for gameplay logic