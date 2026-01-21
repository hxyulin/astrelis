# Custom Asset Loaders

This guide explains how to create custom asset loaders in Astrelis for loading game-specific data formats, configuration files, and procedural content.

## Overview

**Custom loaders** enable:

- Loading custom file formats (`.level`, `.enemy`, etc.)
- Parsing configuration files (JSON, TOML, RON)
- Procedural asset generation
- Custom asset processing pipelines
- Integration with external tools

**Key Component:** `AssetLoader` trait

**Comparison to Unity:** Similar to Unity's `ScriptedImporter` or custom asset post-processors.

## AssetLoader Trait

```rust
use async_trait::async_trait;
use std::path::Path;

#[async_trait]
pub trait AssetLoader: Send + Sync + 'static {
    /// The asset type this loader produces
    type Asset: Send + Sync + 'static;

    /// File extensions this loader handles
    fn extensions(&self) -> &[&str];

    /// Load asset from file
    async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError>;
}
```

**Key Methods:**
- `extensions()`: File extensions handled (e.g., `["json"]`)
- `load()`: Parse bytes into asset type

## Simple Example: JSON Config Loader

### Step 1: Define Asset Type

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub player_speed: f32,
    pub enemy_count: u32,
    pub difficulty: String,
    pub debug_mode: bool,
}
```

### Step 2: Implement Loader

```rust
use astrelis_assets::{AssetLoader, AssetLoadError};
use async_trait::async_trait;
use std::path::Path;

pub struct GameConfigLoader;

#[async_trait]
impl AssetLoader for GameConfigLoader {
    type Asset = GameConfig;

    fn extensions(&self) -> &[&str] {
        &["json"]
    }

    async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
        // Parse JSON
        let config: GameConfig = serde_json::from_slice(bytes)
            .map_err(|e| AssetLoadError::ParseError(e.to_string()))?;

        Ok(config)
    }
}
```

### Step 3: Register Loader

```rust
use astrelis_assets::AssetServer;

let mut asset_server = AssetServer::new(PathBuf::from("assets"));

// Register custom loader
asset_server.register_loader(GameConfigLoader);
```

### Step 4: Load Assets

```rust
// Create config file: assets/game_config.json
// {
//   "player_speed": 5.0,
//   "enemy_count": 10,
//   "difficulty": "normal",
//   "debug_mode": false
// }

// Load config
let config_handle: Handle<GameConfig> = asset_server.load("game_config.json")?;

// Use config
if let Some(config) = asset_server.get(&config_handle) {
    println!("Player speed: {}", config.player_speed);
    println!("Enemy count: {}", config.enemy_count);
}
```

## Complete Example: Level Data Loader

### Custom Level Format

```rust
use serde::{Deserialize, Serialize};
use glam::Vec2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelData {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub spawn_points: Vec<Vec2>,
    pub enemies: Vec<EnemySpawn>,
    pub background_music: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnemySpawn {
    pub enemy_type: String,
    pub position: Vec2,
    pub patrol_points: Vec<Vec2>,
}
```

### Level Loader

```rust
use astrelis_assets::{AssetLoader, AssetLoadError};
use async_trait::async_trait;

pub struct LevelLoader;

#[async_trait]
impl AssetLoader for LevelLoader {
    type Asset = LevelData;

    fn extensions(&self) -> &[&str] {
        &["level", "lvl"]
    }

    async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
        // Parse RON format (Rusty Object Notation)
        let level: LevelData = ron::de::from_bytes(bytes)
            .map_err(|e| AssetLoadError::ParseError(format!("RON parse error: {}", e)))?;

        // Validate level data
        if level.spawn_points.is_empty() {
            return Err(AssetLoadError::InvalidData(
                "Level must have at least one spawn point".to_string()
            ));
        }

        // Log loading
        log::info!("Loaded level: {} ({}x{})", level.name, level.width, level.height);

        Ok(level)
    }
}
```

### Example Level File (`level1.level`)

```ron
(
    name: "Forest Entrance",
    width: 800,
    height: 600,
    spawn_points: [
        (x: 100.0, y: 300.0),
    ],
    enemies: [
        (
            enemy_type: "goblin",
            position: (x: 400.0, y: 300.0),
            patrol_points: [
                (x: 350.0, y: 300.0),
                (x: 450.0, y: 300.0),
            ],
        ),
    ],
    background_music: "forest_theme.ogg",
)
```

### Usage

```rust
// Register loader
asset_server.register_loader(LevelLoader);

