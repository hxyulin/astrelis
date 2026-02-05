//! Basic example demonstrating the Astrelis engine with the plugin system.
//!
//! This example shows how to:
//! - Create an Engine with plugins
//! - Use the MinimalPlugins or DefaultPlugins
//! - Add custom plugins
//! - Access resources from plugins
//!
//! Run with: cargo run -p astrelis --example basic_engine

use astrelis::prelude::*;

// Define a custom plugin
struct GameConfigPlugin {
    title: String,
    version: String,
}

impl GameConfigPlugin {
    fn new(title: &str, version: &str) -> Self {
        Self {
            title: title.to_string(),
            version: version.to_string(),
        }
    }
}

// A resource that the plugin will register
pub struct GameConfig {
    pub title: String,
    pub version: String,
    pub debug_mode: bool,
}

impl Plugin for GameConfigPlugin {
    type Dependencies = ();

    fn name(&self) -> &'static str {
        "GameConfigPlugin"
    }

    fn build(&self, resources: &mut Resources) {
        resources.insert(GameConfig {
            title: self.title.clone(),
            version: self.version.clone(),
            debug_mode: false,
        });

        println!("GameConfigPlugin: Registered GameConfig resource");
    }
}

// Another plugin that depends on the first
struct DebugPlugin;

impl Plugin for DebugPlugin {
    type Dependencies = GameConfigPlugin;

    fn name(&self) -> &'static str {
        "DebugPlugin"
    }

    fn build(&self, resources: &mut Resources) {
        // Access and modify the GameConfig
        if let Some(config) = resources.get_mut::<GameConfig>() {
            config.debug_mode = true;
            println!("DebugPlugin: Enabled debug mode for {}", config.title);
        }
    }
}

fn main() {
    println!("=== Example 1: Building engine with custom plugins ===\n");

    let engine = Engine::builder()
        .add_plugin(GameConfigPlugin::new("My Game", "1.0.0"))
        .add_plugin(DebugPlugin)
        .build();

    // Access the registered resources
    if let Some(config) = engine.get::<GameConfig>() {
        println!("Game Title: {}", config.title);
        println!("Game Version: {}", config.version);
        println!("Debug Mode: {}", config.debug_mode);
    }

    println!("\n=== Example 2: Using MinimalPlugins ===\n");

    let engine = Engine::builder().add_plugins(MinimalPlugins).build();

    // Check that AssetPlugin was registered
    assert!(engine.has_plugin("AssetPlugin"));
    println!(
        "AssetPlugin is registered: {}",
        engine.has_plugin("AssetPlugin")
    );

    // List all registered plugins
    println!("Registered plugins:");
    for name in engine.plugin_names() {
        println!("  - {}", name);
    }

    println!("\n=== Example 3: Using FnPlugin for quick setup ===\n");

    let engine = Engine::builder()
        .add_plugin(FnPlugin::new("QuickSetup", |resources| {
            resources.insert(42i32);
            resources.insert("Hello, World!".to_string());
        }))
        .build();

    println!("Integer resource: {:?}", engine.get::<i32>());
    println!("String resource: {:?}", engine.get::<String>());

    println!("\n=== Example 4: Pre-inserting resources ===\n");

    let engine = Engine::builder()
        .insert_resource(vec!["item1", "item2", "item3"])
        .add_plugin(FnPlugin::new("ItemCounter", |resources| {
            if let Some(items) = resources.get::<Vec<&'static str>>() {
                println!("Found {} items", items.len());
            }
        }))
        .build();

    println!("Items: {:?}", engine.get::<Vec<&'static str>>());

    println!("\nAll examples completed successfully!");
}
