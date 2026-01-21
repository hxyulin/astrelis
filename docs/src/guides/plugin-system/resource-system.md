# Resource System

This guide explains how to use the Astrelis resource system for managing shared state, singleton services, and global configuration across plugins and systems.

## Overview

**Resources** are type-erased shared data accessible throughout the engine:

- **Singleton services** (AssetServer, Renderer, AudioSystem)
- **Global configuration** (GameSettings, WindowConfig)
- **Shared state** (GameState, PlayerData)
- **System communication** (message queues, event channels)

**Key Concepts:**
- Type-safe access via generics
- Thread-safe with `Arc` and `RwLock`
- Registered by plugins
- Accessible from anywhere with engine reference

**Comparison to Bevy:** Similar to Bevy's `World` resources, but stored in `Engine` instead.

## Resource Storage

Resources are stored in a type-erased HashMap:

```rust
pub struct Resources {
    data: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}
```

**Key Point:** Type safety is maintained through Rust's type system.

## Registering Resources

### From Plugins

```rust
use astrelis::{Plugin, Engine};
use std::sync::Arc;

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, engine: &mut Engine) {
        // Create resource
        let asset_server = AssetServer::new(PathBuf::from("assets"));

        // Register as Arc (shared)
        engine.insert_resource(Arc::new(asset_server));
    }
}
```

### Directly on Engine

```rust
let mut engine = Engine::builder().build();

// Insert resource
let config = GameConfig {
    difficulty: Difficulty::Normal,
    volume: 0.8,
};

engine.insert_resource(Arc::new(config));
```

### Multiple Resources of Same Type

Use wrapper types:

```rust
pub struct PlayerConfig(pub Config);
pub struct EnemyConfig(pub Config);

engine.insert_resource(Arc::new(PlayerConfig(config1)));
engine.insert_resource(Arc::new(EnemyConfig(config2)));
```

## Accessing Resources

### Getting Resources

```rust
// Get resource (returns Option<Arc<T>>)
if let Some(assets) = engine.get::<Arc<AssetServer>>() {
    let texture = assets.load("texture.png")?;
}

// Get resource or panic
let assets = engine.get::<Arc<AssetServer>>()
    .expect("AssetServer not registered");
```

### From App Trait

```rust
use astrelis_winit::{App, AppCtx};

struct MyGame;

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        // Access via context
        if let Some(assets) = ctx.engine.get::<Arc<AssetServer>>() {
            // Use asset server
        }
    }
}
```

### Shared References

Resources are accessed via `Arc`:

```rust
// Get Arc clone (cheap)
let assets1 = engine.get::<Arc<AssetServer>>().unwrap();
let assets2 = engine.get::<Arc<AssetServer>>().unwrap();

// Both point to same instance
assert_eq!(
    Arc::as_ptr(&assets1),
    Arc::as_ptr(&assets2)
);
```

## Mutable Resources

Use `RwLock` or `Mutex` for mutability:

### With RwLock

```rust
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub enum GameState {
    MainMenu,
    Playing,
    Paused,
    GameOver,
}

// Register mutable resource
let game_state = Arc::new(RwLock::new(GameState::MainMenu));
engine.insert_resource(game_state);

// Read access
if let Some(state) = engine.get::<Arc<RwLock<GameState>>>() {
    let current = state.read().unwrap();
    println!("Current state: {:?}", *current);
}

// Write access
if let Some(state) = engine.get::<Arc<RwLock<GameState>>>() {
    let mut current = state.write().unwrap();
    *current = GameState::Playing;
}
```

### With Mutex

```rust
use std::sync::{Arc, Mutex};

pub struct ScoreTracker {
    score: u32,
    high_score: u32,
}

// Register
let tracker = Arc::new(Mutex::new(ScoreTracker {
    score: 0,
    high_score: 0,
}));
engine.insert_resource(tracker);

// Access
if let Some(tracker) = engine.get::<Arc<Mutex<ScoreTracker>>>() {
    let mut data = tracker.lock().unwrap();
    data.score += 10;
}
```

## Resource Patterns

### Singleton Services

Resources as services:

