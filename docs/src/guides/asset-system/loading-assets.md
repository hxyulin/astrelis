# Loading Assets

This guide explains how to use the Astrelis asset system to load images, fonts, shaders, and custom data. Learn to manage asset lifecycles with type-safe handles and generation counters.

## Overview

The **asset system** provides:

- **Async loading**: Non-blocking asset loading
- **Type safety**: `Handle<T>` prevents type errors
- **Generation counters**: Prevent use-after-free bugs
- **Hot reload**: Reload assets without restart (see [Hot Reload](hot-reload.md))
- **Caching**: Automatic deduplication

**Key Components:**
- `AssetServer`: Central asset manager
- `Handle<T>`: Type-safe reference to an asset
- `AssetLoader`: Trait for loading custom assets

**Comparison to Unity:** Similar to Unity's `AssetDatabase` and `Resources.Load()`, but with explicit async handling.

## AssetServer Initialization

### Creating an AssetServer

```rust
use astrelis_assets::AssetServer;
use std::path::PathBuf;

// Create asset server with base path
let asset_server = AssetServer::new(PathBuf::from("assets"));
```

**Base Path:** All asset paths are relative to this directory.

### Using with Engine

The engine provides a shared `AssetServer`:

```rust
use astrelis::Engine;

let engine = Engine::builder()
    .add_plugins(DefaultPlugins)
    .build();

// Access asset server
let assets = engine.get::<AssetServer>().unwrap();
```

## Loading Assets

### Basic Loading

```rust
use astrelis_assets::{AssetServer, Handle};

// Load an image
let texture_handle: Handle<Texture> = assets.load("textures/player.png")?;

// Load a font
let font_handle: Handle<Font> = assets.load("fonts/roboto.ttf")?;

// Load a shader
let shader_handle: Handle<Shader> = assets.load("shaders/custom.wgsl")?;
```

**Key Points:**
- Loading is **async** - asset may not be ready immediately
- Returns `Handle<T>` - type-safe reference
- Path is relative to asset server's base path

### Checking Load Status

```rust
// Check if asset is loaded
if assets.is_loaded(&texture_handle) {
    println!("Texture is ready!");
} else {
    println!("Texture is still loading...");
}

// Get asset (returns None if not loaded)
if let Some(texture) = assets.get(&texture_handle) {
    // Use texture
}
```

### Blocking Wait for Assets

```rust
// Wait for asset to load (blocks current thread)
let texture = assets.load_sync("textures/player.png")?;
```

**Warning:** Only use `load_sync()` during initialization, never during gameplay.

### Loading Multiple Assets

```rust
let mut handles = Vec::new();

// Queue multiple loads
handles.push(assets.load::<Texture>("textures/player.png")?);
handles.push(assets.load::<Texture>("textures/enemy.png")?);
handles.push(assets.load::<Texture>("textures/boss.png")?);

// Wait for all to load
while !handles.iter().all(|h| assets.is_loaded(h)) {
    std::thread::sleep(Duration::from_millis(10));
}

println!("All textures loaded!");
```

## Handle System

### What is a Handle?

```rust
pub struct Handle<T> {
    id: HandleId,
    generation: u32,
    _phantom: PhantomData<T>,
}
```

**Components:**
- `id`: Unique identifier
- `generation`: Prevents stale references
- `_phantom`: Type information

### Generation Counter Safety

Generation counters prevent use-after-free:

```rust
// Load asset (generation = 0)
let handle = assets.load::<Texture>("texture.png")?;

// Use asset
let texture = assets.get(&handle).unwrap();

// Unload asset
assets.unload(&handle);

// Reload asset (generation = 1)
let new_handle = assets.load::<Texture>("texture.png")?;

// Old handle is invalid!
assert!(assets.get(&handle).is_none()); // Returns None

// New handle works
assert!(assets.get(&new_handle).is_some());
```

**Benefit:** Can't accidentally use freed memory.

### Handle Cloning

Handles are cheap to clone:

```rust
let handle1 = assets.load::<Texture>("texture.png")?;
let handle2 = handle1.clone(); // Cheap copy (just ID + generation)

// Both handles reference the same asset
assert_eq!(
    assets.get(&handle1).unwrap() as *const _,
    assets.get(&handle2).unwrap() as *const _
);
```

