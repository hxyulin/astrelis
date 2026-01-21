# Creating Plugins

This guide explains how to create plugins for Astrelis to add functionality, register resources, and build reusable engine components.

## Overview

**Plugins** are modular extensions to the engine that:

- Register resources and systems
- Configure engine behavior
- Provide reusable functionality
- Enable/disable features modularly
- Define dependencies on other plugins

**Benefits:**
- Modular architecture
- Reusable components
- Clean separation of concerns
- Easy feature toggles

**Comparison to Unity:** Similar to Unity packages, but integrated at engine level like Bevy plugins.

## Plugin Trait

```rust
pub trait Plugin: Send + Sync + 'static {
    /// Build the plugin, registering resources and systems
    fn build(&self, engine: &mut Engine);

    /// Optional: plugin name for debugging
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    /// Optional: define dependencies
    fn dependencies(&self) -> Vec<PluginDependency> {
        Vec::new()
    }
}
```

**Key Method:**
- `build()`: Called once during engine initialization

## Your First Plugin: Logger

### Step 1: Define Plugin Struct

```rust
use astrelis::Plugin;

pub struct LoggerPlugin {
    pub log_level: log::LevelFilter,
}

impl Default for LoggerPlugin {
    fn default() -> Self {
        Self {
            log_level: log::LevelFilter::Info,
        }
    }
}
```

### Step 2: Implement Plugin Trait

```rust
use astrelis::{Plugin, Engine};

impl Plugin for LoggerPlugin {
    fn build(&self, engine: &mut Engine) {
        // Initialize logger
        env_logger::Builder::new()
            .filter_level(self.log_level)
            .init();

        log::info!("Logger initialized with level: {:?}", self.log_level);
    }

    fn name(&self) -> &str {
        "LoggerPlugin"
    }
}
```

### Step 3: Use Plugin

```rust
use astrelis::Engine;

let engine = Engine::builder()
    .add_plugin(LoggerPlugin {
        log_level: log::LevelFilter::Debug,
    })
    .build();
```

## Plugin with Resources

Plugins register resources for other systems to use:

```rust
use astrelis::{Plugin, Engine};
use std::sync::Arc;

pub struct AssetPlugin {
    pub base_path: PathBuf,
}

impl Plugin for AssetPlugin {
    fn build(&self, engine: &mut Engine) {
        // Create asset server
        let asset_server = AssetServer::new(self.base_path.clone());

        // Register as shared resource
        engine.insert_resource(Arc::new(asset_server));

        log::info!("AssetPlugin initialized with path: {:?}", self.base_path);
    }

    fn name(&self) -> &str {
        "AssetPlugin"
    }
}
```

**Access Resource:**
```rust
// Later in application code
if let Some(assets) = engine.get::<Arc<AssetServer>>() {
    let texture = assets.load("texture.png")?;
}
```

## Plugin Dependencies

Plugins can depend on other plugins:

```rust
use astrelis::{Plugin, Engine, PluginDependency};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, engine: &mut Engine) {
        // Get graphics context from WindowPlugin
        let graphics = engine.get::<Arc<GraphicsContext>>()
            .expect("RenderPlugin requires WindowPlugin");

        // Initialize renderer
        let renderer = Renderer::new(graphics.clone());
        engine.insert_resource(Arc::new(renderer));
    }

    fn dependencies(&self) -> Vec<PluginDependency> {
        vec![
            PluginDependency::required::<WindowPlugin>(),
        ]
    }
}
```

**Dependency Types:**
- `required()`: Must be present, error if missing
- `optional()`: Skip if missing, no error

## Complete Example: Debug Overlay Plugin

Full-featured plugin with configuration:

