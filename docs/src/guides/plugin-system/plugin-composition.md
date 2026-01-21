# Plugin Composition

This guide explains how to compose plugins in Astrelis, manage dependencies, create plugin bundles, and handle conditional loading for building flexible engine configurations.

## Overview

**Plugin composition** enables:

- Modular engine configuration
- Feature toggles (enable/disable functionality)
- Dependency management
- Reusable plugin bundles
- Conditional loading based on platform/features

**Key Concepts:**
- Plugin bundles: Groups of related plugins
- Dependencies: Required vs optional
- Topological sorting: Automatic dependency ordering
- Conditional plugins: Platform or feature-specific

## Basic Composition

### Adding Multiple Plugins

```rust
use astrelis::Engine;

let engine = Engine::builder()
    .add_plugin(LoggerPlugin::default())
    .add_plugin(WindowPlugin::default())
    .add_plugin(RenderPlugin::default())
    .add_plugin(InputPlugin::default())
    .add_plugin(AssetPlugin::default())
    .build();
```

### Chaining Plugins

```rust
let engine = Engine::builder()
    .add_plugins(vec![
        Box::new(LoggerPlugin::default()),
        Box::new(WindowPlugin::default()),
        Box::new(RenderPlugin::default()),
    ])
    .build();
```

## Plugin Bundles

### Creating a Plugin Bundle

Group related plugins:

```rust
use astrelis::{Plugin, Engine};

pub struct DefaultPlugins;

impl Plugin for DefaultPlugins {
    fn build(&self, engine: &mut Engine) {
        // Add core plugins
        engine.add_plugin(LoggerPlugin::default());
        engine.add_plugin(TimePlugin::default());
        engine.add_plugin(WindowPlugin::default());
        engine.add_plugin(InputPlugin::default());
        engine.add_plugin(AssetPlugin::default());
        engine.add_plugin(RenderPlugin::default());
    }

    fn name(&self) -> &str {
        "DefaultPlugins"
    }
}
```

**Usage:**
```rust
// One line setup
let engine = Engine::builder()
    .add_plugin(DefaultPlugins)
    .build();
```

### Nested Bundles

Bundles can include other bundles:

```rust
pub struct MinimalPlugins;

impl Plugin for MinimalPlugins {
    fn build(&self, engine: &mut Engine) {
        engine.add_plugin(LoggerPlugin::default());
        engine.add_plugin(TimePlugin::default());
    }
}

pub struct DefaultPlugins;

impl Plugin for DefaultPlugins {
    fn build(&self, engine: &mut Engine) {
        // Include minimal plugins
        engine.add_plugin(MinimalPlugins);

        // Add additional plugins
        engine.add_plugin(WindowPlugin::default());
        engine.add_plugin(RenderPlugin::default());
        engine.add_plugin(InputPlugin::default());
    }
}
```

### Configurable Bundles

```rust
pub struct DefaultPlugins {
    pub enable_audio: bool,
    pub enable_networking: bool,
}

impl Default for DefaultPlugins {
    fn default() -> Self {
        Self {
            enable_audio: true,
            enable_networking: false,
        }
    }
}

impl Plugin for DefaultPlugins {
    fn build(&self, engine: &mut Engine) {
        // Core plugins (always added)
        engine.add_plugin(LoggerPlugin::default());
        engine.add_plugin(WindowPlugin::default());
        engine.add_plugin(RenderPlugin::default());

        // Conditional plugins
        if self.enable_audio {
            engine.add_plugin(AudioPlugin::default());
        }

        if self.enable_networking {
            engine.add_plugin(NetworkPlugin::default());
        }
    }
}

// Usage
let engine = Engine::builder()
    .add_plugin(DefaultPlugins {
        enable_audio: true,
        enable_networking: true,
    })
    .build();
```

## Dependency Management

### Declaring Dependencies

