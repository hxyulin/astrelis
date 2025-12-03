# Core Systems

The `astrelis-core` crate provides foundational utilities and data structures used throughout the engine. It has minimal dependencies and is designed to be the base layer for all other crates.

## Module Structure

```
astrelis-core/
├── alloc.rs      - Optimized collections and memory structures
├── math.rs       - Math type re-exports and packed types
├── profiling.rs  - Performance profiling infrastructure
└── logging.rs    - Logging setup utilities
```

## Math Types

### Fast Math (`math::fast`)

Re-exports the `glam` crate for high-performance SIMD math operations:

- `Vec2`, `Vec3`, `Vec4` - Vector types
- `Mat2`, `Mat3`, `Mat4` - Matrix types
- `Quat` - Quaternion for rotations
- `Affine2`, `Affine3` - Affine transformations

All engine crates import math types through `astrelis_core::math` rather than directly from `glam`, ensuring type consistency across the workspace.

### Packed Types (`math::packed`)

GPU-friendly variants implementing `Pod` and `Zeroable` from `bytemuck`:

```rust
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}
```

These types can be directly uploaded to GPU buffers without conversion:

```rust
use astrelis_core::math::{PackedVec2, PackedVec3};
use bytemuck::cast_slice;

let vertices: &[PackedVec3] = &[/* ... */];
let bytes: &[u8] = cast_slice(vertices);
// Upload bytes to GPU buffer
```

### Mint Integration

The `mint` crate provides interoperability between different math libraries. `glam` types convert to/from mint types, allowing integration with external libraries.

## Optimized Collections

### AHash-based Collections

Fast, non-cryptographic hashing for game use cases:

```rust
use astrelis_core::alloc::{HashMap, HashSet};

let mut map = HashMap::new();
map.insert("player", entity_id);

let mut set = HashSet::new();
set.insert(component_id);
```

**Performance**: 2-3x faster than `std::collections::HashMap` for typical game keys (integers, strings).

**When to use**: Any hash map/set where cryptographic security is not required (almost all game code).

### SparseSet

Generational index-based sparse storage with O(1) operations:

```rust
use astrelis_core::alloc::{SparseSet, IndexSlot};

let mut entities = SparseSet::<EntityData>::new();

// Insert returns generational index
let entity = entities.push(EntityData { /* ... */ });

// Access by index (panics if generation mismatch)
let data = entities.get(entity);

// Remove increments generation
entities.remove(entity);

// Old index now invalid (would panic)
// entities.get(entity); // Panic: use after free!

// Reusing slot creates new generation
let new_entity = entities.push(EntityData { /* ... */ });
assert_eq!(entity.index(), new_entity.index()); // Same slot
assert_ne!(entity.generation(), new_entity.generation()); // Different gen
```

#### IndexSlot Design

Efficient generational index packed into 64 bits:

```
┌────────────────────────────────────────────────────────┐
│  Bits 63-32: Generation (u32)                          │
│  Bits 31-0:  Index + 1 (u32, +1 for NonZero)           │
└────────────────────────────────────────────────────────┘
```

Using `NonZeroU64` enables niche optimization:
```rust
assert_eq!(size_of::<IndexSlot>(), size_of::<Option<IndexSlot>>());
// Both are 8 bytes - no space overhead for Option
```

#### Use Cases

- **Entity storage** in ECS systems
- **Handle systems** for resources (textures, fonts, etc.)
- **Any data** requiring stable handles with generation checking
- **Free lists** where slots are reused frequently

#### Performance

- **Insert**: O(1) - reuses free slots or appends
- **Remove**: O(1) - marks slot free, increments generation
- **Access**: O(1) - direct indexing with generation check
- **Iteration**: O(n) - skip free slots, cache-friendly

## Profiling Infrastructure

Integration with the `puffin` profiling framework for performance analysis.

### Initialization

```rust
use astrelis_core::profiling::{init_profiling, ProfilingBackend, new_frame};

// Start profiling server (call once at startup)
init_profiling(ProfilingBackend::PuffinHttp);

// In main loop
loop {
    new_frame(); // Mark frame boundary
    
    // Your game logic here
}
```

### Profiling Functions

```rust
use astrelis_core::profiling::profile_function;

fn expensive_operation() {
    profile_function!(); // Automatically scoped to function
    
    // Function body
}
```

### Manual Scopes

```rust
use astrelis_core::profiling::profile_scope;

fn complex_function() {
    profile_function!();
    
    {
        profile_scope!("physics_step");
        // Physics simulation
    }
    
    {
        profile_scope!("ai_update");
        // AI logic
    }
}
```

