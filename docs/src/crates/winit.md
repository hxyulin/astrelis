# astrelis-winit

The `astrelis-winit` crate provides window creation and event handling abstractions based on `winit`.

## Features

- **App Trait**: Lifecycle management for applications.
- **Windowing**: Easy window creation and configuration.
- **Events**: Unified event system for input and window events.
- **Multi-window**: Support for managing multiple windows.

## Usage

```rust
use astrelis_winit::{app::{App, AppCtx, run_app}, window::WindowDescriptor};

struct MyApp;

impl App for MyApp {
    fn update(&mut self, ctx: &mut AppCtx) {}
    fn render(&mut self, ctx: &mut AppCtx, window: WindowId, events: &mut EventBatch) {}
}

fn main() {
    run_app(|ctx| {
        let window = ctx.create_window(WindowDescriptor::default())?;
        Box::new(MyApp)
    });
}
```

## Modules

### `app`

- `App`: Trait to be implemented by the application.
- `AppCtx`: Context passed to app methods, allowing window creation and exit.
- `run_app`: Entry point to start the event loop.

### `window`

- `Window`: Wrapper around `winit::window::Window`.
- `WindowDescriptor`: Configuration for creating a window.

### `event`

- `Event`: Enum representing all supported events (input, window, etc.).
- `EventBatch`: Collection of events for a frame.
