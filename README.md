# Astrelis Engine

A high performance 2D and 3D game engine designed for rapid development and deployment of games across multiple platforms written in Rust.
Inspired by Unity and Bevy, with parts of the API being inspired by both engines.

## Features

- **Modular Architecture**: Pick and choose the crates you need
- **GPU-Accelerated Rendering**: Built on WGPU for cross-platform graphics
- **Flexible UI System**: Retained-mode UI with Flexbox/Grid layout (Taffy)
- **High-Quality Text**: Subpixel text rendering with cosmic-text
- **Input Handling**: Unified keyboard, mouse, and gamepad input tracking
- **Asset Management**: Async asset loading with caching
- **Windowing**: Cross-platform window management via winit
- **egui Integration**: Optional immediate-mode UI for tools and debugging

## Crates

| Crate | Description |
|-------|-------------|
| `astrelis` | Main engine crate with feature flags |
| `astrelis-core` | Core utilities, math, logging, profiling |
| `astrelis-render` | WGPU-based rendering framework |
| `astrelis-ui` | Retained-mode UI system |
| `astrelis-text` | Text shaping and rendering |
| `astrelis-input` | Input state management |
| `astrelis-assets` | Asset loading and caching |
| `astrelis-winit` | Window and event management |
| `astrelis-egui` | egui integration |
| `astrelis-audio` | Audio playback (WIP) |
| `astrelis-ecs` | Entity Component System (WIP) |
| `astrelis-scene` | Scene management (WIP) |

## Getting Started

### New to Astrelis?

If you're new to Astrelis, start with our comprehensive getting-started guides:

- **[For Unity Developers](docs/src/guides/getting-started/00-for-unity-developers.md)** - Concept mapping and migration guide
- **[For Bevy Developers](docs/src/guides/getting-started/00-for-bevy-developers.md)** - Understanding the architectural differences
- **[Installation Guide](docs/src/guides/getting-started/01-installation.md)** - Set up your environment
- **[Hello Window Tutorial](docs/src/guides/getting-started/03-hello-window.md)** - Your first Astrelis app

### Quick Start

Create a new project:

```bash
cargo new my_game
cd my_game
```

Add Astrelis to `Cargo.toml`:

```toml
[dependencies]
astrelis-core = { git = "https://github.com/yourusername/astrelis", branch = "main" }
astrelis-winit = { git = "https://github.com/yourusername/astrelis", branch = "main" }
astrelis-render = { git = "https://github.com/yourusername/astrelis", branch = "main" }
glam = "0.29"
```

Create a window with rendering:

```rust
use std::sync::Arc;
use astrelis_core::logging;
use astrelis_render::{Color, GraphicsContext, RenderTarget, RenderableWindow};
use astrelis_winit::{
    WindowId, FrameTime,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::WindowDescriptor,
};

struct MyGame {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
}

impl App for MyGame {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        // Game logic goes here
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window resize
        events.dispatch(|event| {
            use astrelis_winit::event::{Event, HandleStatus};
            if let Event::WindowResized(size) = event {
                self.window.resized(*size);
                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        // Render frame
        let mut frame = self.window.begin_drawing();
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::rgb(0.2, 0.5, 0.6),
            |_pass| {
                // Rendering calls go here
            },
        );
        frame.finish();
    }
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics = GraphicsContext::new_owned_sync_or_panic();
        let window = ctx
            .create_window(&WindowDescriptor {
                title: "My Game".to_string(),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window_id = window.id();
        let renderable = RenderableWindow::new(window, graphics.clone());

        Box::new(MyGame {
            graphics,
            window: renderable,
            window_id,
        })
    });
}
```

Run it:

```bash
cargo run
```

**Next steps**:
- Add UI with [First UI Guide](docs/src/guides/getting-started/05-first-ui.md)
- Learn rendering with [Rendering Fundamentals](docs/src/guides/getting-started/04-rendering-fundamentals.md)
- Explore [examples](#examples) in the repository

## Examples

Run examples from individual crates:

```bash
# UI examples
cargo run -p astrelis-ui --example counter
cargo run -p astrelis-ui --example simple_ui
cargo run -p astrelis-ui --example ui_dashboard

# Render examples  
cargo run -p astrelis-render --example image_blitting
cargo run -p astrelis-render --example sprite_sheet

# egui integration
cargo run -p astrelis-egui --example egui_demo
```

## License

Astrelis Engine is licensed under the MIT License. See the [LICENSE](LICENSE) file for more information.
