# Installation

This guide will walk you through setting up Astrelis on your system. By the end, you'll have a working Rust environment and your first Astrelis project ready to build.

## Prerequisites

### 1. Rust Stable

Astrelis requires **Rust stable**. The exact version is specified in `rust-toolchain.toml`, but any recent stable version should work.

**Install Rust** via [rustup](https://rustup.rs/):

```bash
# Linux/macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows
# Download and run rustup-init.exe from https://rustup.rs/
```

**Verify installation**:

```bash
rustc --version
cargo --version
```

You should see output like:
```
rustc 1.83.0 (90b35a623 2024-11-26)
cargo 1.83.0 (5ffbef321 2024-10-29)
```

**Update Rust** to the latest stable version:

```bash
rustup update stable
```

### 2. Platform-Specific Dependencies

Astrelis uses WGPU for GPU rendering and winit for windowing. You'll need platform-specific graphics and window system libraries.

#### Linux

**Ubuntu/Debian**:
```bash
sudo apt install build-essential libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
    libasound2-dev libpulse-dev libgl1-mesa-dev libxcb-render0-dev libxcb-shape0-dev \
    libxcb-xfixes0-dev libxkbcommon-dev pkg-config
```

**Fedora**:
```bash
sudo dnf install gcc libX11-devel libXcursor-devel libXrandr-devel libXi-devel \
    alsa-lib-devel pulseaudio-libs-devel mesa-libGL-devel libxcb-devel \
    libxkbcommon-devel
```

**Arch Linux**:
```bash
sudo pacman -S base-devel libx11 libxcursor libxrandr libxi alsa-lib pulseaudio \
    mesa libxcb libxkbcommon
```

#### macOS

**Install Xcode Command Line Tools**:
```bash
xcode-select --install
```

macOS includes Metal and CoreGraphics by default, so no additional dependencies are needed.

#### Windows

**Install Visual Studio** (2019 or later) with "Desktop development with C++" workload:
- Download from [visualstudio.microsoft.com](https://visualstudio.microsoft.com/)
- Or install just the build tools: [Visual C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)

Alternatively, use **MinGW-w64** with MSYS2:
```bash
# In MSYS2 terminal
pacman -S mingw-w64-x86_64-toolchain
```

### 3. (Optional) GPU Drivers

Ensure your **GPU drivers are up to date**:
- **NVIDIA**: Download from [nvidia.com/drivers](https://www.nvidia.com/drivers)
- **AMD**: Download from [amd.com/support](https://www.amd.com/support)
- **Intel**: Usually updated via OS updates

Astrelis uses WGPU, which supports:
- **Vulkan** (Linux, Windows, Android)
- **Metal** (macOS, iOS)
- **DirectX 12** (Windows)
- **WebGPU** (Web via wasm)

WGPU will automatically select the best backend for your platform.

## Creating Your First Project

### Option 1: Using Cargo

Create a new Rust project with Cargo:

```bash
cargo new my_game
cd my_game
```

### Option 2: Workspace Project (Recommended for Larger Games)

For larger projects, use a Cargo workspace to organize multiple crates:

```bash
mkdir my_game
cd my_game
cargo new --bin game
cargo new --lib game_logic
```

Create a `Cargo.toml` at the workspace root:

```toml
[workspace]
members = ["game", "game_logic"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
authors = ["Your Name <you@example.com>"]

[workspace.dependencies]
# Common dependencies can be defined here
glam = "0.29"
```

## Adding Astrelis Dependencies

Edit your `Cargo.toml` to add Astrelis crates. Astrelis is **modular** - you only include what you need.

### Minimal Setup (Window Only)

For a basic window with rendering:

```toml
[package]
name = "my_game"
version = "0.1.0"
edition = "2024"

[dependencies]
# Core functionality
astrelis-core = { git = "https://github.com/yourusername/astrelis", branch = "main" }
# Windowing and event loop
astrelis-winit = { git = "https://github.com/yourusername/astrelis", branch = "main" }
# GPU rendering
astrelis-render = { git = "https://github.com/yourusername/astrelis", branch = "main" }

# Math and utilities
glam = "0.29"
```

### With UI and Assets

For games with UI and asset loading:

```toml
[dependencies]
# Core
astrelis-core = { git = "https://github.com/yourusername/astrelis", branch = "main" }
astrelis-winit = { git = "https://github.com/yourusername/astrelis", branch = "main" }
astrelis-render = { git = "https://github.com/yourusername/astrelis", branch = "main" }

# UI system
astrelis-ui = { git = "https://github.com/yourusername/astrelis", branch = "main" }
# Text rendering
astrelis-text = { git = "https://github.com/yourusername/astrelis", branch = "main" }
# Asset loading
astrelis-assets = { git = "https://github.com/yourusername/astrelis", branch = "main" }

# Utilities
glam = "0.29"
```

### Full Engine with Plugin System

For the full engine experience:

```toml
[dependencies]
# Main engine facade
astrelis = { git = "https://github.com/yourusername/astrelis", branch = "main" }
# Individual crates if needed
astrelis-core = { git = "https://github.com/yourusername/astrelis", branch = "main" }
astrelis-winit = { git = "https://github.com/yourusername/astrelis", branch = "main" }
astrelis-render = { git = "https://github.com/yourusername/astrelis", branch = "main" }
astrelis-ui = { git = "https://github.com/yourusername/astrelis", branch = "main" }
astrelis-assets = { git = "https://github.com/yourusername/astrelis", branch = "main" }

glam = "0.29"
```

### Important Dependency Version Pinning

Astrelis uses specific versions of external dependencies. Ensure compatibility:

```toml
[dependencies.wgpu]
version = "27.0.1"

[dependencies.winit]
version = "0.30.12"
```

These versions are tested and known to work with Astrelis. Using different versions may cause issues.

## Workspace Dependencies (Advanced)

For workspace projects, define dependencies once at the workspace level:

**Root `Cargo.toml`**:
```toml
[workspace]
members = ["game", "game_logic"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
# Astrelis crates
astrelis-core = { git = "https://github.com/yourusername/astrelis", branch = "main" }
astrelis-winit = { git = "https://github.com/yourusername/astrelis", branch = "main" }
astrelis-render = { git = "https://github.com/yourusername/astrelis", branch = "main" }
astrelis-ui = { git = "https://github.com/yourusername/astrelis", branch = "main" }

# External dependencies
glam = "0.29"
wgpu = "27.0.1"
winit = "0.30.12"
```

**Crate `Cargo.toml`**:
```toml
[package]
name = "game"
version.workspace = true
edition.workspace = true

[dependencies]
# Reference workspace dependencies
astrelis-core.workspace = true
astrelis-winit.workspace = true
astrelis-render.workspace = true
glam.workspace = true
```

This ensures all crates use the same versions and simplifies updates.

## Verifying Your Installation

Create a simple test to verify everything works:

**`src/main.rs`**:
```rust
use std::sync::Arc;
use astrelis_core::logging::init as init_logging;
use astrelis_winit::{run_app, App, AppCtx, WindowDescriptor};
use astrelis_render::{GraphicsContext, RenderableWindow, RenderTarget, Color};
use glam::Vec2;

struct TestApp {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
}

impl App for TestApp {
    fn update(&mut self, _ctx: &mut AppCtx) {
        // Nothing to update yet
    }

    fn render(&mut self, _ctx: &mut AppCtx, _window_id: winit::window::WindowId, _events: &mut astrelis_winit::EventBatch) {
        // Begin frame
        let mut frame = self.window.begin_drawing();

        // Clear to a nice blue color
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb(0.2, 0.4, 0.8),
            |_pass| {
                // Nothing to render yet
            },
        );

        // Finish frame
        frame.finish();
    }
}

fn main() {
    // Initialize logging
    init_logging();

    // Run the app
    run_app(|ctx| {
        // Create graphics context
        let graphics = GraphicsContext::new_owned_sync();

        // Create window
        let descriptor = WindowDescriptor {
            title: "Astrelis Test".to_string(),
            size: Vec2::new(800.0, 600.0),
            ..Default::default()
        };
        let window = ctx.create_window(&descriptor).expect("Failed to create window");

        // Create renderable window
        let renderable = RenderableWindow::new(window, graphics.clone());

        Box::new(TestApp {
            graphics,
            window: renderable,
        })
    });
}
```

**Build and run**:

```bash
cargo build
cargo run
```

You should see a window with a blue background. If so, **congratulations!** Astrelis is installed correctly.

**Common Build Errors**:

- **"could not find `wgpu`"**: Add `wgpu = "27.0.1"` to your `[dependencies]`
- **"could not find `winit`"**: Add `winit = "0.30.12"` to your `[dependencies]`
- **Linker errors on Linux**: Install platform dependencies (see above)
- **"surface configuration is invalid"**: GPU driver issue - update drivers

## Feature Flags

Astrelis crates may expose optional features. Check each crate's `Cargo.toml` for available features.

Example with features:

```toml
[dependencies]
astrelis-render = { git = "...", features = ["trace"] }  # Enable GPU trace
astrelis-ui = { git = "...", features = ["inspector"] }  # Enable UI inspector
```

Common features:
- `trace`: Enable tracing for debugging (GPU capture)
- `inspector`: Enable debug UI overlays
- `profiling`: Enable puffin profiling
- `serialize`: Enable serde serialization support

## Project Structure

A typical Astrelis project structure:

```
my_game/
├── Cargo.toml
├── src/
│   ├── main.rs           # Entry point
│   ├── app.rs            # App trait implementation
│   ├── rendering/        # Custom rendering code
│   ├── ui/               # UI screens and widgets
│   └── game/             # Game logic
├── assets/               # Game assets
│   ├── textures/
│   ├── fonts/
│   └── shaders/
└── examples/             # Example code
```

## Next Steps

Now that Astrelis is installed, you're ready to build your first app:

1. **[Architecture Overview](02-architecture-overview.md)** - Understand Astrelis's design
2. **[Hello Window](03-hello-window.md)** - Create your first app
3. **[Rendering Fundamentals](04-rendering-fundamentals.md)** - Learn the rendering system
4. **[First UI](05-first-ui.md)** - Build interactive UI

## Troubleshooting

### Build is Slow

First build can take 10-15 minutes as Cargo compiles all dependencies. Subsequent builds are much faster (incremental compilation).

**Speed up builds**:

Add to `.cargo/config.toml`:
```toml
[build]
# Use mold linker on Linux (install: cargo install mold)
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

# Or use lld on any platform
# rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

### Out of Memory During Build

Reduce parallel jobs:

```bash
cargo build -j 2  # Use only 2 parallel jobs
```

Or set in `.cargo/config.toml`:
```toml
[build]
jobs = 2
```

### wgpu Validation Errors

Add to `src/main.rs`:
```rust
std::env::set_var("RUST_LOG", "warn");  # Reduce log noise
```

Or disable wgpu validation (not recommended):
```rust
// When creating GraphicsContext
let context = GraphicsContext::new_with_backend(Backend::preferred(), false);  // false = no validation
```

### Window Not Appearing (Linux Wayland)

Try forcing X11:

```bash
WAYLAND_DISPLAY= cargo run  # Force X11
```

## Getting Help

If you encounter issues:

1. Check the [Troubleshooting Guide](../troubleshooting.md)
2. Search [GitHub Issues](https://github.com/yourusername/astrelis/issues)
3. Ask in [Discussions](https://github.com/yourusername/astrelis/discussions)
4. Join the [Discord](https://discord.gg/astrelis) (if available)

You're now ready to build with Astrelis!