```rust
use astrelis::{Plugin, PluginDependency};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, engine: &mut Engine) {
        // WindowPlugin must be registered first
        let graphics = engine.get::<Arc<GraphicsContext>>()
            .expect("RenderPlugin requires WindowPlugin");

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

### Required vs Optional Dependencies

```rust
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, engine: &mut Engine) {
        // Optional: use profiler if available
        if let Some(profiler) = engine.get::<Arc<Profiler>>() {
            log::info!("Audio profiling enabled");
        }

        // Required: must have asset server
        let assets = engine.get::<Arc<AssetServer>>()
            .expect("AudioPlugin requires AssetPlugin");

        let audio_system = AudioSystem::new(assets.clone());
        engine.insert_resource(Arc::new(audio_system));
    }

    fn dependencies(&self) -> Vec<PluginDependency> {
        vec![
            PluginDependency::required::<AssetPlugin>(),
            PluginDependency::optional::<ProfilingPlugin>(),
        ]
    }
}
```

### Topological Sorting

Engine automatically orders plugins by dependencies:

```rust
// Unordered registration
let engine = Engine::builder()
    .add_plugin(RenderPlugin)      // Depends on WindowPlugin
    .add_plugin(AudioPlugin)       // Depends on AssetPlugin
    .add_plugin(WindowPlugin)      // No dependencies
    .add_plugin(AssetPlugin)       // No dependencies
    .build();

// Actual initialization order (automatic):
// 1. WindowPlugin
// 2. AssetPlugin
// 3. RenderPlugin (depends on WindowPlugin)
// 4. AudioPlugin (depends on AssetPlugin)
```

**Benefit:** No need to manually order plugins.

### Circular Dependency Detection

```rust
pub struct PluginA;

impl Plugin for PluginA {
    fn dependencies(&self) -> Vec<PluginDependency> {
        vec![PluginDependency::required::<PluginB>()]
    }
}

pub struct PluginB;

impl Plugin for PluginB {
    fn dependencies(&self) -> Vec<PluginDependency> {
        vec![PluginDependency::required::<PluginA>()] // Circular!
    }
}

// Panics with helpful error
let engine = Engine::builder()
    .add_plugin(PluginA)
    .add_plugin(PluginB)
    .build(); // Error: Circular dependency detected
```

## Conditional Plugin Loading

### Platform-Specific Plugins

```rust
pub struct PlatformPlugins;

impl Plugin for PlatformPlugins {
    fn build(&self, engine: &mut Engine) {
        #[cfg(target_os = "windows")]
        engine.add_plugin(WindowsPlugin);

        #[cfg(target_os = "macos")]
        engine.add_plugin(MacOSPlugin);

        #[cfg(target_os = "linux")]
        engine.add_plugin(LinuxPlugin);

        #[cfg(target_arch = "wasm32")]
        engine.add_plugin(WebPlugin);
    }
}
```

### Feature-Based Loading

```rust
pub struct OptionalPlugins;

impl Plugin for OptionalPlugins {
    fn build(&self, engine: &mut Engine) {
        #[cfg(feature = "audio")]
        engine.add_plugin(AudioPlugin::default());

        #[cfg(feature = "networking")]
        engine.add_plugin(NetworkPlugin::default());

        #[cfg(feature = "physics")]
        engine.add_plugin(PhysicsPlugin::default());
    }
}
```

**Cargo.toml:**
```toml
[features]
default = ["audio"]
audio = ["rodio"]
networking = ["tokio", "quinn"]
physics = ["rapier"]
```

### Debug/Release Plugins

```rust
pub struct DebugPlugins;

impl Plugin for DebugPlugins {
    fn build(&self, engine: &mut Engine) {
        #[cfg(debug_assertions)]
        {
            engine.add_plugin(DebugOverlayPlugin::default());
            engine.add_plugin(ProfilingPlugin::default());
            engine.add_plugin(HotReloadPlugin::default());
        }

        #[cfg(not(debug_assertions))]
        {
            // No debug plugins in release
            log::info!("Debug plugins disabled in release build");
        }
    }
}
```

## Plugin Groups

Organize plugins by functionality:

### Core Plugins

```rust
pub struct CorePlugins;

impl Plugin for CorePlugins {
    fn build(&self, engine: &mut Engine) {
        engine.add_plugin(LoggerPlugin::default());
        engine.add_plugin(TimePlugin::default());
        engine.add_plugin(ProfilingPlugin::default());
    }

    fn name(&self) -> &str {
        "CorePlugins"
    }
}
```

### Rendering Plugins

```rust
pub struct RenderingPlugins;

