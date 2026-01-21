# Hot Reload

This guide explains how to set up hot reloading for assets in Astrelis. Learn to edit textures, shaders, and config files while your game is running without restarting.

## Overview

**Hot reload** allows assets to be updated while the application runs:

- Edit a texture → see changes immediately
- Modify a shader → recompile and update live
- Change config files → reload without restart

**Benefits:**
- Faster iteration (no restart required)
- Test changes instantly
- Better artist/designer workflow
- Debug rendering issues quickly

**Comparison to Unity:** Similar to Unity's automatic asset import and hot reload.

## How Hot Reload Works

```text
1. File watcher detects change (notify crate)
2. AssetServer receives file change event
3. Asset is reloaded from disk
4. AssetEvent::Modified is emitted
5. Systems respond to modified event
6. GPU resources are updated
```

## Setting Up Hot Reload

### Step 1: Enable File Watching

```rust
use astrelis_assets::AssetServer;
use std::path::PathBuf;

// Create asset server with file watching enabled
let asset_server = AssetServer::builder()
    .base_path(PathBuf::from("assets"))
    .watch(true) // Enable file watching
    .build()?;
```

**File Watcher:** Uses the `notify` crate to monitor filesystem changes.

### Step 2: Handle Modified Events

```rust
use astrelis_assets::{AssetEvent, Handle};
use astrelis_render::Texture;

struct Game {
    texture_handle: Handle<Texture>,
}

impl Game {
    fn update(&mut self, assets: &AssetServer) {
        // Check for modified textures
        for event in assets.drain_events::<Texture>() {
            if let AssetEvent::Modified { handle } = event {
                if handle == self.texture_handle {
                    println!("Texture was reloaded!");
                    // GPU resources automatically updated
                }
            }
        }
    }
}
```

### Step 3: Test Hot Reload

1. Run your game
2. Edit `assets/textures/player.png` in an image editor
3. Save the file
4. Changes appear immediately in the running game

## Hot Reloading Different Asset Types

### Textures

Textures automatically update GPU resources:

```rust
use astrelis_assets::{AssetServer, Handle};
use astrelis_render::Texture;

struct Sprite {
    texture: Handle<Texture>,
}

impl Sprite {
    fn update(&mut self, assets: &AssetServer) {
        for event in assets.drain_events::<Texture>() {
            if let AssetEvent::Modified { handle } = event {
                if handle == self.texture {
                    // Texture GPU memory automatically updated
                    println!("Sprite texture reloaded");
                }
            }
        }
    }

    fn render(&self, assets: &AssetServer, renderer: &mut Renderer) {
        // Always uses latest texture data
        if let Some(texture) = assets.get(&self.texture) {
            renderer.draw_sprite(texture);
        }
    }
}
```

**Automatic:** GPU texture data is updated automatically when file changes.

### Shaders

Shaders require pipeline recreation:

```rust
use astrelis_assets::{AssetEvent, Handle};
use astrelis_render::Shader;

struct Material {
    shader_handle: Handle<Shader>,
    pipeline: Option<wgpu::RenderPipeline>,
}

impl Material {
    fn update(&mut self, assets: &AssetServer, graphics: &GraphicsContext) {
        for event in assets.drain_events::<Shader>() {
            if let AssetEvent::Modified { handle } = event {
                if handle == self.shader_handle {
                    println!("Shader modified, recompiling...");

                    // Recreate pipeline with new shader
                    if let Some(shader) = assets.get(&self.shader_handle) {
                        match self.recreate_pipeline(graphics, shader) {
                            Ok(pipeline) => {
                                self.pipeline = Some(pipeline);
                                println!("Shader compiled successfully!");
                            }
                            Err(e) => {
                                eprintln!("Shader compilation failed: {}", e);
                                // Keep old pipeline
                            }
                        }
                    }
                }
            }
        }
    }

    fn recreate_pipeline(
        &self,
        graphics: &GraphicsContext,
        shader: &Shader,
    ) -> Result<wgpu::RenderPipeline, String> {
        let shader_module = graphics.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Hot Reloaded Shader"),
            source: wgpu::ShaderSource::Wgsl(shader.source.clone().into()),
        });

        // Create pipeline with new shader module
        // ... pipeline creation code

        Ok(pipeline)
    }
}
```

**Manual:** Pipelines must be recreated when shaders change.

### Configuration Files (JSON/TOML)

```rust
use serde::{Deserialize, Serialize};
use astrelis_assets::{AssetServer, Handle};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameConfig {
    player_speed: f32,
    enemy_count: u32,
    difficulty: String,
}

struct Game {
    config_handle: Handle<GameConfig>,
    config: GameConfig,
}

impl Game {
    fn update(&mut self, assets: &AssetServer) {
        // Check for config changes
        for event in assets.drain_events::<GameConfig>() {
            if let AssetEvent::Modified { handle } = event {
                if handle == self.config_handle {
                    // Reload config
                    if let Some(new_config) = assets.get(&self.config_handle) {
                        self.config = new_config.clone();
                        println!("Config reloaded: {:?}", self.config);
                    }
                }
            }
        }
    }
}
```