### Weak Handles (If Supported)

```rust
// Strong handle (keeps asset alive)
let strong_handle = assets.load::<Texture>("texture.png")?;

// Weak handle (doesn't prevent unloading)
let weak_handle = strong_handle.downgrade();

// Upgrade weak handle (returns None if asset was unloaded)
if let Some(strong) = weak_handle.upgrade() {
    let texture = assets.get(&strong).unwrap();
}
```

## Asset Types

### Built-in Asset Types

| Type | Extension | Use Case |
|------|-----------|----------|
| `Texture` | `.png`, `.jpg`, `.bmp` | Images, sprites |
| `Font` | `.ttf`, `.otf` | Text rendering |
| `Shader` | `.wgsl` | Custom shaders |
| `Audio` | `.wav`, `.mp3`, `.ogg` | Sound effects, music |
| `Scene` | `.scene` | Level data |

### Loading Textures

```rust
use astrelis_assets::Handle;
use astrelis_render::Texture;

let handle: Handle<Texture> = assets.load("textures/sprite.png")?;

// Wait for loading
while !assets.is_loaded(&handle) {
    std::thread::sleep(Duration::from_millis(10));
}

// Get texture
let texture = assets.get(&handle).unwrap();

// Use in rendering
ui.build(|root| {
    root.image(texture)
        .width(Length::px(256.0))
        .height(Length::px(256.0))
        .build();
});
```

### Loading Fonts

```rust
use astrelis_text::Font;

let font_handle: Handle<Font> = assets.load("fonts/roboto.ttf")?;

// Use in UI
ui.build(|root| {
    root.text("Hello, World!")
        .font(font_handle)
        .font_size(24.0)
        .build();
});
```

### Loading Shaders

```rust
let shader_handle: Handle<Shader> = assets.load("shaders/custom.wgsl")?;

// Get shader source
let shader = assets.get(&shader_handle).unwrap();

// Create shader module
let shader_module = graphics.device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("Custom Shader"),
    source: wgpu::ShaderSource::Wgsl(shader.source.clone().into()),
});
```

## Asset Lifecycle

### Load → Use → Unload

```rust
// 1. Load
let handle = assets.load::<Texture>("texture.png")?;

// 2. Wait for loading
while !assets.is_loaded(&handle) {
    std::thread::sleep(Duration::from_millis(10));
}

// 3. Use
let texture = assets.get(&handle).unwrap();
render_with_texture(texture);

// 4. Unload (when done)
assets.unload(&handle);
```

### Automatic Unloading

Assets are unloaded when no handles remain:

```rust
{
    let handle = assets.load::<Texture>("texture.png")?;
    // ... use asset
} // Handle dropped, asset unloaded (if no other handles exist)
```

### Preventing Unload

Keep a handle alive:

```rust
struct Game {
    // Asset stays loaded as long as Game exists
    player_texture: Handle<Texture>,
}

impl Game {
    fn new(assets: &AssetServer) -> Self {
        Self {
            player_texture: assets.load("textures/player.png").unwrap(),
        }
    }
}
```

### Manual Unload

```rust
// Force unload (even if handles exist)
assets.unload(&handle);

// Handle becomes invalid
assert!(assets.get(&handle).is_none());
```

## Asset Events

### AssetEvent Types

```rust
pub enum AssetEvent<T> {
    /// Asset finished loading
    Loaded { handle: Handle<T> },

    /// Asset was modified (hot reload)
    Modified { handle: Handle<T> },

    /// Asset was unloaded
    Unloaded { handle: Handle<T> },

    /// Asset failed to load
    Failed { path: PathBuf, error: String },
}
```

### Listening for Events

```rust
use astrelis_assets::AssetEvent;

// Drain events
for event in assets.drain_events::<Texture>() {
    match event {
        AssetEvent::Loaded { handle } => {
            println!("Texture loaded: {:?}", handle);
        }
        AssetEvent::Modified { handle } => {
            println!("Texture modified: {:?}", handle);
        }
        AssetEvent::Unloaded { handle } => {
            println!("Texture unloaded: {:?}", handle);
        }
        AssetEvent::Failed { path, error } => {
            eprintln!("Failed to load {}: {}", path.display(), error);
        }
    }
}
```

