//! Plugin Composition Demo - Multiple Plugins with Dependencies
//!
//! Demonstrates the plugin system for engine extensibility:
//! - Creating custom plugins
//! - Plugin dependencies and ordering
//! - Resource sharing between plugins
//! - Plugin initialization lifecycle
//! - Topological sorting of plugins
//!
//! Shows how plugins compose to build complex engine functionality.

use astrelis::plugin::{Plugin, PluginContext};
use astrelis::Engine;
use std::any::TypeId;

// Example resource types
#[derive(Debug)]
struct GameConfig {
    title: String,
    version: String,
}

#[derive(Debug)]
struct PlayerData {
    name: String,
    score: u32,
}

#[derive(Debug)]
struct PhysicsConfig {
    gravity: f32,
    timestep: f32,
}

// Plugin 1: Core Configuration Plugin (no dependencies)
struct ConfigPlugin {
    title: String,
}

impl ConfigPlugin {
    fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
        }
    }
}

impl Plugin for ConfigPlugin {
    fn name(&self) -> &str {
        "ConfigPlugin"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![] // No dependencies
    }

    fn build(&self, ctx: &mut PluginContext) {
        println!("  [ConfigPlugin] Initializing...");

        let config = GameConfig {
            title: self.title.clone(),
            version: "1.0.0".to_string(),
        };

        ctx.insert_resource(config);
        println!("    ✓ GameConfig resource registered");
    }
}

// Plugin 2: Player Plugin (depends on ConfigPlugin)
struct PlayerPlugin {
    default_player_name: String,
}

impl PlayerPlugin {
    fn new(name: impl Into<String>) -> Self {
        Self {
            default_player_name: name.into(),
        }
    }
}

impl Plugin for PlayerPlugin {
    fn name(&self) -> &str {
        "PlayerPlugin"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<ConfigPlugin>()] // Depends on ConfigPlugin
    }

    fn build(&self, ctx: &mut PluginContext) {
        println!("  [PlayerPlugin] Initializing...");

        // Access config resource from ConfigPlugin
        if let Some(config) = ctx.get_resource::<GameConfig>() {
            println!("    ℹ Game: {} v{}", config.title, config.version);
        }

        let player = PlayerData {
            name: self.default_player_name.clone(),
            score: 0,
        };

        ctx.insert_resource(player);
        println!("    ✓ PlayerData resource registered");
    }
}

// Plugin 3: Physics Plugin (depends on ConfigPlugin)
struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn name(&self) -> &str {
        "PhysicsPlugin"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<ConfigPlugin>()] // Depends on ConfigPlugin
    }

    fn build(&self, ctx: &mut PluginContext) {
        println!("  [PhysicsPlugin] Initializing...");

        let physics_config = PhysicsConfig {
            gravity: -9.81,
            timestep: 1.0 / 60.0,
        };

        ctx.insert_resource(physics_config);
        println!("    ✓ PhysicsConfig resource registered");
    }
}

// Plugin 4: Game Plugin (depends on PlayerPlugin and PhysicsPlugin)
struct GamePlugin;

impl Plugin for GamePlugin {
    fn name(&self) -> &str {
        "GamePlugin"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![
            TypeId::of::<PlayerPlugin>(),   // Needs player system
            TypeId::of::<PhysicsPlugin>(),  // Needs physics system
        ]
    }

    fn build(&self, ctx: &mut PluginContext) {
        println!("  [GamePlugin] Initializing...");

        // Access all dependent resources
        if let Some(config) = ctx.get_resource::<GameConfig>() {
            println!("    ℹ Config: {}", config.title);
        }

        if let Some(player) = ctx.get_resource::<PlayerData>() {
            println!("    ℹ Player: {} (Score: {})", player.name, player.score);
        }

        if let Some(physics) = ctx.get_resource::<PhysicsConfig>() {
            println!("    ℹ Physics: gravity={}, timestep={}", physics.gravity, physics.timestep);
        }

        println!("    ✓ Game systems integrated");
    }
}

fn main() {
    println!("\n═══════════════════════════════════════════════════════");
    println!("  🔌 PLUGIN COMPOSITION DEMO - Modular Engine");
    println!("═══════════════════════════════════════════════════════");
    println!("  PLUGIN FEATURES:");
    println!("    • Custom plugin creation");
    println!("    • Dependency management");
    println!("    • Resource sharing");
    println!("    • Topological sorting");
    println!("    • Initialization lifecycle\n");

    println!("  PLUGIN DEPENDENCY GRAPH:");
    println!("    ConfigPlugin (root)");
    println!("    ├─ PlayerPlugin");
    println!("    ├─ PhysicsPlugin");
    println!("    └─ GamePlugin");
    println!("         ├─ depends on PlayerPlugin");
    println!("         └─ depends on PhysicsPlugin\n");

    println!("═══════════════════════════════════════════════════════\n");

    println!("Building engine with plugins...\n");

    let engine = Engine::builder()
        .add_plugin(ConfigPlugin::new("Plugin Composition Demo"))
        .add_plugin(PlayerPlugin::new("Hero"))
        .add_plugin(PhysicsPlugin)
        .add_plugin(GamePlugin)
        .build();

    println!("\n✅ Engine initialized with {} resources",
             if engine.has_resource::<GameConfig>() { "GameConfig" } else { "" });

    println!("\n📊 VERIFICATION:");

    // Verify resources are accessible
    if let Some(config) = engine.get::<GameConfig>() {
        println!("  ✓ GameConfig: {} v{}", config.title, config.version);
    }

    if let Some(player) = engine.get::<PlayerData>() {
        println!("  ✓ PlayerData: {} (Score: {})", player.name, player.score);
    }

    if let Some(physics) = engine.get::<PhysicsConfig>() {
        println!("  ✓ PhysicsConfig: gravity={}, timestep={}", physics.gravity, physics.timestep);
    }

    println!("\n═══════════════════════════════════════════════════════");
    println!("  KEY CONCEPTS:");
    println!("    • Plugins are initialized in dependency order");
    println!("    • Resources are type-safe and shared");
    println!("    • Cyclic dependencies are detected at build time");
    println!("    • Plugins compose to build complex systems");
    println!("═══════════════════════════════════════════════════════\n");
}
