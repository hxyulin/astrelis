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

## Quick Start

```rust
use astrelis::prelude::*;

fn main() {
    // See examples in individual crates for usage
}
```

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