```rust
// Asset management service
pub struct AssetServer {
    // ... fields
}

impl AssetServer {
    pub fn load<T>(&self, path: &str) -> Handle<T> {
        // ... implementation
    }
}

// Register as singleton
engine.insert_resource(Arc::new(AssetServer::new(path)));

// Access from anywhere
fn load_texture(engine: &Engine) -> Handle<Texture> {
    let assets = engine.get::<Arc<AssetServer>>().unwrap();
    assets.load("texture.png")
}
```

### Configuration

Global settings:

```rust
#[derive(Clone)]
pub struct GameSettings {
    pub master_volume: f32,
    pub resolution: (u32, u32),
    pub fullscreen: bool,
    pub vsync: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            resolution: (1920, 1080),
            fullscreen: false,
            vsync: true,
        }
    }
}

// Register
engine.insert_resource(Arc::new(RwLock::new(GameSettings::default())));

// Apply settings
fn apply_settings(engine: &Engine, renderer: &mut Renderer) {
    if let Some(settings) = engine.get::<Arc<RwLock<GameSettings>>>() {
        let settings = settings.read().unwrap();
        renderer.set_vsync(settings.vsync);
        renderer.set_resolution(settings.resolution);
    }
}
```

### Message Passing

Inter-system communication:

```rust
use crossbeam::channel::{unbounded, Sender, Receiver};

pub enum GameMessage {
    PlayerScored(u32),
    EnemyDefeated(EntityId),
    LevelCompleted,
}

pub struct MessageBus {
    sender: Sender<GameMessage>,
    receiver: Receiver<GameMessage>,
}

impl MessageBus {
    pub fn new() -> Self {
        let (sender, receiver) = unbounded();
        Self { sender, receiver }
    }

    pub fn send(&self, msg: GameMessage) {
        self.sender.send(msg).ok();
    }

    pub fn drain(&self) -> Vec<GameMessage> {
        self.receiver.try_iter().collect()
    }
}

// Register
engine.insert_resource(Arc::new(MessageBus::new()));

// Send messages
fn on_player_scored(engine: &Engine, points: u32) {
    if let Some(bus) = engine.get::<Arc<MessageBus>>() {
        bus.send(GameMessage::PlayerScored(points));
    }
}

// Receive messages
fn process_messages(engine: &Engine) {
    if let Some(bus) = engine.get::<Arc<MessageBus>>() {
        for msg in bus.drain() {
            match msg {
                GameMessage::PlayerScored(points) => {
                    println!("Player scored {} points!", points);
                }
                GameMessage::EnemyDefeated(id) => {
                    println!("Enemy {} defeated!", id);
                }
                GameMessage::LevelCompleted => {
                    println!("Level completed!");
                }
            }
        }
    }
}
```

### Cached Data

Share expensive computations:

```rust
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

pub struct NavMeshCache {
    meshes: RwLock<HashMap<String, Arc<NavMesh>>>,
}

impl NavMeshCache {
    pub fn new() -> Self {
        Self {
            meshes: RwLock::new(HashMap::new()),
        }
    }

    pub fn get_or_compute(&self, level_name: &str) -> Arc<NavMesh> {
        // Check cache
        {
            let cache = self.meshes.read().unwrap();
            if let Some(mesh) = cache.get(level_name) {
                return mesh.clone();
            }
        }

        // Compute (expensive)
        let mesh = Arc::new(compute_nav_mesh(level_name));

        // Cache result
        {
            let mut cache = self.meshes.write().unwrap();
            cache.insert(level_name.to_string(), mesh.clone());
        }

        mesh
    }
}

// Register
engine.insert_resource(Arc::new(NavMeshCache::new()));
```

## Resource Lifecycle

### Initialization

Resources are initialized by plugins:

```rust
impl Plugin for MyPlugin {
    fn build(&self, engine: &mut Engine) {
        let resource = MyResource::new();
        engine.insert_resource(Arc::new(resource));
    }
}
```

### Access During Runtime

```rust
impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        // Access resource
        if let Some(resource) = ctx.engine.get::<Arc<MyResource>>() {
            resource.update(time.delta);
        }
    }
}
```

### Cleanup

Resources are dropped when engine is dropped:

```rust
pub struct MyResource {
    // ... fields
}

impl Drop for MyResource {
    fn drop(&mut self) {
        log::info!("MyResource cleaned up");
        // Cleanup code here
    }
}

{
    let engine = Engine::builder()
        .add_plugin(MyPlugin)
        .build();

    // Use engine...
} // Engine dropped, MyResource::drop() called
```

## Advanced Patterns

### Lazy Initialization

Initialize resources on first access:

```rust
pub struct LazyResource {
    inner: Mutex<Option<ExpensiveData>>,
}

impl LazyResource {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub fn get(&self) -> MutexGuard<ExpensiveData> {
        let mut inner = self.inner.lock().unwrap();

        if inner.is_none() {
            log::info!("Initializing expensive resource...");
            *inner = Some(ExpensiveData::compute());
        }

        MutexGuard::map(inner, |opt| opt.as_mut().unwrap())
    }
}
```

### Resource Dependencies

Resources can depend on other resources:

```rust
pub struct DependentResource {
    dependency: Arc<BaseResource>,
}

impl Plugin for DependentPlugin {
    fn build(&self, engine: &mut Engine) {
        // Get dependency
        let base = engine.get::<Arc<BaseResource>>()
            .expect("DependentPlugin requires BasePlugin");

        // Create dependent resource
        let dependent = DependentResource {
            dependency: base.clone(),
        };

        engine.insert_resource(Arc::new(dependent));
    }

    fn dependencies(&self) -> Vec<PluginDependency> {
        vec![PluginDependency::required::<BasePlugin>()]
    }
}
```

### Type-Safe Resource Access

Create helper trait:

```rust
pub trait EngineExt {
    fn assets(&self) -> Option<Arc<AssetServer>>;
    fn renderer(&self) -> Option<Arc<Renderer>>;
    fn input(&self) -> Option<Arc<InputState>>;
}

impl EngineExt for Engine {
    fn assets(&self) -> Option<Arc<AssetServer>> {
        self.get::<Arc<AssetServer>>()
    }

    fn renderer(&self) -> Option<Arc<Renderer>> {
        self.get::<Arc<Renderer>>()
    }

    fn input(&self) -> Option<Arc<InputState>> {
        self.get::<Arc<InputState>>()
    }
}

// Usage
let assets = engine.assets().unwrap();
let renderer = engine.renderer().unwrap();
```

## Thread Safety

### Arc for Shared Ownership

```rust
// Cheap to clone, thread-safe
let resource1 = engine.get::<Arc<MyResource>>().unwrap();
let resource2 = engine.get::<Arc<MyResource>>().unwrap();

// Can pass to different threads
std::thread::spawn(move || {
    resource2.do_work();
});
```

### RwLock for Shared Mutability

```rust
let state = engine.get::<Arc<RwLock<GameState>>>().unwrap();

// Multiple readers
let reader1 = state.read().unwrap();
let reader2 = state.read().unwrap(); // OK: multiple readers

// Single writer
drop(reader1);
drop(reader2);
let mut writer = state.write().unwrap(); // Exclusive access
```

### Avoiding Deadlocks

```rust
// BAD: Can deadlock
let state = engine.get::<Arc<RwLock<GameState>>>().unwrap();
let data = engine.get::<Arc<RwLock<GameData>>>().unwrap();

let _state_lock = state.write().unwrap();
let _data_lock = data.write().unwrap(); // Potential deadlock if another thread locks in reverse order

// GOOD: Consistent lock ordering
let _data_lock = data.write().unwrap(); // Always lock data first
let _state_lock = state.write().unwrap(); // Then state
```

## Performance Considerations

### Arc Clone Cost

```rust
// Arc clone is cheap (~1-2 CPU cycles)
let resource = engine.get::<Arc<MyResource>>().unwrap(); // ~1ns

// But avoid excessive clones in tight loops
for _ in 0..1_000_000 {
    let r = engine.get::<Arc<MyResource>>().unwrap(); // Slower
}

// Better: clone once
let resource = engine.get::<Arc<MyResource>>().unwrap();
for _ in 0..1_000_000 {
    resource.use_data(); // Fast
}
```

### Lock Contention