### Example: Loading Screen

```rust
struct LoadingScreen {
    textures_to_load: Vec<Handle<Texture>>,
}

impl LoadingScreen {
    fn update(&mut self, assets: &AssetServer) -> bool {
        // Process load events
        for event in assets.drain_events::<Texture>() {
            if let AssetEvent::Loaded { handle } = event {
                self.textures_to_load.retain(|h| h != &handle);
            }
        }

        // All loaded?
        self.textures_to_load.is_empty()
    }
}
```

## Error Handling

### Load Errors

```rust
use astrelis_assets::AssetError;

match assets.load::<Texture>("missing.png") {
    Ok(handle) => {
        // Handle will be invalid if load fails
        // Check with is_loaded() or drain_events()
    }
    Err(AssetError::NotFound) => {
        eprintln!("Asset file not found");
    }
    Err(AssetError::InvalidFormat) => {
        eprintln!("Asset format invalid");
    }
    Err(e) => {
        eprintln!("Asset load error: {}", e);
    }
}
```

### Graceful Fallback

```rust
// Try to load asset, fallback to default
let texture_handle = assets.load::<Texture>("textures/custom.png")
    .unwrap_or_else(|_| {
        // Use default texture
        assets.load::<Texture>("textures/default.png").unwrap()
    });
```

### Error Events

```rust
for event in assets.drain_events::<Texture>() {
    if let AssetEvent::Failed { path, error } = event {
        // Log error
        log::error!("Failed to load {}: {}", path.display(), error);

        // Show error UI
        show_error_message(&format!("Failed to load asset: {}", path.display()));
    }
}
```

## Asset Paths

### Relative Paths

```rust
// Relative to asset server base path
let handle = assets.load::<Texture>("textures/player.png")?;
// Loads from: {base_path}/textures/player.png
```

### Nested Directories

```rust
let handle = assets.load::<Texture>("characters/heroes/knight/idle.png")?;
```

### Path Conventions

```text
assets/
├── textures/
│   ├── characters/
│   ├── environment/
│   └── ui/
├── fonts/
│   └── roboto.ttf
├── shaders/
│   └── custom.wgsl
├── audio/
│   ├── music/
│   └── sfx/
└── scenes/
    └── level1.scene
```

## Performance Considerations

### Preloading

Load assets before they're needed:

```rust
// During level loading
fn load_level_assets(&mut self, assets: &AssetServer) {
    self.asset_handles.clear();

    // Preload all level assets
    for path in level_asset_paths {
        let handle = assets.load::<Texture>(path).unwrap();
        self.asset_handles.push(handle);
    }

    // Wait for all to load
    while !self.asset_handles.iter().all(|h| assets.is_loaded(h)) {
        std::thread::sleep(Duration::from_millis(10));
    }
}
```

### Lazy Loading

Only load when needed:

```rust
struct ResourceManager {
    texture_cache: HashMap<String, Handle<Texture>>,
}

impl ResourceManager {
    fn get_texture(&mut self, assets: &AssetServer, path: &str) -> Handle<Texture> {
        self.texture_cache.entry(path.to_string())
            .or_insert_with(|| assets.load(path).unwrap())
            .clone()
    }
}
```

### Unloading Unused Assets

```rust
// Unload assets from previous level
for handle in &old_level_assets {
    assets.unload(handle);
}
old_level_assets.clear();
```

### Memory Budget

Track memory usage:

```rust
struct AssetMemoryTracker {
    loaded_assets: Vec<(String, usize)>,
    budget: usize,
}

impl AssetMemoryTracker {
    fn check_budget(&self) -> bool {
        let total: usize = self.loaded_assets.iter().map(|(_, size)| size).sum();
        total <= self.budget
    }

    fn unload_least_recently_used(&mut self, assets: &AssetServer) {
        // Unload oldest asset
        if let Some((path, _)) = self.loaded_assets.first() {
            // ... unload logic
        }
    }
}
```

## Common Patterns

