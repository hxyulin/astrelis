# Hello Window

In this guide, you'll create your first Astrelis application: a window with a colored background. This minimal example demonstrates the core App lifecycle and rendering pattern.

By the end of this guide, you'll understand:
- How to set up the App trait
- The game loop lifecycle (on_start, update, render, on_exit)
- Basic rendering with FrameContext
- The RAII pattern for automatic resource cleanup

## Prerequisites

- Completed [Installation](01-installation.md)
- Basic understanding of Rust (structs, traits, ownership)

## The Complete Example

Here's the complete code. We'll break it down section by section:

**`src/main.rs`**:
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

// Your game state
struct HelloApp {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
}

// Implement the App trait for game loop
impl App for HelloApp {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        // Game logic goes here (empty for now)
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Only render our window
        if window_id != self.window_id {
            return;
        }

        // Handle window resize events
        events.dispatch(|event| {
            use astrelis_winit::event::{Event, HandleStatus};
            if let Event::WindowResized(size) = event {
                self.window.resized(*size);
                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        // Begin the frame
        let mut frame = self.window.begin_drawing();

        // Clear to a nice teal color and render (nothing to render yet)
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::rgb(0.2, 0.5, 0.6),
            |_pass| {
                // Rendering calls would go here
            },
        );

        // Finish automatically submits and presents
        frame.finish();
    }
}

fn main() {
    // Initialize logging (important for debugging)
    logging::init();

    // Run the app
    run_app(|ctx| {
        // Create graphics context (GPU access)
        let graphics = GraphicsContext::new_owned_sync_or_panic();

        // Create a window
        let descriptor = WindowDescriptor {
            title: "Hello, Astrelis!".to_string(),
            ..Default::default()
        };

        let window = ctx
            .create_window(&descriptor)
            .expect("Failed to create window");

        let window_id = window.id();

        // Wrap window with rendering capabilities
        let renderable = RenderableWindow::new(window, graphics.clone());

        // Return your app
        Box::new(HelloApp {
            graphics,
            window: renderable,
            window_id,
        })
    });
}
```

## Breaking It Down

### 1. Imports

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
```

**What's being imported?**:
- **`Arc`**: Shared ownership for GraphicsContext
- **`logging`**: Initialize tracing for debug output
- **`GraphicsContext`**: GPU device and queue
- **`RenderableWindow`**: Window with rendering capabilities
- **`App` trait**: Your game loop interface
- **`run_app`**: Entry point that starts the event loop

### 2. App State

```rust
struct HelloApp {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
}
```

Your app state holds everything your game needs:
- **`graphics`**: Shared GPU context (`Arc` allows cloning without copying)
- **`window`**: The renderable window
- **`window_id`**: Used to identify which window to render (multi-window support)

**Why `Arc`?**: `GraphicsContext` is expensive to create. `Arc` (Atomic Reference Count) allows multiple owners with automatic cleanup when the last owner drops it.

### 3. The App Trait

The `App` trait defines your game loop lifecycle:

```rust
impl App for HelloApp {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        // Called every frame for game logic
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Called every frame for rendering
    }
}
```

**Lifecycle methods** (in order):
1. **`on_start()`** (optional): Called once on startup
2. **`update()`**: Called every frame for game logic
3. **`render()`**: Called every frame after update for rendering
4. **`on_exit()`** (optional): Called once on shutdown

**Important**: `render()` is called **per window**. If you have multiple windows, it's called once for each.

### 4. The Render Method

```rust
fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    // 1. Check if this is our window
    if window_id != self.window_id {
        return;
    }

    // 2. Handle resize events
    events.dispatch(|event| {
        use astrelis_winit::event::{Event, HandleStatus};
        if let Event::WindowResized(size) = event {
            self.window.resized(*size);
            HandleStatus::consumed()  // Event was handled
        } else {
            HandleStatus::ignored()   // Event not for us
        }
    });

    // 3. Begin drawing
    let mut frame = self.window.begin_drawing();

    // 4. Clear and render
    frame.clear_and_render(
        RenderTarget::Surface,     // Render to the window surface
        Color::rgb(0.2, 0.5, 0.6), // Clear color (teal)
        |_pass| {
            // Render pass is active here
            // We'll add rendering code in future guides
        },
    ); // Render pass automatically ends here

    // 5. Finish the frame
    frame.finish(); // Submits commands and presents to screen
}
```

#### The RAII Pattern

Astrelis uses **RAII (Resource Acquisition Is Initialization)** for automatic cleanup:

1. **`begin_drawing()`** acquires the surface texture and creates a command encoder
2. **`clear_and_render()`** creates a render pass, runs your closure, then automatically drops the pass
3. **`finish()`** submits GPU commands and presents the frame

**Why closures?**: The closure in `clear_and_render()` ensures the render pass is dropped before `finish()` is called. This is required by WGPU.

**Without closures** (don't do this - for illustration only):
```rust
// Bad: Manual pass management
let mut pass = RenderPassBuilder::new().build(&mut frame);
// ... render stuff ...
drop(pass);  // Must manually drop before finish()
frame.finish();
```

**With closures** (recommended):
```rust
// Good: Automatic scoping
frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
    // ... render stuff ...
}); // pass automatically dropped here
frame.finish();
```

### 5. Main Function

```rust
fn main() {
    logging::init();  // Set up tracing

    run_app(|ctx| {
        // Create GPU context
        let graphics = GraphicsContext::new_owned_sync_or_panic();

        // Create window
        let window = ctx
            .create_window(&WindowDescriptor {
                title: "Hello, Astrelis!".to_string(),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window_id = window.id();

        // Wrap with rendering
        let renderable = RenderableWindow::new(window, graphics.clone());

        // Return app instance
        Box::new(HelloApp {
            graphics,
            window: renderable,
            window_id,
        })
    });
}
```

**`run_app()`** starts the event loop and never returns (until app exits).

**The closure** receives `ctx: &AppCtx` for creating windows and returns `Box<dyn App>` (your app instance).

## Running Your App

Build and run:

```bash
cargo run
```

You should see a window with a teal background!

**Expected output**:
```
[INFO] GraphicsContext initialized
[INFO] Backend: Vulkan (or Metal on macOS, DX12 on Windows)
[INFO] Window created: "Hello, Astrelis!" (800x600)
```

## Customizing the Window

### Window Size

```rust
let descriptor = WindowDescriptor {
    title: "My Game".to_string(),
    size: Some(WinitPhysicalSize::new(1280.0, 720.0)),
    ..Default::default()
};
```

### Fullscreen

```rust
use astrelis_winit::window::Fullscreen;

let descriptor = WindowDescriptor {
    title: "Fullscreen Game".to_string(),
    fullscreen: Some(Fullscreen::Borderless),
    ..Default::default()
};
```

### Resizable

```rust
let descriptor = WindowDescriptor {
    title: "Fixed Size Window".to_string(),
    resizable: false,  // Prevent resizing
    ..Default::default()
};
```

### VSync

```rust
use astrelis_render::WindowContextDescriptor;
use wgpu::PresentMode;

let renderable = RenderableWindow::new_with_descriptor(
    window,
    graphics.clone(),
    WindowContextDescriptor {
        present_mode: PresentMode::Immediate,  // Disable VSync
        ..Default::default()
    },
);
```

**Present modes**:
- `Fifo` (default): VSync enabled, capped at monitor refresh rate
- `Immediate`: No VSync, lowest latency but may tear
- `Mailbox`: Triple buffering if supported

## Changing the Clear Color

The clear color is specified in `clear_and_render()`:

```rust
// RGB values from 0.0 to 1.0
frame.clear_and_render(
    RenderTarget::Surface,
    Color::rgb(1.0, 0.5, 0.0),  // Orange
    |_pass| {},
);

// Or use named colors (if available)
Color::from_rgb(0.1, 0.1, 0.1)  // Dark gray

// With alpha channel
Color::rgba(1.0, 0.0, 0.0, 0.5)  // Semi-transparent red
```

## Adding the Update Loop

Right now, `update()` is empty. Let's add frame counting:

```rust
struct HelloApp {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    frame_count: u64,  // Add this
}

impl App for HelloApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        self.frame_count += 1;

        // Print every 60 frames (~1 second at 60 FPS)
        if self.frame_count % 60 == 0 {
            tracing::info!("Frame: {}, FPS: {:.1}", self.frame_count, 1.0 / time.delta.as_secs_f32());
        }
    }

    // ... render stays the same ...
}
```

Don't forget to initialize `frame_count: 0` in `main()`.

## Common Issues

### Black Screen

**Problem**: Window appears but is black.

**Causes**:
1. Forgot to call `frame.finish()` - Commands never submitted
2. Render pass not dropped before `finish()` - Use `clear_and_render()` closure pattern
3. Surface lost (minimize/resize) - Handle `WindowResized` event

**Fix**: Ensure you're using the closure pattern and handling resize events.

### Window Not Responding

**Problem**: Window opens but is frozen.

**Cause**: `update()` or `render()` is blocking (infinite loop, expensive operation).

**Fix**: Keep `update()` and `render()` fast. Move expensive work to background threads.

### Crash on Resize

**Problem**: App crashes when resizing window.

**Cause**: Not handling `WindowResized` event.

**Fix**: Always call `self.window.resized(size)` in the resize event handler (see example above).

### "Surface configuration is invalid"

**Problem**: Error during `begin_drawing()`.

**Cause**: Surface size is zero (window minimized) or GPU driver issue.

**Fix**:
```rust
fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    if window_id != self.window_id {
        return;
    }

    // Check window is not minimized
    let size = self.window.size();
    if size.width == 0 || size.height == 0 {
        return;  // Skip rendering when minimized
    }

    let mut frame = self.window.begin_drawing();
    // ... rest of rendering ...
}
```

## Understanding Frame Timing

The `FrameTime` parameter in `update()` provides timing information:

```rust
fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
    let delta = time.delta.as_secs_f32();  // Time since last frame (e.g., 0.016 for 60 FPS)
    let fps = 1.0 / delta;

    // Use delta for time-based movement (frame-rate independent)
    self.player_position.x += self.player_speed * delta;
}
```

**Always use delta time** for movement and animations to ensure consistent speed across different frame rates.

## Next Steps

You've created your first Astrelis app! Next, learn about:

1. **[Rendering Fundamentals](04-rendering-fundamentals.md)** - Draw shapes, sprites, and custom content
2. **[First UI](05-first-ui.md)** - Add buttons, text, and interactive UI
3. **[Asset Loading](../asset-system/loading-assets.md)** - Load textures and fonts

## Complete Code

Here's the final code with frame counting:

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

struct HelloApp {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    frame_count: u64,
}

impl App for HelloApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        self.frame_count += 1;
        if self.frame_count % 60 == 0 {
            let fps = 1.0 / time.delta.as_secs_f32();
            tracing::info!("Frame: {}, FPS: {:.1}", self.frame_count, fps);
        }
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        events.dispatch(|event| {
            use astrelis_winit::event::{Event, HandleStatus};
            if let Event::WindowResized(size) = event {
                self.window.resized(*size);
                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        let mut frame = self.window.begin_drawing();
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::rgb(0.2, 0.5, 0.6),
            |_pass| {},
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
                title: "Hello, Astrelis!".to_string(),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window_id = window.id();
        let renderable = RenderableWindow::new(window, graphics.clone());

        Box::new(HelloApp {
            graphics,
            window: renderable,
            window_id,
            frame_count: 0,
        })
    });
}
```

Try changing the clear color, window size, or adding more frame statistics. Experiment and have fun!
