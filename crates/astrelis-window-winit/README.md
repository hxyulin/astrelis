# astrelis-window-winit

[winit](https://crates.io/crates/winit) 0.30 backend for the Astrelis windowing abstraction.

This crate implements the traits defined in [`astrelis-window`](../astrelis-window)
using winit as the platform layer. It supports all desktop platforms (Windows,
macOS, Linux/Wayland/X11) and provides `raw-window-handle` integration for GPU
surface creation.

## Quick Start

```rust
use astrelis_window::backend::{AppHandler, EventLoopContext, WindowBackend};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::WindowEvent;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;
use astrelis_window::WindowBuilder;
use astrelis_window_winit::WinitBackend;

struct App { window_id: Option<WindowId> }

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        if state == AppLifecycle::Resumed {
            let attrs = WindowBuilder::new()
                .with_title("Hello Astrelis")
                .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
                .build();
            self.window_id = Some(ctx.create_window(attrs).unwrap());
            ctx.set_control_flow(ControlFlow::Poll);
        }
    }

    fn on_window_event(&mut self, ctx: &mut dyn EventLoopContext, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) { ctx.exit(); }
    }

    fn on_events_cleared(&mut self, _: &mut dyn EventLoopContext) {}
}

fn main() {
    let backend = WinitBackend::new().unwrap();
    backend.run(&mut App { window_id: None }).unwrap();
}
```

## Examples

Run any example with:

```sh
cargo run -p astrelis-window-winit --example <name>
```

| Example | Description |
|---------|-------------|
| `basic_window` | Single window with game-mode polling |
| `resizable_window` | Min/max size constraints, maximize toggle |
| `cursor_grab` | Cursor hide/lock/confine and icon switching |
| `multi_window` | Multiple independent windows, dynamic open/close |

## Platform Capabilities

Not all features are available on all platforms. Use
`ctx.capabilities().supports(Capability::X)` to query at runtime.
See [`astrelis_window::capability`] for the full list.

## License

MIT