### Audio Files

```rust
use astrelis_audio::AudioClip;

struct SoundEffect {
    clip_handle: Handle<AudioClip>,
}

impl SoundEffect {
    fn update(&mut self, assets: &AssetServer, audio_system: &mut AudioSystem) {
        for event in assets.drain_events::<AudioClip>() {
            if let AssetEvent::Modified { handle } = event {
                if handle == self.clip_handle {
                    println!("Audio clip reloaded");
                    // Stop old playback, use new clip
                    audio_system.stop_all();
                }
            }
        }
    }
}
```

## Best Practices

### Graceful Shader Recompilation

Handle shader compilation errors gracefully:

```rust
struct MaterialSystem {
    materials: Vec<Material>,
}

impl MaterialSystem {
    fn update(&mut self, assets: &AssetServer, graphics: &GraphicsContext) {
        for event in assets.drain_events::<Shader>() {
            if let AssetEvent::Modified { handle } = event {
                for material in &mut self.materials {
                    if material.shader_handle == handle {
                        match material.try_reload_shader(graphics, assets) {
                            Ok(()) => {
                                log::info!("Shader reloaded successfully");
                            }
                            Err(e) => {
                                log::error!("Shader compilation failed: {}", e);
                                // Show error overlay
                                self.show_shader_error_overlay(&e);
                                // Keep using old shader
                            }
                        }
                    }
                }
            }
        }
    }

    fn show_shader_error_overlay(&self, error: &str) {
        // Display error in-game
        // ... overlay rendering
    }
}
```

### Debouncing File Changes

Some editors save files multiple times:

```rust
use std::time::{Duration, Instant};
use std::collections::HashMap;

struct AssetReloadDebouncer {
    last_reload: HashMap<PathBuf, Instant>,
    debounce_duration: Duration,
}

impl AssetReloadDebouncer {
    fn new() -> Self {
        Self {
            last_reload: HashMap::new(),
            debounce_duration: Duration::from_millis(100),
        }
    }

    fn should_reload(&mut self, path: &Path) -> bool {
        let now = Instant::now();

        if let Some(last_time) = self.last_reload.get(path) {
            if now.duration_since(*last_time) < self.debounce_duration {
                return false; // Too soon, ignore
            }
        }

        self.last_reload.insert(path.to_path_buf(), now);
        true
    }
}

// Usage
if debouncer.should_reload(&asset_path) {
    // Reload asset
}
```

### Preserving State Across Reloads

```rust
struct Character {
    texture: Handle<Texture>,
    position: Vec2,
    health: f32,
    // ... other state
}

impl Character {
    fn handle_texture_reload(&mut self, assets: &AssetServer) {
        for event in assets.drain_events::<Texture>() {
            if let AssetEvent::Modified { handle } = event {
                if handle == self.texture {
                    // Texture reloaded, but position/health preserved
                    println!("Texture updated, state preserved");
                }
            }
        }
    }
}
```

### Conditional Hot Reload (Debug Only)

```rust
#[cfg(debug_assertions)]
fn enable_hot_reload(asset_server: &mut AssetServer) {
    asset_server.enable_watching();
}

#[cfg(not(debug_assertions))]
fn enable_hot_reload(_asset_server: &mut AssetServer) {
    // Disabled in release builds
}
```

## Live Shader Editing

Complete example for live shader development:

```rust
use astrelis_assets::{AssetServer, AssetEvent, Handle};
use astrelis_render::{Shader, GraphicsContext};

struct LiveShaderEditor {
    shader_handle: Handle<Shader>,
    pipeline: Option<wgpu::RenderPipeline>,
    last_error: Option<String>,
}

impl LiveShaderEditor {
    fn new(assets: &AssetServer) -> Result<Self, Box<dyn std::error::Error>> {
        let shader_handle = assets.load("shaders/custom.wgsl")?;

        Ok(Self {
            shader_handle,
            pipeline: None,
            last_error: None,
        })
    }

    fn update(&mut self, assets: &AssetServer, graphics: &GraphicsContext) {
        // Handle shader modifications
        for event in assets.drain_events::<Shader>() {
            if let AssetEvent::Modified { handle } = event {
                if handle == self.shader_handle {
                    self.reload_shader(assets, graphics);
                }
            }
        }

        // Initialize pipeline on first load
        if self.pipeline.is_none() && assets.is_loaded(&self.shader_handle) {
            self.reload_shader(assets, graphics);
        }
    }

    fn reload_shader(&mut self, assets: &AssetServer, graphics: &GraphicsContext) {
        let Some(shader) = assets.get(&self.shader_handle) else {
            return;
        };

        println!("Recompiling shader...");

        match self.compile_shader(graphics, shader) {
            Ok(pipeline) => {
                self.pipeline = Some(pipeline);
                self.last_error = None;
                println!("✓ Shader compiled successfully");
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
                eprintln!("✗ Shader compilation failed:\n{}", e);
                // Keep old pipeline if it exists
            }
        }
    }

    fn compile_shader(
        &self,
        graphics: &GraphicsContext,
        shader: &Shader,
    ) -> Result<wgpu::RenderPipeline, String> {
        // Create shader module
        let shader_module = graphics.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Live Shader"),
            source: wgpu::ShaderSource::Wgsl(shader.source.clone().into()),
        });

        // Create pipeline
        // ... pipeline creation code

        Ok(pipeline)
    }

    fn render(&self, render_pass: &mut wgpu::RenderPass) {
        if let Some(pipeline) = &self.pipeline {
            render_pass.set_pipeline(pipeline);
            // ... draw commands
        }
    }

    fn render_error_overlay(&self, ui: &mut UiSystem) {
        if let Some(error) = &self.last_error {
            ui.build(|root| {
                root.container(|c| {
                    c.text("Shader Compilation Error")
                        .font_size(24.0)
                        .color(Color::RED)
                        .build();

                    c.text(error)
                        .font_size(14.0)
                        .color(Color::WHITE)
                        .build();
                })
                .background_color(Color::rgba(0.1, 0.0, 0.0, 0.9))
                .padding(20.0)
                .build();
            });
        }
    }
}
```

