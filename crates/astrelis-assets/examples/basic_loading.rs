//! Basic asset loading example demonstrating the asset system.
//!
//! This example shows:
//! - Creating an asset server
//! - Registering custom loaders
//! - Loading assets synchronously
//! - Checking asset state
//! - Processing events

use std::io::Write;

use astrelis_assets::prelude::*;

/// A simple JSON config asset.
#[derive(Debug)]
struct JsonConfig {
    data: serde_json::Value,
}

impl Asset for JsonConfig {
    fn type_name() -> &'static str {
        "JsonConfig"
    }
}

/// Loader for JSON config files.
struct JsonConfigLoader;

impl AssetLoader for JsonConfigLoader {
    type Asset = JsonConfig;

    fn extensions(&self) -> &[&str] {
        &["json"]
    }

    fn load(&self, ctx: LoadContext<'_>) -> Result<Self::Asset, AssetError> {
        let data: serde_json::Value =
            serde_json::from_slice(ctx.bytes).map_err(|e| AssetError::LoaderError {
                path: ctx.source.display_path(),
                message: format!("JSON parse error: {}", e),
            })?;

        Ok(JsonConfig { data })
    }
}

/// A simple image asset (just stores dimensions and pixel count).
#[derive(Debug)]
struct SimpleImage {
    width: u32,
    height: u32,
    pixel_count: usize,
}

impl Asset for SimpleImage {
    fn type_name() -> &'static str {
        "SimpleImage"
    }
}

/// Loader for simple "image" files (fake format for demo).
struct SimpleImageLoader;

impl AssetLoader for SimpleImageLoader {
    type Asset = SimpleImage;

    fn extensions(&self) -> &[&str] {
        &["img"]
    }

    fn load(&self, ctx: LoadContext<'_>) -> Result<Self::Asset, AssetError> {
        // Simple format: first 8 bytes are width/height as u32 LE
        if ctx.bytes.len() < 8 {
            return Err(AssetError::LoaderError {
                path: ctx.source.display_path(),
                message: "Image file too small".to_string(),
            });
        }

        let width = u32::from_le_bytes([ctx.bytes[0], ctx.bytes[1], ctx.bytes[2], ctx.bytes[3]]);
        let height = u32::from_le_bytes([ctx.bytes[4], ctx.bytes[5], ctx.bytes[6], ctx.bytes[7]]);
        let pixel_count = ctx.bytes.len().saturating_sub(8) / 4; // Assume RGBA

        Ok(SimpleImage {
            width,
            height,
            pixel_count,
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for our test assets
    let temp_dir = tempfile::tempdir()?;
    let assets_path = temp_dir.path();

    println!("Created temp directory: {}", assets_path.display());

    // Create some test asset files
    create_test_assets(assets_path)?;

    // Create the asset server with the temp directory as base path
    let mut server = AssetServer::with_base_path(assets_path);

    // Register our loaders
    server.register_loader(JsonConfigLoader);
    server.register_loader(SimpleImageLoader);

    // Also register the built-in text loader
    server.register_loader(astrelis_assets::TextLoader);

    println!("\n=== Loading Assets ===\n");

    // Load assets synchronously
    let config_handle: Handle<JsonConfig> = server.load_sync("config.json")?;
    let readme_handle: Handle<String> = server.load_sync("readme.txt")?;
    let image_handle: Handle<SimpleImage> = server.load_sync("sprite.img")?;

    // Check states
    println!("Config ready: {}", server.is_ready(&config_handle));
    println!("Readme ready: {}", server.is_ready(&readme_handle));
    println!("Image ready: {}", server.is_ready(&image_handle));

    // Access the assets
    if let Some(config) = server.get(&config_handle) {
        println!("\nConfig data: {}", config.data);
    }

    if let Some(readme) = server.get(&readme_handle) {
        println!("\nReadme contents:\n{}", *readme);
    }

    if let Some(image) = server.get(&image_handle) {
        println!(
            "\nImage: {}x{} ({} pixels)",
            image.width, image.height, image.pixel_count
        );
    }

    // Process events
    println!("\n=== Events ===\n");
    for event in server.drain_events() {
        match event {
            AssetEvent::Created { version, .. } => {
                println!("Asset created (version {})", version);
            }
            AssetEvent::Modified { version, .. } => {
                println!("Asset modified (version {})", version);
            }
            AssetEvent::Removed { .. } => {
                println!("Asset removed");
            }
            AssetEvent::LoadFailed { error, .. } => {
                println!("Asset failed to load: {}", error);
            }
        }
    }

    // Demonstrate version tracking
    println!("\n=== Version Tracking ===\n");
    println!("Config version: {:?}", server.version(&config_handle));
    println!("Readme version: {:?}", server.version(&readme_handle));

    // Demonstrate inserting an asset directly
    println!("\n=== Direct Insert ===\n");
    let inline_config = JsonConfig {
        data: serde_json::json!({
            "name": "inline",
            "value": 42
        }),
    };
    let inline_handle = server.insert(AssetSource::memory("inline://config"), inline_config);
    println!(
        "Inserted inline config, ready: {}",
        server.is_ready(&inline_handle)
    );

    if let Some(config) = server.get(&inline_handle) {
        println!("Inline config: {}", config.data);
    }

    println!("\n=== Done ===");

    Ok(())
}

fn create_test_assets(path: &std::path::Path) -> std::io::Result<()> {
    // Create config.json
    let config_path = path.join("config.json");
    let mut config_file = std::fs::File::create(&config_path)?;
    writeln!(
        config_file,
        r#"{{
    "game_name": "Astrelis Demo",
    "version": "1.0.0",
    "settings": {{
        "fullscreen": false,
        "vsync": true,
        "volume": 0.8
    }}
}}"#
    )?;
    println!("Created: {}", config_path.display());

    // Create readme.txt
    let readme_path = path.join("readme.txt");
    let mut readme_file = std::fs::File::create(&readme_path)?;
    writeln!(readme_file, "Welcome to the Astrelis Asset System!")?;
    writeln!(readme_file, "This is a simple text asset loaded from disk.")?;
    println!("Created: {}", readme_path.display());

    // Create sprite.img (fake image format)
    let image_path = path.join("sprite.img");
    let mut image_file = std::fs::File::create(&image_path)?;
    // Write width (64) and height (64) as u32 LE
    image_file.write_all(&64u32.to_le_bytes())?;
    image_file.write_all(&64u32.to_le_bytes())?;
    // Write some fake RGBA pixel data
    let pixels: Vec<u8> = (0..64 * 64 * 4).map(|i| (i % 256) as u8).collect();
    image_file.write_all(&pixels)?;
    println!("Created: {}", image_path.display());

    Ok(())
}