### Viewing Profiles

1. Start your application with profiling enabled
2. Run `puffin_viewer` from CLI
3. Navigate to `http://127.0.0.1:8585` in the viewer
4. View frame timings, hierarchical scopes, flamegraphs

### Zero-Cost When Disabled

Profiling has minimal overhead when compiled with optimizations:
- Macros expand to no-ops when profiling is disabled
- No runtime checks in release builds
- Scopes are compile-time eliminated

### Current Profiling Coverage

Performance-critical areas instrumented:

- **Text rendering**: measurement, layout, drawing, atlas updates
- **UI system**: layout computation, rendering, event handling
- **Rendering**: frame acquisition, command encoding, submission
- **Collections**: SparseSet operations (in debug builds)

## Logging

Simple wrapper around `tracing` for structured logging:

```rust
use tracing::{info, warn, error, debug, trace};

info!("Application started");
warn!("Low memory: {} MB free", free_memory);
error!(error = ?err, "Failed to load asset");
debug!(entity = ?entity_id, "Spawned entity");
```

Logging is configured with `tracing-subscriber`:

```rust
use astrelis_core::logging;

// Initialize with default settings
logging::init();

// Or with custom filter
logging::init_with_filter("info,astrelis_render=debug");
```

## Design Rationale

### Why Static Lifetimes?

Core resources like `GraphicsContext` use `&'static` references:

**Benefits**:
- Eliminates lifetime parameters from public APIs
- Simplifies resource sharing across systems
- No runtime reference counting overhead
- Clear ownership semantics (owned by program)

**Trade-offs**:
- Resources persist for entire program lifetime
- Cannot be freed early (acceptable for game engines)
- Requires `Box::leak` or similar patterns

### Why AHash Over SipHash?

Rust's default hasher (SipHash) is cryptographically secure but slower:
- **SipHash**: DoS-resistant, ~5ns per hash
- **AHash**: Non-cryptographic, ~2ns per hash

For game engines:
- DoS attacks are not a concern (not user-controlled keys)
- Hash table performance is critical (entity lookups, component access)
- 2-3x speedup is significant in hot paths

### Why Generational Indices?

Direct pointers/references are unsafe with dynamic allocation:
- Dangling pointers when entities are destroyed
- Use-after-free bugs are common in complex systems

Generational indices provide:
- **Safety**: Invalid handles detected at runtime
- **Performance**: No reference counting, no atomic operations
- **Debuggability**: Clear error messages on stale access
- **Determinism**: Same entity IDs across runs (with deterministic allocation)

### Why Puffin Over Other Profilers?

Comparison with alternatives:

- **Tracy**: More features, but complex integration, C++ dependency
- **perf/Instruments**: OS-specific, requires external tools
- **criterion**: For benchmarks, not runtime profiling
- **Puffin**: Simple Rust integration, frame-based (perfect for games), visual viewer

Puffin chosen for:
- Easy integration (single function call)
- Frame-based profiling model matches game loops
- Built-in HTTP server and viewer
- Zero cost when disabled
- Pure Rust implementation

## Integration Example

Typical usage in an engine crate:

```rust
use astrelis_core::{
    alloc::{HashMap, SparseSet},
    math::{Vec2, Vec3},
    profiling::profile_function,
};

pub struct EntityManager {
    entities: SparseSet<Entity>,
    components: HashMap<ComponentId, Box<dyn Component>>,
}

impl EntityManager {
    pub fn spawn(&mut self, entity: Entity) -> IndexSlot {
        profile_function!();
        self.entities.push(entity)
    }
    
    pub fn add_component(&mut self, entity: IndexSlot, component: Box<dyn Component>) {
        profile_function!();
        let entity = self.entities.get(entity);
        self.components.insert(entity.id(), component);
    }
    
    pub fn get_position(&self, entity: IndexSlot) -> Vec3 {
        profile_function!();
        let entity = self.entities.get(entity);
        entity.transform.position
    }
}
```

## Future Enhancements

Planned additions to core:

1. **SmallVec** - Stack-allocated vectors for small arrays (children lists)
2. **SmolStr** - Inlined string type for small strings (component names)
3. **Arena allocators** - Bulk allocation for temporary data
4. **Profiling stats** - Memory allocations, CPU/GPU timings
5. **Custom allocators** - Per-subsystem allocation tracking
6. **Math utilities** - Common game math (lerp, easing, etc.)