## Live Texture Editing Workflow

Example workflow for artists:

1. **Setup:**
```rust
let texture_handle = assets.load("textures/character.png")?;
```

2. **Edit texture in Photoshop/GIMP:**
   - Modify colors, add details, etc.
   - Save file (Ctrl+S)

3. **Automatic reload:**
   - File watcher detects change
   - Texture reloads from disk
   - GPU texture updated
   - Changes visible immediately

4. **No restart needed!**

## Performance Considerations

### File Watch Overhead

File watching has minimal overhead:
- Polling: ~1ms per check
- Event-driven (recommended): ~0.01ms

### Reload Frequency

```rust
// Limit reload frequency
let mut last_reload_time = Instant::now();
let reload_cooldown = Duration::from_millis(100);

for event in assets.drain_events::<Texture>() {
    if let AssetEvent::Modified { handle } = event {
        let now = Instant::now();
        if now.duration_since(last_reload_time) > reload_cooldown {
            // Reload
            last_reload_time = now;
        }
    }
}
```

### Disable in Release Builds

```rust
let asset_server = AssetServer::builder()
    .base_path(PathBuf::from("assets"))
    .watch(cfg!(debug_assertions)) // Only in debug builds
    .build()?;
```

## Platform Support

| Platform | File Watching | Notes |
|----------|---------------|-------|
| Windows | ✅ Yes | Native API |
| macOS | ✅ Yes | FSEvents |
| Linux | ✅ Yes | inotify |
| Web | ❌ No | No filesystem access |
| Mobile | ❌ No | Sandboxed filesystem |

## Troubleshooting

### Changes Not Detected

**Cause:** File watcher not enabled or file saved to wrong location.

**Fix:** Verify file watcher is enabled:
```rust
// Check if watching is enabled
assert!(asset_server.is_watching());

// Verify file path
let full_path = Path::new("assets").join("textures/player.png");
println!("Watching: {}", full_path.display());
```

### Multiple Reloads for One Save

**Cause:** Editor saves file multiple times.

**Fix:** Use debouncing (see best practices above).

### Shader Errors After Reload

**Cause:** Syntax error in modified shader.

**Fix:** Keep old pipeline and show error:
```rust
match compile_shader(shader) {
    Ok(pipeline) => self.pipeline = Some(pipeline),
    Err(e) => {
        eprintln!("Shader error: {}", e);
        // Keep old pipeline, show error overlay
    }
}
```

### Performance Drops

**Cause:** Too many assets being watched.

**Fix:** Watch only specific directories:
```rust
asset_server.watch_directory("assets/textures")?;
asset_server.watch_directory("assets/shaders")?;
// Don't watch assets/audio (large files, rarely change)
```

## Comparison to Unity

| Unity | Astrelis | Notes |
|-------|----------|-------|
| Automatic asset import | File watching | Similar concept |
| Asset post-processors | AssetLoader | Custom processing |
| `AssetDatabase.Refresh()` | Automatic | No manual refresh |
| Play mode changes lost | State preserved | Better workflow |

## Next Steps

- **Practice:** Try the `hot_reload_demo` example
- **Learn More:** [Custom Loaders](custom-loaders.md) for custom hot reload logic
- **Integration:** Add hot reload to your project
- **Examples:** `hot_reload_demo`, `live_shader_editor`

## See Also

- [Loading Assets](loading-assets.md) - Asset loading basics
- [Custom Loaders](custom-loaders.md) - Custom reload behavior
- API Reference: [`AssetServer::watch()`](../../api/astrelis-assets/struct.AssetServer.html#method.watch)
- API Reference: [`AssetEvent`](../../api/astrelis-assets/enum.AssetEvent.html)