impl Plugin for RenderingPlugins {
    fn build(&self, engine: &mut Engine) {
        engine.add_plugin(WindowPlugin::default());
        engine.add_plugin(GraphicsPlugin::default());
        engine.add_plugin(RenderPlugin::default());
        engine.add_plugin(MaterialPlugin::default());
    }

    fn dependencies(&self) -> Vec<PluginDependency> {
        vec![PluginDependency::required::<CorePlugins>()]
    }
}
```

### Content Plugins

```rust
pub struct ContentPlugins;

impl Plugin for ContentPlugins {
    fn build(&self, engine: &mut Engine) {
        engine.add_plugin(AssetPlugin::default());
        engine.add_plugin(TextPlugin::default());
        engine.add_plugin(AudioPlugin::default());
        engine.add_plugin(AnimationPlugin::default());
    }

    fn dependencies(&self) -> Vec<PluginDependency> {
        vec![PluginDependency::required::<CorePlugins>()]
    }
}
```

### Complete Setup

```rust
let engine = Engine::builder()
    .add_plugin(CorePlugins)
    .add_plugin(RenderingPlugins)
    .add_plugin(ContentPlugins)
    .add_plugin(InputPlugin::default())
    .add_plugin(UIPlugin::default())
    .build();
```

## Plugin Override

Replace default plugin with custom version:

```rust
pub struct CustomWindowPlugin {
    pub title: String,
    pub size: (u32, u32),
}

impl Plugin for CustomWindowPlugin {
    fn build(&self, engine: &mut Engine) {
        // Custom window initialization
        let window = create_custom_window(&self.title, self.size);
        engine.insert_resource(Arc::new(window));
    }

    fn name(&self) -> &str {
        "WindowPlugin" // Same name as default
    }
}

// Use custom window plugin instead of default
let engine = Engine::builder()
    .add_plugin(CustomWindowPlugin {
        title: "My Game".to_string(),
        size: (1920, 1080),
    })
    // Don't add default WindowPlugin
    .add_plugin(RenderPlugin::default())
    .build();
```

## Performance Considerations

### Plugin Initialization Cost

```text
Core plugins:        ~1ms
Window/Graphics:     ~50-100ms (GPU initialization)
Asset system:        ~1ms
UI system:           ~5ms
Audio system:        ~10-20ms

Total startup:       ~70-130ms
```

### Lazy Initialization

Defer expensive setup:

```rust
pub struct LazyAudioPlugin;

impl Plugin for LazyAudioPlugin {
    fn build(&self, engine: &mut Engine) {
        // Don't initialize audio yet, just register initializer
        engine.insert_resource(Arc::new(LazyAudioSystem::new()));
    }
}

pub struct LazyAudioSystem {
    initialized: Mutex<bool>,
    system: Mutex<Option<AudioSystem>>,
}

impl LazyAudioSystem {
    fn get_or_init(&self) -> MutexGuard<Option<AudioSystem>> {
        let mut init = self.initialized.lock().unwrap();
        if !*init {
            // Initialize on first use
            log::info!("Initializing audio system...");
            *self.system.lock().unwrap() = Some(AudioSystem::new());
            *init = true;
        }
        self.system.lock().unwrap()
    }
}
```

## Plugin Templates

### Minimal Game Setup

```rust
let engine = Engine::builder()
    .add_plugin(LoggerPlugin::default())
    .add_plugin(WindowPlugin::default())
    .add_plugin(RenderPlugin::default())
    .build();
```

### UI Application

```rust
let engine = Engine::builder()
    .add_plugin(DefaultPlugins)
    .add_plugin(UIPlugin::default())
    .add_plugin(TextPlugin::default())
    .build();
```

### Networked Game

```rust
let engine = Engine::builder()
    .add_plugin(DefaultPlugins {
        enable_networking: true,
        ..Default::default()
    })
    .add_plugin(PhysicsPlugin::default())
    .add_plugin(NetworkPlugin::default())
    .build();
```

### Headless Server

```rust
let engine = Engine::builder()
    .add_plugin(LoggerPlugin::default())
    .add_plugin(TimePlugin::default())
    .add_plugin(NetworkPlugin::default())
    .add_plugin(PhysicsPlugin::default())
    // No window/render plugins
    .build();