### Asset Manager

Centralize asset loading:

```rust
pub struct GameAssets {
    pub player_texture: Handle<Texture>,
    pub enemy_texture: Handle<Texture>,
    pub font: Handle<Font>,
}

impl GameAssets {
    pub fn load(assets: &AssetServer) -> Result<Self, AssetError> {
        Ok(Self {
            player_texture: assets.load("textures/player.png")?,
            enemy_texture: assets.load("textures/enemy.png")?,
            font: assets.load("fonts/game.ttf")?,
        })
    }

    pub fn is_loaded(&self, assets: &AssetServer) -> bool {
        assets.is_loaded(&self.player_texture)
            && assets.is_loaded(&self.enemy_texture)
            && assets.is_loaded(&self.font)
    }
}
```

### Resource Pool

Reuse assets across systems:

```rust
pub struct TexturePool {
    textures: HashMap<String, Handle<Texture>>,
}

impl TexturePool {
    pub fn get_or_load(&mut self, assets: &AssetServer, path: &str) -> Handle<Texture> {
        if let Some(handle) = self.textures.get(path) {
            handle.clone()
        } else {
            let handle = assets.load(path).unwrap();
            self.textures.insert(path.to_string(), handle.clone());
            handle
        }
    }
}
```

### Async Loading with Tokio

```rust
use tokio::task;

async fn load_assets_async(asset_server: &AssetServer) -> Vec<Handle<Texture>> {
    let paths = vec![
        "texture1.png",
        "texture2.png",
        "texture3.png",
    ];

    let mut handles = Vec::new();

    for path in paths {
        let handle = asset_server.load::<Texture>(path).unwrap();
        handles.push(handle);
    }

    // Wait for all to load (non-blocking)
    while !handles.iter().all(|h| asset_server.is_loaded(h)) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    handles
}
```

## Comparison to Unity

| Unity | Astrelis | Notes |
|-------|----------|-------|
| `Resources.Load<T>()` | `assets.load::<T>()` | Similar API |
| `AssetDatabase` | `AssetServer` | Central manager |
| `UnityEngine.Object` | `Handle<T>` | Asset reference |
| `AssetBundle` | N/A | Use custom loaders |
| `Resources.UnloadAsset()` | `assets.unload()` | Manual unload |
| Asset reference counting | Generation counters | Prevents use-after-free |

## Troubleshooting

### Asset Not Found

**Cause:** Path is incorrect or file doesn't exist.

**Fix:** Verify path relative to asset base directory:
```rust
// Check if file exists
let full_path = Path::new("assets").join("textures/player.png");
assert!(full_path.exists(), "Asset file not found: {:?}", full_path);
```

### Handle is Invalid

**Cause:** Asset was unloaded or generation mismatch.

**Fix:** Keep handle alive or reload asset:
```rust
if assets.get(&handle).is_none() {
    // Reload asset
    handle = assets.load("texture.png")?;
}
```

### Asset Never Loads

**Cause:** Async loader stuck or file format unsupported.

**Fix:** Check for Failed events:
```rust
for event in assets.drain_events::<Texture>() {
    if let AssetEvent::Failed { path, error } = event {
        eprintln!("Load failed: {} - {}", path.display(), error);
    }
}
```

### Memory Leak

**Cause:** Handles not dropped, assets never unloaded.

**Fix:** Explicitly unload or ensure handles are dropped:
```rust
// Clear all handles
asset_handles.clear();

// Or explicitly unload
for handle in &asset_handles {
    assets.unload(handle);
}
```

## Next Steps

- **Practice:** Try loading textures in the UI examples
- **Learn More:** [Hot Reload](hot-reload.md) for live asset updates
- **Advanced:** [Custom Loaders](custom-loaders.md) for custom asset types
- **Examples:** `image_loading`, `hot_reload_demo`

## See Also

- [Hot Reload](hot-reload.md) - Live asset updates
- [Custom Loaders](custom-loaders.md) - Loading custom formats
- API Reference: [`AssetServer`](../../api/astrelis-assets/struct.AssetServer.html)
- API Reference: [`Handle<T>`](../../api/astrelis-assets/struct.Handle.html)
