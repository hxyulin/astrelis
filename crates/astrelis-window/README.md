# astrelis-window

Backend-agnostic windowing traits and types for the Astrelis engine.

This crate defines the platform-independent windowing abstraction (Layer 2).
It has **zero platform dependencies** — only `astrelis-core` and
`raw-window-handle` for GPU interop. Concrete backends like
[`astrelis-window-winit`](../astrelis-window-winit) implement the traits.

## Architecture

```
WindowBackend::new()     — initialize the platform
     │
     ▼
WindowBackend::run(handler)  — enter the event loop
     │
     ├─ AppHandler::on_lifecycle(ctx, Resumed)
     │       └─ ctx.create_window(attrs) → WindowId
     │
     ├─ AppHandler::on_window_event(ctx, id, event)
     │       └─ ctx.window(id) → &dyn Window
     │
     └─ AppHandler::on_events_cleared(ctx)
             └─ ctx.set_control_flow(Poll | Wait | WaitUntil)
```

## Key Types

| Type | Description |
|------|-------------|
| `WindowBackend` | Top-level entry point trait |
| `AppHandler` | User callback trait (lifecycle, events, frame) |
| `EventLoopContext` | Create/destroy/access windows during callbacks |
| `Window` | Trait for manipulating a window (~40 methods) |
| `WindowBuilder` | Fluent builder for `WindowAttributes` (20+ fields) |
| `WindowEvent` | Comprehensive event enum (25+ variants) |
| `Capability` | Runtime query for platform-specific features |

## Type Safety

Sizes and positions are type-safe at two levels:

- **Logical vs Physical** — DPI-aware coordinate spaces (from `astrelis-core`)
- **Inner vs Outer** — drawable area vs window frame (newtypes in `types` module)

```rust
use astrelis_window::types::{InnerSize, OuterSize, LogicalInnerSize};

// These are different types — can't mix them:
let inner: InnerSize = InnerSize::new(800.0, 600.0);
let outer: OuterSize = OuterSize::new(820.0, 640.0);
// inner == outer;  // compile error!

// Convert between logical and physical:
let logical = LogicalInnerSize::new(400.0, 300.0);
let physical: InnerSize = logical.to_physical(2.0);
```

## Control Flow

| Mode | Use Case |
|------|----------|
| `ControlFlow::Poll` | Games — continuous rendering, never sleeps |
| `ControlFlow::Wait` | Apps — sleeps until events arrive (low CPU) |
| `ControlFlow::WaitUntil(duration)` | Hybrid — periodic updates (animations, cursor blink) |

## License

MIT