// Load level
let level_handle: Handle<LevelData> = asset_server.load("levels/level1.level")?;

// Spawn level
if let Some(level) = asset_server.get(&level_handle) {
    spawn_level(level);
}
```

## Async Loading with Dependencies

Load dependencies within the loader:

```rust
use astrelis_assets::{AssetLoader, AssetServer, Handle};

pub struct MaterialLoader {
    asset_server: Arc<AssetServer>,
}

#[async_trait]
impl AssetLoader for MaterialLoader {
    type Asset = Material;

    fn extensions(&self) -> &[&str] {
        &["mat"]
    }

    async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
        // Parse material description
        let desc: MaterialDesc = serde_json::from_slice(bytes)?;

        // Load dependent textures
        let albedo_handle = self.asset_server.load(&desc.albedo_path)?;
        let normal_handle = self.asset_server.load(&desc.normal_path)?;

        // Wait for textures to load
        while !self.asset_server.is_loaded(&albedo_handle)
           || !self.asset_server.is_loaded(&normal_handle) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Create material
        Ok(Material {
            albedo: albedo_handle,
            normal: normal_handle,
            roughness: desc.roughness,
            metallic: desc.metallic,
        })
    }
}
```

## Procedural Asset Generation

Generate assets algorithmically:

```rust
pub struct ProceduralTextureLoader;

#[async_trait]
impl AssetLoader for ProceduralTextureLoader {
    type Asset = Texture;

    fn extensions(&self) -> &[&str] {
        &["proctex"]
    }

    async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
        // Parse procedural texture description
        let desc: ProceduralTextureDesc = serde_json::from_slice(bytes)?;

        // Generate texture data
        let texture_data = match desc.generator_type.as_str() {
            "checkerboard" => generate_checkerboard(&desc),
            "perlin_noise" => generate_perlin_noise(&desc),
            "voronoi" => generate_voronoi(&desc),
            _ => return Err(AssetLoadError::UnsupportedFormat(desc.generator_type)),
        };

        // Create GPU texture
        let texture = create_texture_from_rgba(
            &graphics.device,
            &graphics.queue,
            desc.width,
            desc.height,
            &texture_data,
        );

        Ok(texture)
    }
}

fn generate_checkerboard(desc: &ProceduralTextureDesc) -> Vec<u8> {
    let mut data = vec![0u8; (desc.width * desc.height * 4) as usize];

    for y in 0..desc.height {
        for x in 0..desc.width {
            let checker_x = (x / desc.checker_size) % 2;
            let checker_y = (y / desc.checker_size) % 2;
            let is_white = (checker_x + checker_y) % 2 == 0;

            let color = if is_white { 255 } else { 0 };
            let index = ((y * desc.width + x) * 4) as usize;

            data[index] = color;
            data[index + 1] = color;
            data[index + 2] = color;
            data[index + 3] = 255;
        }
    }

    data
}
```

## Binary Format Loader

Parse custom binary formats:

```rust
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

pub struct CustomMeshLoader;

#[async_trait]
impl AssetLoader for CustomMeshLoader {
    type Asset = Mesh;

    fn extensions(&self) -> &[&str] {
        &["mesh"]
    }

    async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
        let mut cursor = Cursor::new(bytes);

        // Read header
        let magic = cursor.read_u32::<LittleEndian>()?;
        if magic != 0x4853454D { // "MESH"
            return Err(AssetLoadError::InvalidData("Invalid magic number".to_string()));
        }

        let version = cursor.read_u32::<LittleEndian>()?;
        let vertex_count = cursor.read_u32::<LittleEndian>()?;
        let index_count = cursor.read_u32::<LittleEndian>()?;

