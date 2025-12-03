# astrelis-core

The `astrelis-core` crate provides the foundational building blocks for the Astrelis engine. It contains essential utilities, data structures, and types used across all other crates.

## Features

- **Math**: Re-exports `glam` types (`Vec2`, `Vec3`, `Mat4`, etc.) and provides GPU-friendly packed types.
- **Allocation**: Optimized collections (`HashMap`, `HashSet` using AHash) and data structures (`SparseSet`).
- **Profiling**: Integration with `puffin` for performance analysis.
- **Logging**: Setup for `tracing` based logging.
- **Geometry**: Common geometry types like `Size`, `Rect`.

## Usage

```rust
use astrelis_core::{
    math::Vec2,
    alloc::{HashMap, SparseSet},
    profiling::profile_function,
};

// Math
let position = Vec2::new(10.0, 20.0);

// Collections
let mut map = HashMap::new();
map.insert("key", "value");

// Profiling
fn update() {
    profile_function!();
    // ...
}
```

## Modules

### `alloc`

Provides optimized memory structures:
- `HashMap` / `HashSet`: Uses `ahash` for faster hashing than std.
- `SparseSet`: Generational index-based storage for ECS-like patterns.
- `IndexSlot`: Safe handle for `SparseSet`.

### `math`

Canonical math types for the engine:
- `fast`: Re-exports `glam` types for SIMD-accelerated math.
- `packed`: `Pod` + `Zeroable` types for GPU buffers.

### `profiling`

Performance instrumentation:
- `init_profiling`: Sets up the puffin server.
- `profile_function!`: Scopes profiling to the current function.
- `profile_scope!`: Creates a named profiling scope.

### `logging`

Structured logging:
- `init`: Initializes the tracing subscriber.
- `init_with_filter`: Initializes with a custom filter string.
