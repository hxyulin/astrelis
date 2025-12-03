# astrelis-assets

The `astrelis-assets` crate provides a basic asset management system.

## Features

- **AssetManager**: Central storage for assets.
- **Handle**: Type-safe handle to an asset, using generational indices.

## Usage

```rust
// Conceptual usage
let mut assets = AssetManager::new();
let handle: Handle<Texture> = assets.load("texture.png");
```

## Modules

- `AssetManager`: Stores assets by type.
- `Handle`: A reference to an asset.
