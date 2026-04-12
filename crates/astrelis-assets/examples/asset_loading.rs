//! Demonstrates basic asset loading using `load_from_bytes`.
//!
//! Run with: `cargo run -p astrelis-assets --example asset_loading`

use std::path::Path;
use std::sync::Arc;

use astrelis_assets::{Asset, AssetLoadError, AssetLoader, AssetServer};

/// A minimal asset that holds UTF-8 text content.
struct TextAsset {
    content: String,
}

impl Asset for TextAsset {
    fn type_name() -> &'static str {
        "TextAsset"
    }
}

/// Loader that converts raw bytes into a [`TextAsset`].
struct TextLoader;

impl AssetLoader for TextLoader {
    type Asset = TextAsset;

    fn extensions(&self) -> &[&str] {
        &["txt"]
    }

    fn load(&self, bytes: &[u8], _path: &Path) -> Result<Self::Asset, AssetLoadError> {
        let content = String::from_utf8(bytes.to_vec())
            .map_err(|e| AssetLoadError::Parse(e.to_string()))?;
        Ok(TextAsset { content })
    }
}

fn main() {
    astrelis_profiling::init();
    astrelis_profiling::set_thread_name("main");
    astrelis_core::logging::init_default();

    // Create a server and register our text loader.
    astrelis_profiling::profile_scope!("setup");
    let mut server = AssetServer::new("assets");
    server.add_loader(TextLoader);

    // Load an asset directly from bytes (no file I/O required).
    {
        astrelis_profiling::profile_scope!("load_from_bytes");
        let handle = server
            .load_from_bytes::<TextAsset>(b"Hello from astrelis-assets!", "greeting.txt")
            .expect("failed to load from bytes");

        // The asset is immediately available after `load_from_bytes`.
        let asset: Arc<TextAsset> = server.get(&handle).expect("asset should be loaded");
        tracing::info!("Loaded asset: {}", asset.content);
    }

    // Attempting to load with a missing loader produces a clear error.
    let err = server
        .load_from_bytes::<TextAsset>(b"data", "image.png")
        .unwrap_err();
    tracing::info!("Expected error: {err}");

    astrelis_profiling::new_frame();
    astrelis_profiling::finish();
}
