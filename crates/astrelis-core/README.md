# astrelis-core

Core types and math for the Astrelis engine.

This is the foundational crate (Layer 0) that all other engine crates depend on.
It contains only pure types with no runtime behavior, I/O, or platform dependencies.

## Modules

| Module | Description |
|--------|-------------|
| `math` | Linear algebra re-exports from `glam`, plus `packed` GPU-ready `#[repr(C)]` types |
| `color` | RGBA color type with named constants and format conversions |
| `geometry` | Coordinate-space-aware primitives (`Point`, `Size`, `Rect`) with `Logical`/`Physical` markers |
| `id` | Generic type-safe `Id<T>` handle — prevents mixing IDs from different domains |

## Usage

```rust
use astrelis_core::math::{Vec2, Vec3, Mat4};
use astrelis_core::math::packed;
use astrelis_core::color::Color;
use astrelis_core::geometry::{Logical, Physical, Point, Size, Rect};
use astrelis_core::id::Id;

// Math — use glam types directly
let position = Vec3::new(1.0, 2.0, 3.0);

// Packed types for GPU upload
let gpu_pos: packed::Vec3 = position.into();
let bytes: &[u8] = bytemuck::bytes_of(&gpu_pos);

// Colors
let red = Color::RED;
let custom = Color::from_rgba8(128, 64, 32, 255);
let packed_u32 = custom.to_u32();

// Type-safe coordinate spaces
let logical = Point::<Logical>::new(100.0, 200.0);
let physical: Point<Physical> = logical.to_physical(2.0); // HiDPI scale
assert_eq!(physical.x, 200.0);

// Type-safe IDs
struct Window;
struct Entity;
let win_id: Id<Window> = Id::new(1);
let ent_id: Id<Entity> = Id::new(1);
// win_id == ent_id; // compile error — different domains!
```

## License

MIT
