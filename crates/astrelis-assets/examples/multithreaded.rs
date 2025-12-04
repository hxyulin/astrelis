//! Multithreaded asset loading example.
//!
//! This example demonstrates:
//! - Loading assets from multiple threads
//! - Thread-safe handle sharing
//! - Concurrent asset access
//! - Using channels for async notification

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use astrelis_assets::prelude::*;

/// A game configuration asset.
#[derive(Debug, Clone)]
struct GameConfig {
    name: String,
    max_entities: u32,
}

impl Asset for GameConfig {
    fn type_name() -> &'static str {
        "GameConfig"
    }
}

/// Loader for game config files (simple key=value format).
struct GameConfigLoader;

impl AssetLoader for GameConfigLoader {
    type Asset = GameConfig;

    fn extensions(&self) -> &[&str] {
        &["cfg", "config"]
    }

    fn load(&self, ctx: LoadContext<'_>) -> Result<Self::Asset, AssetError> {
        let text = std::str::from_utf8(ctx.bytes).map_err(|e| AssetError::LoaderError {
            path: ctx.source.display_path(),
            message: format!("Invalid UTF-8: {}", e),
        })?;

        let mut name = String::from("Unknown");
        let mut max_entities = 1000;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "name" => name = value.to_string(),
                    "max_entities" => {
                        max_entities = value.parse().unwrap_or(1000);
                    }
                    _ => {}
                }
            }
        }

        // Simulate some loading work
        thread::sleep(Duration::from_millis(50));

        Ok(GameConfig { name, max_entities })
    }
}

/// A level data asset.
#[derive(Debug, Clone)]
struct LevelData {
    id: u32,
    width: u32,
    height: u32,
    tile_count: usize,
}

impl Asset for LevelData {
    fn type_name() -> &'static str {
        "LevelData"
    }
}

/// Loader for level files.
struct LevelLoader;

impl AssetLoader for LevelLoader {
    type Asset = LevelData;

    fn extensions(&self) -> &[&str] {
        &["level", "lvl"]
    }