        // Read vertices
        let mut vertices = Vec::with_capacity(vertex_count as usize);
        for _ in 0..vertex_count {
            let x = cursor.read_f32::<LittleEndian>()?;
            let y = cursor.read_f32::<LittleEndian>()?;
            let z = cursor.read_f32::<LittleEndian>()?;

            let nx = cursor.read_f32::<LittleEndian>()?;
            let ny = cursor.read_f32::<LittleEndian>()?;
            let nz = cursor.read_f32::<LittleEndian>()?;

            let u = cursor.read_f32::<LittleEndian>()?;
            let v = cursor.read_f32::<LittleEndian>()?;

            vertices.push(Vertex {
                position: [x, y, z],
                normal: [nx, ny, nz],
                uv: [u, v],
            });
        }

        // Read indices
        let mut indices = Vec::with_capacity(index_count as usize);
        for _ in 0..index_count {
            indices.push(cursor.read_u32::<LittleEndian>()?);
        }

        Ok(Mesh { vertices, indices })
    }
}
```

## Error Handling

### Custom Error Types

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AssetLoadError {
    #[error("File not found: {0}")]
    NotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Dependency missing: {0}")]
    MissingDependency(String),
}
```

### Graceful Error Handling

```rust
async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
    // Try primary format
    if let Ok(asset) = serde_json::from_slice::<MyAsset>(bytes) {
        return Ok(asset);
    }

    // Fallback to legacy format
    if let Ok(asset) = parse_legacy_format(bytes) {
        log::warn!("Loaded legacy format for {}, consider upgrading", path.display());
        return Ok(asset);
    }

    // All parsing failed
    Err(AssetLoadError::ParseError(
        "Could not parse asset in any known format".to_string()
    ))
}
```

## Post-Processing

Transform assets after loading:

```rust
pub struct ImageLoader {
    graphics: Arc<GraphicsContext>,
}

#[async_trait]
impl AssetLoader for ImageLoader {
    type Asset = Texture;

    fn extensions(&self) -> &[&str] {
        &["png", "jpg", "jpeg"]
    }

    async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
        // Load image
        let img = image::load_from_memory(bytes)
            .map_err(|e| AssetLoadError::ParseError(e.to_string()))?;

        // Post-process: resize if too large
        let img = if img.width() > 2048 || img.height() > 2048 {
            log::warn!("Resizing large texture: {}", path.display());
            img.resize(2048, 2048, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        // Post-process: convert to RGBA8
        let rgba = img.to_rgba8();

        // Upload to GPU
        let texture = upload_texture_to_gpu(
            &self.graphics.device,
            &self.graphics.queue,
            &rgba,
            rgba.width(),
            rgba.height(),
        );

        Ok(texture)
    }
}
```

## Caching and Optimization

### Deduplication

Asset server automatically deduplicates by path:

```rust
// Both loads return same handle
let handle1 = assets.load::<Texture>("texture.png")?;
let handle2 = assets.load::<Texture>("texture.png")?;

assert_eq!(handle1, handle2);
```

### Loader State

Loaders can maintain state:

```rust
pub struct AtlasTextureLoader {
    atlas_cache: Arc<Mutex<HashMap<String, Arc<Atlas>>>>,
}

#[async_trait]
impl AssetLoader for AtlasTextureLoader {
    type Asset = Texture;

    fn extensions(&self) -> &[&str] {
        &["atlas"]
    }

    async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
        // Check cache
        let cache_key = path.to_string_lossy().to_string();
        {
            let cache = self.atlas_cache.lock().unwrap();
            if let Some(atlas) = cache.get(&cache_key) {
                return Ok(Texture::from_atlas(atlas.clone()));
            }
        }

        // Load and cache
        let atlas = Arc::new(parse_atlas(bytes)?);
        self.atlas_cache.lock().unwrap().insert(cache_key, atlas.clone());

        Ok(Texture::from_atlas(atlas))
    }
}
```

