# astrelis-assets

Type-safe async asset management for the Astrelis engine.

This is a Layer 2 crate providing generic asset loading, handle-based tracking,
and optional hot-reload. It depends only on `astrelis-core` and has no platform
or GPU dependencies — loaders for specific formats (textures, fonts, etc.) are
implemented in consuming crates.

## Architecture

```
AssetServer
 ├── LoaderRegistry ── maps (TypeId, extension) → ErasedAssetLoader
 ├── StorageMap ────── maps TypeId → Assets<T> (type-erased)
 ├── PathMap ───────── deduplicates loads by path
 └── Background Thread ── reads files, invokes loaders, sends results
```

## Modules

| Module | Description |
|--------|-------------|
| `handle` | `Handle<T>`, `WeakHandle<T>`, `UntypedHandle` — reference-counted asset references |
| `loader` | `AssetLoader` trait and internal `LoaderRegistry` |
| `storage` | Type-erased per-type asset storage |
| `event` | `LoadState` and `AssetEvent` for tracking load lifecycle |
| `server` | `AssetServer` — the central coordinator |

## Usage

```rust,no_run
use astrelis_assets::{Asset, AssetLoader, AssetServer, AssetLoadError};
use std::path::Path;

// 1. Define your asset type.
struct TextAsset { content: String }

impl Asset for TextAsset {
    fn type_name() -> &'static str { "TextAsset" }
}

// 2. Define a loader.
struct TextLoader;

impl AssetLoader for TextLoader {
    type Asset = TextAsset;
    fn extensions(&self) -> &[&str] { &["txt"] }
    fn load(&self, bytes: &[u8], _path: &Path) -> Result<Self::Asset, AssetLoadError> {
        let content = String::from_utf8(bytes.to_vec())
            .map_err(|e| AssetLoadError::Parse(e.to_string()))?;
        Ok(TextAsset { content })
    }
}

// 3. Set up the server.
let mut server = AssetServer::new("assets");
server.add_loader(TextLoader);

// 4. Load assets (returns immediately, loads in background).
let handle = server.load::<TextAsset>("hello.txt");

// 5. Process loads each frame.
let events = server.update();

// 6. Access loaded assets.
if let Some(asset) = server.get(&handle) {
    println!("{}", asset.content);
}
```

## Features

| Feature | Description |
|---------|-------------|
| `hot-reload` | Enables automatic file watching and asset reloading via the `notify` crate |

## License

MIT