```

## Testing Plugin Composition

```rust
#[test]
fn test_default_plugins() {
    let engine = Engine::builder()
        .add_plugin(DefaultPlugins)
        .build();

    // Verify all resources registered
    assert!(engine.get::<Arc<AssetServer>>().is_some());
    assert!(engine.get::<Arc<GraphicsContext>>().is_some());
    assert!(engine.get::<Arc<InputState>>().is_some());
}

#[test]
fn test_conditional_plugins() {
    let engine = Engine::builder()
        .add_plugin(DefaultPlugins {
            enable_audio: false,
            enable_networking: false,
        })
        .build();

    // Audio should not be registered
    assert!(engine.get::<Arc<AudioSystem>>().is_none());
}

#[test]
#[should_panic]
fn test_missing_dependency() {
    // Should panic: RenderPlugin requires WindowPlugin
    let engine = Engine::builder()
        .add_plugin(RenderPlugin::default())
        .build();
}
```

## Best Practices

### ✅ DO: Use Bundles for Common Setups

```rust
// Good: Reusable setup
pub struct GamePlugins;

impl Plugin for GamePlugins {
    fn build(&self, engine: &mut Engine) {
        engine.add_plugin(DefaultPlugins);
        engine.add_plugin(PhysicsPlugin::default());
        engine.add_plugin(UIPlugin::default());
    }
}
```

### ✅ DO: Declare All Dependencies

```rust
fn dependencies(&self) -> Vec<PluginDependency> {
    vec![
        PluginDependency::required::<WindowPlugin>(),
        PluginDependency::required::<AssetPlugin>(),
    ]
}
```

### ✅ DO: Make Bundles Configurable

```rust
pub struct GamePlugins {
    pub enable_audio: bool,
    pub enable_physics: bool,
}
```

### ❌ DON'T: Rely on Plugin Order

```rust
// BAD: Order-dependent
let engine = Engine::builder()
    .add_plugin(WindowPlugin) // Must be first!
    .add_plugin(RenderPlugin) // Must be second!
    .build();

// GOOD: Use dependencies instead
impl Plugin for RenderPlugin {
    fn dependencies(&self) -> Vec<PluginDependency> {
        vec![PluginDependency::required::<WindowPlugin>()]
    }
}
```

### ❌ DON'T: Create Monolithic Bundles

```rust
// BAD: Everything in one bundle
pub struct AllPlugins; // Too broad

// GOOD: Focused bundles
pub struct CorePlugins;
pub struct RenderingPlugins;
pub struct GameplayPlugins;
```

## Troubleshooting

### Plugin Not Initialized

**Cause:** Forgot to add plugin or bundle.

**Fix:**
```rust
let engine = Engine::builder()
    .add_plugin(MyPlugin::default()) // Add this!
    .build();
```

### Wrong Initialization Order

**Cause:** Manual ordering conflicts with dependencies.

**Fix:** Remove manual ordering, let engine sort:
```rust
// Engine automatically orders by dependencies
let engine = Engine::builder()
    .add_plugin(RenderPlugin) // Depends on WindowPlugin
    .add_plugin(WindowPlugin) // No dependencies
    .build();
// Initializes: WindowPlugin → RenderPlugin (automatic)
```

### Circular Dependencies

**Cause:** Plugin A depends on B, B depends on A.

**Fix:** Restructure dependencies or merge plugins:
```rust
// Before: Circular
PluginA -> PluginB -> PluginA (circular!)

// After: Linear
PluginA -> PluginBase <- PluginB
```

## Next Steps

- **Practice:** Create custom plugin bundles for your project
- **Learn More:** [Resource System](resource-system.md) for shared state
- **Advanced:** [Creating Plugins](creating-plugins.md) for plugin development
- **Examples:** `plugin_composition`, `custom_bundles`

## See Also

- [Creating Plugins](creating-plugins.md) - Plugin development
- [Resource System](resource-system.md) - Resource management
- API Reference: [`Engine::builder()`](../../api/astrelis/struct.Engine.html#method.builder)
- API Reference: [`PluginDependency`](../../api/astrelis/struct.PluginDependency.html)