```rust
// BAD: Holds lock too long
{
    let mut state = game_state.write().unwrap();
    expensive_computation(); // Other threads blocked!
    state.value = result;
}

// GOOD: Minimize lock duration
let result = expensive_computation(); // No lock held
{
    let mut state = game_state.write().unwrap();
    state.value = result; // Fast
}
```

## Testing

```rust
#[test]
fn test_resource_system() {
    let engine = Engine::builder().build();

    // Register resource
    let config = Arc::new(GameConfig::default());
    engine.insert_resource(config.clone());

    // Retrieve resource
    let retrieved = engine.get::<Arc<GameConfig>>().unwrap();

    // Same instance
    assert_eq!(Arc::as_ptr(&config), Arc::as_ptr(&retrieved));
}

#[test]
fn test_mutable_resource() {
    let engine = Engine::builder().build();

    // Register mutable resource
    let state = Arc::new(RwLock::new(0u32));
    engine.insert_resource(state.clone());

    // Modify
    {
        let state = engine.get::<Arc<RwLock<u32>>>().unwrap();
        *state.write().unwrap() = 42;
    }

    // Verify
    {
        let state = engine.get::<Arc<RwLock<u32>>>().unwrap();
        assert_eq!(*state.read().unwrap(), 42);
    }
}
```

## Best Practices

### ✅ DO: Use Arc for Resources

```rust
// Good: Arc for shared ownership
engine.insert_resource(Arc::new(AssetServer::new(path)));
```

### ✅ DO: Use RwLock for Mutable State

```rust
// Good: RwLock for mutability
engine.insert_resource(Arc::new(RwLock::new(GameState::MainMenu)));
```

### ✅ DO: Keep Resource Names Descriptive

```rust
// Good: Clear type
pub struct PlayerInventory { /* ... */ }

// Bad: Generic type
pub struct Data { /* ... */ }
```

### ❌ DON'T: Store Resources Directly

```rust
// BAD: Trying to store resource directly
engine.insert_resource(AssetServer::new(path)); // Won't compile

// GOOD: Wrap in Arc
engine.insert_resource(Arc::new(AssetServer::new(path)));
```

### ❌ DON'T: Hold Locks Across Await Points

```rust
// BAD: Deadlock risk
async fn bad_async(state: Arc<RwLock<State>>) {
    let _lock = state.write().unwrap();
    some_async_operation().await; // Lock held across await!
}

// GOOD: Release lock before await
async fn good_async(state: Arc<RwLock<State>>) {
    {
        let _lock = state.write().unwrap();
        // Do work
    } // Lock released
    some_async_operation().await;
}
```

## Comparison to Other Systems

| Bevy | Astrelis | Notes |
|------|----------|-------|
| `World::insert_resource()` | `Engine::insert_resource()` | Similar API |
| `Res<T>` | `engine.get::<Arc<T>>()` | Explicit Arc |
| `ResMut<T>` | `Arc<RwLock<T>>` | Explicit locking |
| `Resource` trait | Any + Send + Sync | Less restrictive |

## Troubleshooting

### Resource Not Found

**Cause:** Resource not registered or wrong type.

**Fix:**
```rust
// Check if registered
if engine.get::<Arc<MyResource>>().is_none() {
    println!("MyResource not registered!");
}

// Ensure plugin registered resource
engine.add_plugin(MyPlugin);
```

### Type Mismatch

**Cause:** Accessing with wrong type.

**Fix:**
```rust
// Registered as Arc<RwLock<T>>
engine.insert_resource(Arc::new(RwLock::new(data)));

// Must access as Arc<RwLock<T>>
let resource = engine.get::<Arc<RwLock<MyData>>>().unwrap(); // Correct type
```

## Next Steps

- **Practice:** Create custom resources for your game
- **Integration:** [Creating Plugins](creating-plugins.md) for plugin development
- **Advanced:** [Plugin Composition](plugin-composition.md) for complex setups
- **Examples:** `resource_usage`, `custom_resources`

## See Also

- [Creating Plugins](creating-plugins.md) - Resource registration
- [Plugin Composition](plugin-composition.md) - Resource dependencies
- API Reference: [`Engine::insert_resource()`](../../api/astrelis/struct.Engine.html#method.insert_resource)
- API Reference: [`Engine::get()`](../../api/astrelis/struct.Engine.html#method.get)