    fn load(&self, ctx: LoadContext<'_>) -> Result<Self::Asset, AssetError> {
        // Simple binary format: id (u32), width (u32), height (u32), then tiles
        if ctx.bytes.len() < 12 {
            return Err(AssetError::LoaderError {
                path: ctx.source.display_path(),
                message: "Level file too small".to_string(),
            });
        }

        let id = u32::from_le_bytes([ctx.bytes[0], ctx.bytes[1], ctx.bytes[2], ctx.bytes[3]]);
        let width = u32::from_le_bytes([ctx.bytes[4], ctx.bytes[5], ctx.bytes[6], ctx.bytes[7]]);
        let height = u32::from_le_bytes([ctx.bytes[8], ctx.bytes[9], ctx.bytes[10], ctx.bytes[11]]);
        let tile_count = ctx.bytes.len().saturating_sub(12);

        // Simulate loading work
        thread::sleep(Duration::from_millis(100));

        Ok(LevelData {
            id,
            width,
            height,
            tile_count,
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Multithreaded Asset Loading Example ===\n");

    // Create temp directory with test assets
    let temp_dir = tempfile::tempdir()?;
    let assets_path = temp_dir.path();
    create_test_assets(assets_path)?;

    // Create the asset server wrapped in Arc<Mutex> for thread safety
    let server = Arc::new(Mutex::new({
        let mut server = AssetServer::with_base_path(assets_path);
        server.register_loader(GameConfigLoader);
        server.register_loader(LevelLoader);
        server.register_loader(astrelis_assets::TextLoader);
        server
    }));

    // Channel to collect results from worker threads
    let (tx, rx) = std::sync::mpsc::channel::<String>();

    println!("Spawning worker threads...\n");

    // Spawn multiple threads to load different assets
    let mut handles = Vec::new();

    // Thread 1: Load game config
    {
        let server = Arc::clone(&server);
        let tx = tx.clone();
        handles.push(thread::spawn(move || {
            let thread_id = thread::current().id();
            tx.send(format!("[{:?}] Starting config load...", thread_id))
                .unwrap();

            let handle: Handle<GameConfig> = {
                let mut server = server.lock().unwrap();
                server.load_sync("game.cfg").expect("Failed to load config")
            };

            // Access the loaded asset
            let server = server.lock().unwrap();
            if let Some(config) = server.get(&handle) {
                tx.send(format!(
                    "[{:?}] Loaded config: name='{}', max_entities={}",
                    thread_id, config.name, config.max_entities
                ))
                .unwrap();
            }
        }));
    }

    // Thread 2: Load level 1
    {
        let server = Arc::clone(&server);
        let tx = tx.clone();
        handles.push(thread::spawn(move || {
            let thread_id = thread::current().id();
            tx.send(format!("[{:?}] Starting level1 load...", thread_id))
                .unwrap();

            let handle: Handle<LevelData> = {
                let mut server = server.lock().unwrap();
                server.load_sync("level1.lvl").expect("Failed to load level1")
            };

            let server = server.lock().unwrap();
            if let Some(level) = server.get(&handle) {
                tx.send(format!(
                    "[{:?}] Loaded level1: id={}, {}x{}, {} tiles",
                    thread_id, level.id, level.width, level.height, level.tile_count
                ))
                .unwrap();
            }
        }));
    }

    // Thread 3: Load level 2
    {
        let server = Arc::clone(&server);
        let tx = tx.clone();
        handles.push(thread::spawn(move || {
            let thread_id = thread::current().id();
            tx.send(format!("[{:?}] Starting level2 load...", thread_id))
                .unwrap();

            let handle: Handle<LevelData> = {
                let mut server = server.lock().unwrap();
                server.load_sync("level2.lvl").expect("Failed to load level2")
            };

            let server = server.lock().unwrap();
            if let Some(level) = server.get(&handle) {
                tx.send(format!(
                    "[{:?}] Loaded level2: id={}, {}x{}, {} tiles",
                    thread_id, level.id, level.width, level.height, level.tile_count
                ))
                .unwrap();
            }
        }));
    }

    // Thread 4: Load multiple text files
    {
        let server = Arc::clone(&server);
        let tx = tx.clone();
        handles.push(thread::spawn(move || {
            let thread_id = thread::current().id();
            tx.send(format!("[{:?}] Starting text files load...", thread_id))
                .unwrap();

            let files = ["readme.txt", "notes.txt"];
            for file in files {
                let handle: Handle<String> = {
                    let mut server = server.lock().unwrap();
                    server.load_sync(file).expect("Failed to load text")
                };

                let server = server.lock().unwrap();
                if let Some(text) = server.get(&handle) {
                    let preview: String = text.chars().take(50).collect();
                    tx.send(format!(
                        "[{:?}] Loaded '{}': \"{}...\"",
                        thread_id, file, preview
                    ))
                    .unwrap();
                }
            }
        }));
    }

    // Drop the original sender so rx.iter() will complete
    drop(tx);

    // Print messages as they arrive
    for msg in rx {
        println!("{}", msg);
    }

    // Wait for all threads to complete
    println!("\nWaiting for threads to complete...");
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Print final statistics
    println!("\n=== Final Statistics ===\n");
    let server = server.lock().unwrap();

    // Count assets by checking storages
    if let Some(configs) = server.assets::<GameConfig>() {
        println!("GameConfig assets: {}", configs.len());
    }
    if let Some(levels) = server.assets::<LevelData>() {
        println!("LevelData assets: {}", levels.len());
    }
    if let Some(texts) = server.assets::<String>() {
        println!("String assets: {}", texts.len());
    }

    println!("\n=== Done ===");

    Ok(())
}

fn create_test_assets(path: &std::path::Path) -> std::io::Result<()> {
    // Create game.cfg
    let config_path = path.join("game.cfg");
    let mut file = std::fs::File::create(&config_path)?;
    writeln!(file, "# Game Configuration")?;
    writeln!(file, "name = Astrelis Demo")?;
    writeln!(file, "max_entities = 5000")?;
    println!("Created: {}", config_path.display());

    // Create level1.lvl (binary)
    let level1_path = path.join("level1.lvl");
    let mut file = std::fs::File::create(&level1_path)?;
    file.write_all(&1u32.to_le_bytes())?; // id
    file.write_all(&64u32.to_le_bytes())?; // width
    file.write_all(&48u32.to_le_bytes())?; // height
    file.write_all(&vec![0u8; 64 * 48])?; // tiles
    println!("Created: {}", level1_path.display());

    // Create level2.lvl (binary)
    let level2_path = path.join("level2.lvl");
    let mut file = std::fs::File::create(&level2_path)?;
    file.write_all(&2u32.to_le_bytes())?; // id
    file.write_all(&128u32.to_le_bytes())?; // width
    file.write_all(&96u32.to_le_bytes())?; // height
    file.write_all(&vec![1u8; 128 * 96])?; // tiles
    println!("Created: {}", level2_path.display());

    // Create readme.txt
    let readme_path = path.join("readme.txt");
    let mut file = std::fs::File::create(&readme_path)?;
    writeln!(file, "Welcome to the Astrelis Engine!")?;
    writeln!(file, "This demonstrates multithreaded asset loading.")?;
    println!("Created: {}", readme_path.display());

    // Create notes.txt
    let notes_path = path.join("notes.txt");
    let mut file = std::fs::File::create(&notes_path)?;
    writeln!(file, "Developer Notes:")?;
    writeln!(file, "- Assets are loaded in parallel")?;
    writeln!(file, "- Thread-safe handle sharing")?;
    println!("Created: {}", notes_path.display());

    println!();
    Ok(())
}