```rust
use astrelis::{Plugin, Engine};
use std::sync::{Arc, RwLock};

pub struct DebugOverlayPlugin {
    pub show_fps: bool,
    pub show_memory: bool,
    pub font_size: f32,
}

impl Default for DebugOverlayPlugin {
    fn default() -> Self {
        Self {
            show_fps: true,
            show_memory: true,
            font_size: 14.0,
        }
    }
}

impl Plugin for DebugOverlayPlugin {
    fn build(&self, engine: &mut Engine) {
        // Create debug overlay state
        let overlay_state = Arc::new(RwLock::new(DebugOverlayState {
            fps: 0.0,
            memory_mb: 0.0,
            frame_time_ms: 0.0,
            config: DebugOverlayConfig {
                show_fps: self.show_fps,
                show_memory: self.show_memory,
                font_size: self.font_size,
            },
        }));

        // Register resource
        engine.insert_resource(overlay_state);

        log::info!("Debug overlay plugin initialized");
    }

    fn name(&self) -> &str {
        "DebugOverlayPlugin"
    }
}

pub struct DebugOverlayState {
    pub fps: f32,
    pub memory_mb: f32,
    pub frame_time_ms: f32,
    pub config: DebugOverlayConfig,
}

pub struct DebugOverlayConfig {
    pub show_fps: bool,
    pub show_memory: bool,
    pub font_size: f32,
}
```

**Usage in Game:**
```rust
impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        // Update debug overlay
        if let Some(overlay) = ctx.engine.get::<Arc<RwLock<DebugOverlayState>>>() {
            let mut state = overlay.write().unwrap();
            state.fps = 1.0 / time.delta.as_secs_f32();
            state.frame_time_ms = time.delta.as_secs_f32() * 1000.0;
        }
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Render debug overlay
        if let Some(overlay) = ctx.engine.get::<Arc<RwLock<DebugOverlayState>>>() {
            let state = overlay.read().unwrap();

            if state.config.show_fps {
                ui.build(|root| {
                    root.text(&format!("FPS: {:.1}", state.fps))
                        .font_size(state.config.font_size)
                        .build();
                });
            }
        }
    }
}
```

## Plugin Bundles

Group related plugins:

```rust
pub struct DefaultPlugins;

impl Plugin for DefaultPlugins {
    fn build(&self, engine: &mut Engine) {
        engine
            .add_plugin(LoggerPlugin::default())
            .add_plugin(AssetPlugin::default())
            .add_plugin(WindowPlugin::default())
            .add_plugin(InputPlugin::default())
            .add_plugin(TimePlugin::default());
    }

    fn name(&self) -> &str {
        "DefaultPlugins"
    }
}
```

**Usage:**
```rust
let engine = Engine::builder()
    .add_plugin(DefaultPlugins)
    .build();
```

## Plugin Configuration

### Builder Pattern

```rust
pub struct RenderPlugin {
    pub vsync: bool,
    pub msaa_samples: u32,
    pub power_preference: PowerPreference,
}

impl RenderPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn vsync(mut self, enabled: bool) -> Self {
        self.vsync = enabled;
        self
    }

    pub fn msaa(mut self, samples: u32) -> Self {
        self.msaa_samples = samples;
        self
    }

    pub fn power_preference(mut self, preference: PowerPreference) -> Self {
        self.power_preference = preference;
        self
    }
}

impl Default for RenderPlugin {
    fn default() -> Self {
        Self {
            vsync: true,
            msaa_samples: 4,
            power_preference: PowerPreference::HighPerformance,
        }
    }
}
```

**Usage:**
```rust
let engine = Engine::builder()
    .add_plugin(
        RenderPlugin::new()
            .vsync(false)
            .msaa(8)
    )
    .build();
```

## Conditional Plugins

Enable plugins based on features:

```rust
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, engine: &mut Engine) {
        #[cfg(feature = "audio")]
        {
            let audio_system = AudioSystem::new();
            engine.insert_resource(Arc::new(audio_system));
            log::info!("Audio system initialized");
        }

        #[cfg(not(feature = "audio"))]
        {
            log::warn!("Audio feature disabled");
        }
    }
}
```

