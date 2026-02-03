# Troubleshooting

This comprehensive troubleshooting guide covers common issues, errors, and solutions when working with Astrelis. Use this as a quick reference when encountering problems.

## Table of Contents

- [Compilation Errors](#compilation-errors)
- [Runtime Errors](#runtime-errors)
- [Rendering Issues](#rendering-issues)
- [UI System Problems](#ui-system-problems)
- [Asset Loading Issues](#asset-loading-issues)
- [Performance Problems](#performance-problems)
- [Platform-Specific Issues](#platform-specific-issues)
- [Common Questions (FAQ)](#common-questions-faq)

---

## Compilation Errors

### Error: "no method named `build` found"

**Full error:**
```
error[E0599]: no method named `build` found for struct `EngineBuilder`
```

**Cause:** Using wrong engine building pattern or missing plugin.

**Solution:**
```rust
// Ensure you call build() at the end
let engine = Engine::builder()
    .add_plugin(MyPlugin)
    .build(); // Don't forget this!
```

### Error: "mismatched types: expected `Arc<GraphicsContext>`"

**Full error:**
```
error[E0308]: mismatched types
expected struct `Arc<GraphicsContext>`
found struct `GraphicsContext`
```

**Cause:** Not wrapping GraphicsContext in Arc.

**Solution:**
```rust
// Create with Arc
let graphics = Arc::new(GraphicsContext::new_owned_sync());

// Or use the helper
let graphics = GraphicsContext::new_owned_sync(); // Returns Arc automatically
```

### Error: "cannot find macro `profile_function` in this scope"

**Cause:** Missing profiling feature or import.

**Solution:**
```rust
// Add to Cargo.toml
[dependencies]
astrelis-core = { version = "0.1", features = ["profiling"] }

// Import in code
use astrelis_core::profiling::profile_function;
```

### Error: "trait bound `X: Plugin` is not satisfied"

**Cause:** Type doesn't implement Plugin trait.

**Solution:**
```rust
use astrelis::Plugin;

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, engine: &mut Engine) {
        // Plugin implementation
    }
}
```

### Error: "WGPU version mismatch"

**Full error:**
```
error: the trait bound `wgpu::Device: From<&wgpu::Adapter>` is not satisfied
```

**Cause:** Using incompatible WGPU version.

**Solution:**
```toml
# In Cargo.toml, pin to correct version
[dependencies]
wgpu = "27.0.1"  # Match Astrelis version
```

### Error: "winit version mismatch"

**Cause:** Incompatible winit version.

**Solution:**
```toml
[dependencies]
winit = "0.30.12"  # Match Astrelis version
```

---

## Runtime Errors

### Panic: "called `Result::unwrap()` on an `Err` value: SurfaceLost"

**Cause:** Surface lost (window minimized, resized, or moved) and not handled.

**Solution:**
```rust
impl App for MyGame {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        match self.renderable.begin_drawing() {
            Ok(frame) => {
                // Render normally
            }
            Err(GraphicsError::SurfaceLost) => {
                warn!("Surface lost, recreating...");
                if let Err(e) = self.renderable.recreate_surface() {
                    error!("Failed to recreate surface: {:?}", e);
                }
                return;
            }
            Err(e) => {
                error!("Graphics error: {:?}", e);
            }
        }
    }
}
```

**Related:** [Error Handling Guide](advanced/error-handling.md)

### Panic: "render pass not dropped before finish()"

**Cause:** Render pass still in scope when `frame.finish()` called.

**Solution:**
```rust
// BAD: Pass not dropped
let mut frame = renderable.begin_drawing();
let pass = RenderPassBuilder::new().build(&mut frame);
// pass is still alive!
frame.finish(); // PANIC

// GOOD: Use clear_and_render() for automatic scoping
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        ui.render(pass.wgpu_pass());
    }, // Pass drops here
);
frame.finish(); // OK

// GOOD: Manual drop with explicit scope
{
    let pass = RenderPassBuilder::new().build(&mut frame);
    ui.render(pass.wgpu_pass());
} // Pass drops here
frame.finish(); // OK
```

**Related:** [Rendering Fundamentals](getting-started/04-rendering-fundamentals.md)

### Panic: "no entry found for key" (Resources)

**Cause:** Accessing resource that wasn't registered.

**Solution:**
```rust
// Check if resource exists
if let Some(assets) = engine.get::<Arc<AssetServer>>() {
    // Use assets
} else {
    error!("AssetServer not registered!");
}

// Or register the resource
engine.insert_resource(Arc::new(AssetServer::new(path)));
```

### Error: "Device lost"

**Cause:** GPU driver crashed or device removed.

**Solution:**
```rust
// Device lost is usually not recoverable
// Show error and request exit
self.show_fatal_error("GPU device lost. Please restart.");
ctx.request_exit();
```

**Related:** [Error Handling Guide](advanced/error-handling.md)

---

## Rendering Issues

### Black Screen (Nothing Rendering)

**Possible causes and solutions:**

**1. Forgot to call `frame.finish()`**
```rust
// WRONG
let mut frame = renderable.begin_drawing();
frame.clear_and_render(...);
// Missing frame.finish()!

// CORRECT
let mut frame = renderable.begin_drawing();
frame.clear_and_render(...);
frame.finish(); // Don't forget!
```

**2. Clear color same as background**
```rust
// Check clear color
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK, // Is this visible?
    |pass| { /* ... */ },
);
```

**3. UI not initialized**
```rust
// Ensure UI is built
self.ui.build(|root| {
    root.text("Hello").build();
});
```

**4. Window minimized**
```
// Handle surface lost errors (see Runtime Errors section)
```

### Flickering or Tearing

**Cause:** VSync not enabled or misconfigured.

**Solution:**
```rust
// Enable VSync in surface configuration
let config = wgpu::SurfaceConfiguration {
    present_mode: wgpu::PresentMode::Fifo, // VSync
    // ... other fields
};
```

### Textures Not Displaying

**Possible causes:**

**1. Texture not loaded**
```rust
// Check handle validity
if let Some(texture) = assets.get(&texture_handle) {
    // Use texture
} else {
    warn!("Texture not loaded yet");
}
```

**2. Wrong texture coordinates**
```rust
// Verify UV coordinates are in [0, 1] range
let uv = Vec2::new(
    x.clamp(0.0, 1.0),
    y.clamp(0.0, 1.0),
);
```

**3. Texture not bound to pipeline**
```rust
// Ensure bind group is set
render_pass.set_bind_group(0, &texture_bind_group, &[]);
```

### Shader Compilation Errors

**Cause:** WGSL syntax error or unsupported feature.

**Solution:**
```rust
// Check shader source for errors
let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("My Shader"),
    source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
});

// WGPU will panic with error message
// Fix syntax based on error
```

**Common WGSL mistakes:**
```wgsl
// WRONG: Missing return type
fn my_function() {
    return 42.0;
}

// CORRECT
fn my_function() -> f32 {
    return 42.0;
}

// WRONG: Using undefined variable
let color = undefined_var;

// CORRECT: Define variable first
let color = vec4<f32>(1.0, 0.0, 0.0, 1.0);
```

---

## UI System Problems

### Widget Not Visible

**Possible causes:**

**1. Forgot to build UI**
```rust
// Must call build() at least once
self.ui.build(|root| {
    root.text("Hello").build();
});
```

**2. Widget outside visible area**
```rust
// Check position and size
root.text("Hello")
    .position(Position::absolute(100.0, 100.0)) // Is this on screen?
    .build();
```

**3. Z-index issue**
```rust
// Ensure widget not behind others
root.text("Hello")
    .z_index(10) // Higher z-index renders on top
    .build();
```

**4. Text color same as background**
```rust
// Check text color
root.text("Hello")
    .color(Color::WHITE) // Visible on dark background?
    .build();
```

### Button Not Responding to Clicks

**Possible causes:**

**1. No click handler registered**
```rust
// Must add on_click handler
root.button("Click Me")
    .on_click(|| {
        println!("Clicked!");
    })
    .build();
```

**2. UI consuming events**
```rust
// Ensure UI.handle_events() is called
impl App for MyGame {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        self.ui.handle_events(events); // Don't forget this!

        // Render...
    }
}
```

**3. Button behind other widget**
```rust
// Check z-index and layout order
// Later widgets render on top
```

### Text Not Updating

**Cause:** Using wrong update method or not marking dirty.

**Solution:**
```rust
// Use update_text() for fast updates
self.ui.update_text("my_label", "New text");

// NOT full rebuild
self.ui.build(|root| {
    root.text("New text").id("my_label").build(); // Too slow!
});
```

**Related:** [Incremental Updates Guide](getting-started/06-incremental-updates.md)

### Layout Not Correct

**Possible causes:**

**1. Missing size constraints**
```rust
// Add explicit width/height
root.column()
    .width(Length::px(400))
    .height(Length::fill())
    .build();
```

**2. Wrong flex direction**
```rust
// Check flex_direction
root.row() // Horizontal
    .children(...)
    .build();

root.column() // Vertical
    .children(...)
    .build();
```

**3. Conflicting constraints**
```rust
// Don't use both padding and explicit size without accounting for it
root.container()
    .width(Length::px(100))
    .padding(Length::px(20)) // Total width is 140px!
    .build();
```

**Related:** [Layout Deep Dive](ui/layout-deep-dive.md)

### Slow UI Performance

**Cause:** Full rebuild every frame or inefficient rendering.

**Solution:**
```rust
// Use incremental updates
self.ui.update_text("fps", &format!("FPS: {}", fps)); // <1ms

// NOT full rebuild
self.ui.build(|root| {
    root.text(&format!("FPS: {}", fps)).build(); // ~20ms
});
```

**Related:** [UI Performance Optimization](ui/performance-optimization.md)

---

## Asset Loading Issues

### Asset Not Found

**Cause:** Wrong path or file doesn't exist.

**Solution:**
```rust
// Check if file exists
if !std::path::Path::new("assets/texture.png").exists() {
    error!("Asset file not found!");
}

// Use correct path
let handle = assets.load::<Texture>("assets/texture.png")?;
// or relative to asset directory
let handle = assets.load::<Texture>("texture.png")?;
```

### Asset Loading Hangs

**Cause:** Synchronous loading or waiting for async result incorrectly.

**Solution:**
```rust
// Use async loading
let handle = assets.load::<Texture>("texture.png").await?;

// Or load synchronously on background thread
let handle = task_pool.spawn(async {
    assets.load::<Texture>("texture.png").await
});
```

**Related:** [Async Tasks Guide](advanced/async-tasks.md)

### Hot Reload Not Working

**Possible causes:**

**1. File watcher not set up**
```rust
// Initialize file watcher
let watcher = notify::recommended_watcher(|event| {
    // Handle file changes
})?;

watcher.watch("assets/", RecursiveMode::Recursive)?;
```

**2. Not checking AssetEvent**
```rust
// Check for reload events
for event in assets.drain_events() {
    if let AssetEvent::Modified { path } = event {
        info!("Asset reloaded: {}", path);
        // Handle reload
    }
}
```

**Related:** [Hot Reload Guide](asset-system/hot-reload.md)

### Wrong Asset Type

**Cause:** Loading asset with wrong type parameter.

**Solution:**
```rust
// Ensure type matches file format
let image = assets.load::<Texture>("image.png")?; // Correct
let image = assets.load::<Font>("image.png")?;    // Wrong!
```

---

## Performance Problems

### Low Frame Rate

**Diagnosis steps:**

**1. Profile with Puffin**
```rust
init_profiling(ProfilingBackend::PuffinHttp);
profile_function!();

// Open http://127.0.0.1:8585
```

**2. Check frame time**
```rust
if time.delta.as_secs_f32() > 0.016 {
    warn!("Slow frame: {:.2}ms", time.delta.as_secs_f32() * 1000.0);
}
```

**3. Common causes:**
- Full UI rebuild every frame (use incremental updates)
- Text shaping every frame (use update_text())
- Too many draw calls (use instancing)
- Expensive update logic (profile with puffin)

**Related:** [Performance Tuning Guide](advanced/performance-tuning.md)

### High Memory Usage

**Diagnosis:**

**1. Check allocations**
```rust
// Track memory usage (see Performance guide)
let allocated = memory_tracker.allocated_bytes();
info!("Memory: {:.2} MB", allocated as f32 / 1_000_000.0);
```

**2. Common causes:**
- Leaked resources (forgot to drop)
- Too many textures loaded
- Large buffers not released
- Circular Arc references

**Solution:**
```rust
// Implement Drop for cleanup
impl Drop for MyResource {
    fn drop(&mut self) {
        info!("Resource cleaned up");
    }
}

// Break Arc cycles with Weak
use std::sync::Weak;
let weak_ref: Weak<T> = Arc::downgrade(&arc_ref);
```

### Stuttering/Frame Spikes

**Cause:** Irregular expensive operations.

**Solution:**
```rust
// Spread work across frames
let mut processor = ChunkedProcessor::new(items, 100);

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        // Process 100 items per frame
        processor.process_chunk(|item| {
            expensive_operation(item);
        });
    }
}
```

**Related:** [Async Tasks Guide](advanced/async-tasks.md)

---

## Platform-Specific Issues

### macOS: Metal Validation Errors

**Error:**
```
[Metal] Validation layer error: ...
```

**Cause:** Metal API misuse or unsupported feature.

**Solution:**
```rust
// Ensure using Metal-compatible format
let format = wgpu::TextureFormat::Bgra8UnormSrgb; // Metal compatible

// Check Metal validation layers
// Set environment variable:
// METAL_DEVICE_WRAPPER_TYPE=1
```

### Windows: DX12 vs Vulkan

**Issue:** Performance differences or compatibility problems.

**Solution:**
```rust
// Force specific backend
let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
    backends: wgpu::Backends::DX12, // or VULKAN
    ..Default::default()
});
```

### Linux: X11 vs Wayland

**Issue:** Window creation fails or input not working.

**Solution:**
```bash
# Force X11
WINIT_UNIX_BACKEND=x11 cargo run

# Force Wayland
WINIT_UNIX_BACKEND=wayland cargo run
```

### Web (WASM): Compilation Errors

**Cause:** Using unsupported features or missing wasm-bindgen.

**Solution:**
```toml
# In Cargo.toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
web-sys = "0.3"
```

```bash
# Build for web
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen --out-dir out --target web target/wasm32-unknown-unknown/release/my_game.wasm
```

---

## Common Questions (FAQ)

### Q: How do I create a window?

**A:** Use the `App` trait and `run_app()`:

```rust
use astrelis_winit::{run_app, App, AppCtx};

fn main() {
    run_app(|ctx| {
        let window = ctx.create_window(descriptor)?;
        Box::new(MyApp { window })
    });
}
```

**Related:** [Hello Window Guide](getting-started/03-hello-window.md)

### Q: How do I draw a rectangle?

**A:** Use UiSystem with a container widget:

```rust
ui.build(|root| {
    root.container()
        .width(Length::px(100))
        .height(Length::px(100))
        .background_color(Color::RED)
        .build();
});
```

**Related:** [First UI Guide](getting-started/05-first-ui.md)

### Q: How do I handle keyboard input?

**A:** Use event dispatch or InputState:

```rust
// Event-based
events.dispatch(|event| {
    if let Event::KeyPressed { key, .. } = event {
        match key {
            VirtualKeyCode::Space => {
                player.jump();
                HandleStatus::consumed()
            }
            _ => HandleStatus::ignored()
        }
    } else {
        HandleStatus::ignored()
    }
});

// State-based
if let Some(input) = engine.get::<Arc<InputState>>() {
    if input.is_key_pressed(VirtualKeyCode::W) {
        player.move_forward();
    }
}
```

**Related:** [Input Handling Guide](advanced/input-handling.md)

### Q: How do I load an image?

**A:** Use AssetServer:

```rust
let assets = engine.get::<Arc<AssetServer>>().unwrap();
let texture_handle = assets.load::<Texture>("texture.png")?;

// Later, get the texture
if let Some(texture) = assets.get(&texture_handle) {
    // Use texture
}
```

**Related:** [Loading Assets Guide](asset-system/loading-assets.md)

### Q: How do I create a custom widget?

**A:** Implement the Widget trait:

```rust
pub struct MyWidget {
    // ... fields
}

impl Widget for MyWidget {
    fn build(&mut self, builder: &mut WidgetBuilder) {
        builder
            .width(Length::px(100))
            .height(Length::px(100))
            .background_color(Color::BLUE);
    }
}
```

**Related:** [Custom Widgets Guide](ui/custom-widgets.md)

### Q: How do I write a custom shader?

**A:** Create WGSL shader and ShaderModule:

```wgsl
// shader.wgsl
@vertex
fn vs_main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(position, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0); // Red
}
```

```rust
let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("Custom Shader"),
    source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
});
```

**Related:** [Custom Shaders Guide](rendering/custom-shaders.md)

### Q: How do I handle errors gracefully?

**A:** Use Result types and match expressions:

```rust
match renderable.begin_drawing() {
    Ok(frame) => {
        // Render normally
    }
    Err(GraphicsError::SurfaceLost) => {
        warn!("Surface lost, recreating...");
        renderable.recreate_surface()?;
    }
    Err(e) => {
        error!("Unrecoverable error: {:?}", e);
    }
}
```

**Related:** [Error Handling Guide](advanced/error-handling.md)

### Q: How do I improve performance?

**A:** Use incremental updates and profiling:

```rust
// Use update_text() instead of rebuild
ui.update_text("label", "New text"); // <1ms

// Profile with puffin
init_profiling(ProfilingBackend::PuffinHttp);
profile_function!();

// Check frame time
if time.delta.as_secs_f32() > 0.016 {
    warn!("Slow frame");
}
```

**Related:** [Performance Tuning Guide](advanced/performance-tuning.md)

### Q: How do I create multiple windows?

**A:** Use WindowManager and track windows by ID:

```rust
impl App for MyApp {
    fn on_start(&mut self, ctx: &mut AppCtx) {
        // Create main window
        let main = ctx.create_window(descriptor1)?;

        // Create tool window
        let tool = ctx.create_window(descriptor2)?;

        self.windows.insert(main, WindowState::new());
        self.windows.insert(tool, WindowState::new());
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Render correct window
        if let Some(state) = self.windows.get_mut(&window_id) {
            state.render();
        }
    }
}
```

**Related:** [Multi-Window Apps Guide](advanced/multi-window-apps.md)

### Q: How do I run background tasks?

**A:** Use TaskPool and async/await:

```rust
let task_pool = engine.get::<TaskPool>().unwrap();

let handle = task_pool.spawn(async {
    // Background work
    expensive_computation().await
});

// Check completion
if handle.is_ready() {
    let result = handle.now_or_never().unwrap();
}
```

**Related:** [Async Tasks Guide](advanced/async-tasks.md)

---

## Getting Help

If this guide doesn't solve your problem:

1. **Check examples:** Look at the examples directory for working code
2. **Read API docs:** Check docs.rs for API documentation
3. **Search issues:** Look for similar issues on GitHub
4. **Ask for help:** Open a GitHub issue with:
   - Minimal reproducible example
   - Full error message
   - Rust version (`rustc --version`)
   - Platform (Windows/macOS/Linux/Web)
   - Astrelis version

## Related Guides

**Getting Started:**
- [Installation](getting-started/01-installation.md)
- [Hello Window](getting-started/03-hello-window.md)
- [Rendering Fundamentals](getting-started/04-rendering-fundamentals.md)

**Advanced:**
- [Error Handling](advanced/error-handling.md)
- [Performance Tuning](advanced/performance-tuning.md)
- [Multi-Window Apps](advanced/multi-window-apps.md)

**UI System:**
- [First UI](getting-started/05-first-ui.md)
- [Incremental Updates](getting-started/06-incremental-updates.md)
- [Custom Widgets](ui/custom-widgets.md)

**Asset System:**
- [Loading Assets](asset-system/loading-assets.md)
- [Hot Reload](asset-system/hot-reload.md)