## Testing Loaders

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_game_config_loader() {
        let loader = GameConfigLoader;

        let json = r#"{
            "player_speed": 5.0,
            "enemy_count": 10,
            "difficulty": "normal",
            "debug_mode": false
        }"#;

        let config = loader.load(Path::new("test.json"), json.as_bytes())
            .await
            .unwrap();

        assert_eq!(config.player_speed, 5.0);
        assert_eq!(config.enemy_count, 10);
        assert_eq!(config.difficulty, "normal");
        assert!(!config.debug_mode);
    }

    #[tokio::test]
    async fn test_invalid_json() {
        let loader = GameConfigLoader;

        let result = loader.load(Path::new("test.json"), b"invalid json")
            .await;

        assert!(result.is_err());
    }
}
```

## Best Practices

### ✅ DO: Validate Input

```rust
async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
    let config: GameConfig = serde_json::from_slice(bytes)?;

    // Validate
    if config.player_speed <= 0.0 {
        return Err(AssetLoadError::InvalidData("Player speed must be positive".to_string()));
    }

    Ok(config)
}
```

### ✅ DO: Provide Helpful Errors

```rust
Err(AssetLoadError::ParseError(format!(
    "Failed to parse {} at line {}: {}",
    path.display(),
    line_number,
    error_message
)))
```

### ✅ DO: Log Loading Info

```rust
log::info!("Loaded level: {} ({} enemies, {} items)",
    level.name,
    level.enemies.len(),
    level.items.len()
);
```

### ❌ DON'T: Block on I/O

```rust
// BAD: Synchronous file read
async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
    let additional_data = std::fs::read("other_file.txt")?; // DON'T!
}

// GOOD: Use provided bytes
async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
    // All data is in bytes parameter
}
```

### ❌ DON'T: Panic on Errors

```rust
// BAD
async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
    let config: GameConfig = serde_json::from_slice(bytes).unwrap(); // DON'T!
}

// GOOD
async fn load(&self, path: &Path, bytes: &[u8]) -> Result<Self::Asset, AssetLoadError> {
    let config: GameConfig = serde_json::from_slice(bytes)
        .map_err(|e| AssetLoadError::ParseError(e.to_string()))?;
}
```

## Comparison to Unity

| Unity | Astrelis | Notes |
|-------|----------|-------|
| `ScriptedImporter` | `AssetLoader` | Similar concept |
| `AssetPostprocessor` | Post-processing in `load()` | Custom processing |
| Asset bundles | Custom loaders | Flexible format support |
| `OnPostprocessAllAssets` | AssetEvent::Modified | Hot reload integration |

## Troubleshooting

### Loader Not Called

**Cause:** Loader not registered or wrong extension.

**Fix:** Verify registration and extension:
```rust
asset_server.register_loader(MyLoader);
// Loader handles: ["myext"]
let handle = assets.load::<MyAsset>("file.myext")?; // Must match
```

### Parse Errors

**Cause:** Invalid file format or corrupted data.

**Fix:** Add detailed error messages:
```rust
.map_err(|e| AssetLoadError::ParseError(
    format!("Failed to parse {}: {}", path.display(), e)
))
```

### Dependencies Not Loaded

**Cause:** Async dependencies not awaited.

**Fix:** Wait for dependencies:
```rust
while !asset_server.is_loaded(&dependency_handle) {
    tokio::time::sleep(Duration::from_millis(10)).await;
}
```

## Next Steps

- **Practice:** Create a custom loader for your game's level format
- **Integration:** [Loading Assets](loading-assets.md) for usage patterns
- **Advanced:** [Hot Reload](hot-reload.md) for live editing custom formats
- **Examples:** `custom_asset_loader`

## See Also

- [Loading Assets](loading-assets.md) - Asset loading basics
- [Hot Reload](hot-reload.md) - Live asset updates
- API Reference: [`AssetLoader`](../../api/astrelis-assets/trait.AssetLoader.html)
- API Reference: [`AssetLoadError`](../../api/astrelis-assets/enum.AssetLoadError.html)