**Cargo.toml:**
```toml
[features]
default = ["audio"]
audio = ["rodio"]
```

## Plugin State Management

Share state across systems:

```rust
use std::sync::{Arc, RwLock};

pub struct GameStatePlugin {
    pub initial_state: GameState,
}

impl Plugin for GameStatePlugin {
    fn build(&self, engine: &mut Engine) {
        let game_state = Arc::new(RwLock::new(self.initial_state.clone()));
        engine.insert_resource(game_state);
    }
}

#[derive(Clone)]
pub enum GameState {
    MainMenu,
    Playing,
    Paused,
    GameOver,
}

// Access from anywhere
fn update_game(engine: &Engine) {
    if let Some(state) = engine.get::<Arc<RwLock<GameState>>>() {
        let mut state = state.write().unwrap();
        *state = GameState::Playing;
    }
}
```

## Plugin Lifecycle

### Initialization Order

Plugins initialize in the order they're added:

```rust
let engine = Engine::builder()
    .add_plugin(LoggerPlugin::default())  // 1st
    .add_plugin(WindowPlugin::default())  // 2nd (depends on logger)
    .add_plugin(RenderPlugin::default())  // 3rd (depends on window)
    .build();
```

**Important:** Add dependencies before dependents.

### Topological Sorting

Engine automatically sorts plugins by dependencies:

```rust
// Engine handles ordering automatically
let engine = Engine::builder()
    .add_plugin(RenderPlugin)   // Depends on WindowPlugin
    .add_plugin(WindowPlugin)   // No dependencies
    .build();

// Actually initializes: WindowPlugin → RenderPlugin
```

### Cleanup

Plugins don't have explicit cleanup, use RAII:

```rust
pub struct ResourcePlugin;

impl Plugin for ResourcePlugin {
    fn build(&self, engine: &mut Engine) {
        struct ResourceManager {
            // ... fields
        }

        impl Drop for ResourceManager {
            fn drop(&mut self) {
                // Cleanup when engine is dropped
                log::info!("ResourceManager cleaned up");
            }
        }

        engine.insert_resource(Arc::new(ResourceManager { /* ... */ }));
    }
}
```

## Testing Plugins

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_plugin() {
        let engine = Engine::builder()
            .add_plugin(LoggerPlugin {
                log_level: log::LevelFilter::Debug,
            })
            .build();

        // Logger should be initialized
        log::debug!("Test message");
    }

    #[test]
    fn test_asset_plugin_resources() {
        let engine = Engine::builder()
            .add_plugin(AssetPlugin {
                base_path: PathBuf::from("assets"),
            })
            .build();

        // Resource should be available
        assert!(engine.get::<Arc<AssetServer>>().is_some());
    }
}
```

### Integration Tests

```rust
#[test]
fn test_plugin_dependencies() {
    // Should succeed: dependencies met
    let engine = Engine::builder()
        .add_plugin(WindowPlugin::default())
        .add_plugin(RenderPlugin::default()) // Depends on WindowPlugin
        .build();

    assert!(engine.get::<Arc<GraphicsContext>>().is_some());
    assert!(engine.get::<Arc<Renderer>>().is_some());
}

#[test]
#[should_panic(expected = "requires WindowPlugin")]
fn test_missing_dependency() {
    // Should fail: RenderPlugin requires WindowPlugin
    let engine = Engine::builder()
        .add_plugin(RenderPlugin::default())
        .build();
}
```

## Best Practices

### ✅ DO: Single Responsibility

```rust
// GOOD: Each plugin does one thing
pub struct InputPlugin; // Only handles input
pub struct AudioPlugin; // Only handles audio
pub struct PhysicsPlugin; // Only handles physics
```

### ✅ DO: Use Builder Pattern

```rust
pub struct PhysicsPlugin {
    pub gravity: Vec2,
    pub time_step: f32,
}

impl PhysicsPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn gravity(mut self, gravity: Vec2) -> Self {
        self.gravity = gravity;
        self
    }
}
```

### ✅ DO: Document Dependencies

```rust
/// Physics simulation plugin.
///
/// # Dependencies
/// - Requires `TimePlugin` for frame timing
/// - Requires `TransformPlugin` for positions
pub struct PhysicsPlugin;
```

### ❌ DON'T: Panic in build()

```rust
// BAD
impl Plugin for MyPlugin {
    fn build(&self, engine: &mut Engine) {
        let resource = engine.get::<MyResource>()
            .unwrap(); // DON'T PANIC!
    }
}

// GOOD
impl Plugin for MyPlugin {
    fn build(&self, engine: &mut Engine) {
        let resource = engine.get::<MyResource>()
            .expect("MyPlugin requires ResourcePlugin");
    }

    fn dependencies(&self) -> Vec<PluginDependency> {
        vec![PluginDependency::required::<ResourcePlugin>()]
    }
}
```

### ❌ DON'T: Do Heavy Work in build()

```rust
// BAD: Loading assets in build()
impl Plugin for MyPlugin {
    fn build(&self, engine: &mut Engine) {
        let texture = load_all_textures(); // Slow!
    }
}

// GOOD: Register resource, load lazily
impl Plugin for MyPlugin {
    fn build(&self, engine: &mut Engine) {
        let loader = TextureLoader::new();
        engine.insert_resource(Arc::new(loader));
    }
}
```

## Plugin Examples

### Profiling Plugin

```rust
pub struct ProfilingPlugin {
    pub backend: ProfilingBackend,
}

impl Plugin for ProfilingPlugin {
    fn build(&self, engine: &mut Engine) {
        init_profiling(self.backend);
        log::info!("Profiling enabled: {:?}", self.backend);
    }
}

pub enum ProfilingBackend {
    Puffin,
    Tracy,
    None,
}
```

### Configuration Plugin

```rust
pub struct ConfigPlugin {
    pub config_path: PathBuf,
}

impl Plugin for ConfigPlugin {
    fn build(&self, engine: &mut Engine) {
        let config = load_config(&self.config_path)
            .expect("Failed to load config");

        engine.insert_resource(Arc::new(config));
    }
}
```

## Comparison to Unity and Bevy

| Unity | Bevy | Astrelis | Notes |
|-------|------|----------|-------|
| Packages | Plugins | Plugins | Modularity |
| N/A | Systems | Resources | Data access |
| N/A | Dependencies | Dependencies | Plugin ordering |
| ScriptableObject | Resource | Resource | Shared data |

## Troubleshooting

### Plugin Not Initialized

**Cause:** Forgot to add plugin to engine.

**Fix:**
```rust
let engine = Engine::builder()
    .add_plugin(MyPlugin::default()) // Add this!
    .build();
```

### Dependency Not Found

**Cause:** Plugin order incorrect or dependency missing.

**Fix:** Add dependency first or use `dependencies()`:
```rust
fn dependencies(&self) -> Vec<PluginDependency> {
    vec![PluginDependency::required::<WindowPlugin>()]
}
```

### Resource Already Registered

**Cause:** Two plugins registering same resource type.

**Fix:** Coordinate plugins or use wrapper types:
```rust
pub struct PhysicsConfig(pub Config);
pub struct AudioConfig(pub Config);
```

## Next Steps

- **Practice:** Create a custom plugin for your game
- **Learn More:** [Plugin Composition](plugin-composition.md) for complex setups
- **Advanced:** [Resource System](resource-system.md) for resource management
- **Examples:** `plugin_tutorial`, `custom_plugin`

## See Also

- [Plugin Composition](plugin-composition.md) - Combining plugins
- [Resource System](resource-system.md) - Managing resources
- API Reference: [`Plugin`](../../api/astrelis/trait.Plugin.html)
- API Reference: [`Engine`](../../api/astrelis/struct.Engine.html)
