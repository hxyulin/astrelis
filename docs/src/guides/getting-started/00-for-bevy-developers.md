# For Bevy Developers

Welcome to Astrelis! If you're coming from Bevy, this guide will help you understand the architectural differences and how to think about building apps in Astrelis. While both are Rust game engines, they take fundamentally different approaches to architecture and game loop management.

## Quick Concept Mapping

| Bevy Concept | Astrelis Equivalent | Notes |
|--------------|---------------------|-------|
| `World` | `Engine` resources or manual state | Astrelis doesn't have a centralized ECS World yet. Use the `Engine` for resources or manage state in your `App` struct. |
| `Entity` | Manual struct or `astrelis-ecs` (WIP) | No built-in entities yet. Manage objects as structs in vectors or use a third-party ECS. |
| `Component` | Struct fields | Store data in struct fields instead of components. |
| `System` | `App` trait methods | Game logic lives in `update()` and `render()` instead of systems. |
| `Query<T>` | Direct struct access | Access state directly instead of querying. |
| `Commands` | Direct mutation | Mutate state directly instead of using deferred commands. |
| `Resource` | `Engine::insert()`/`get()` | Type-erased resource storage similar to Bevy. |
| `Plugin` | `Plugin` trait | Very similar! Astrelis plugins work like Bevy plugins. |
| `App::add_plugin()` | `Engine::builder().add_plugin()` | Nearly identical API. |
| `EventReader`/`Writer` | Manual channel or event system | No built-in global event system yet. |
| `ResMut<Assets<T>>` | `AssetServer` | Similar asset management with handles. |
| `Handle<T>` | `Handle<T>` | Identical concept! Type-safe asset handles. |
| `Camera` | `RenderableWindow` + render passes | Manual render control instead of automatic camera systems. |
| `SpriteBundle` | Manual rendering | No automatic sprite rendering yet. |
| `UiBundle` | `UiSystem` | Declarative UI, but different API style. |
| `Time` | `AppCtx` duration tracking | Frame timing available in context. |

## Fundamental Architecture Difference

### Bevy: ECS-First Architecture

Bevy is built entirely around **ECS (Entity Component System)**. Everything is an Entity with Components, processed by Systems:

```rust
// Bevy
use bevy::prelude::*;

#[derive(Component)]
struct Player {
    speed: f32,
}

#[derive(Component)]
struct Velocity(Vec2);

fn player_movement_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&Player, &mut Velocity)>,
) {
    for (player, mut velocity) in query.iter_mut() {
        if keyboard.pressed(KeyCode::KeyA) {
            velocity.0.x = -player.speed;
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Update, player_movement_system)
        .run();
}
```

Bevy automatically schedules and runs systems in parallel based on their access patterns.

### Astrelis: Traditional App Architecture

Astrelis uses a **traditional game loop** with manual state management:

```rust
// Astrelis
use std::sync::Arc;
use astrelis_winit::{run_app, App, AppCtx};
use astrelis_render::{GraphicsContext, RenderableWindow};

struct Player {
    speed: f32,
    velocity: Vec2,
}

struct MyGame {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    player: Player,
}

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx) {
        // Handle input and update state directly
        // (Input system integration pending)
        self.player.velocity.x = -self.player.speed;
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        let mut frame = self.window.begin_drawing();
        frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
            // Render manually
        });
        frame.finish();
    }
}

fn main() {
    run_app(|ctx| {
        let graphics = GraphicsContext::new_owned_sync();
        let window = ctx.create_window(descriptor).unwrap();
        let renderable = RenderableWindow::new(window, graphics.clone());
        Box::new(MyGame {
            graphics,
            window: renderable,
            player: Player { speed: 5.0, velocity: Vec2::ZERO },
        })
    });
}
```

Astrelis gives you **direct control** over the game loop instead of automatic system scheduling.

## Key Architectural Differences

### 1. ECS vs Manual State Management

**Bevy**: Everything is an Entity with Components. You query for entities with specific components.

**Astrelis**: You manage state however you want. Use vectors, hash maps, or even integrate a third-party ECS:

```rust
// Astrelis: Manual state management
struct GameState {
    // Option 1: Simple vectors
    players: Vec<Player>,
    enemies: Vec<Enemy>,

    // Option 2: Hash maps for lookup
    entities: HashMap<EntityId, Entity>,

    // Option 3: Integrate a third-party ECS
    world: hecs::World,  // or bevy_ecs, or specs

    // Option 4: Wait for astrelis-ecs (in development)
}

struct MyGame {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    state: GameState,
}
```

The future `astrelis-ecs` crate will provide built-in ECS support, but for now you have full flexibility.

### 2. Systems vs Methods

**Bevy**: Game logic lives in **systems** that run automatically:

```rust
// Bevy systems
fn movement_system(mut query: Query<(&Velocity, &mut Transform)>) {
    for (velocity, mut transform) in query.iter_mut() {
        transform.translation += velocity.0.extend(0.0);
    }
}

fn collision_system(query: Query<&Transform, With<Collider>>) {
    // Collision detection
}

App::new()
    .add_systems(Update, (movement_system, collision_system))
```

**Astrelis**: Game logic lives in **`App` trait methods** that you call explicitly:

```rust
// Astrelis methods
impl MyGame {
    fn update_movement(&mut self) {
        for entity in &mut self.state.entities {
            entity.transform.position += entity.velocity;
        }
    }

    fn update_collisions(&mut self) {
        // Collision detection
    }
}

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx) {
        // Call your methods in whatever order you want
        self.update_movement();
        self.update_collisions();
    }
}
```

### 3. Parallel Systems vs Sequential Execution

**Bevy**: Systems run in parallel automatically based on their access patterns. Bevy analyzes which systems can run concurrently and schedules them efficiently.

**Astrelis**: Your code runs sequentially unless you explicitly use threads/async. You control parallelism:

```rust
impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx) {
        // Runs sequentially by default
        self.update_physics();
        self.update_ai();
        self.update_rendering_data();

        // Or use Rayon for parallelism
        use rayon::prelude::*;
        self.enemies.par_iter_mut().for_each(|enemy| {
            enemy.update();
        });
    }
}
```

### 4. Resources: Similar But Different

**Bevy**: Type-based resource storage with automatic injection:

```rust
// Bevy
fn my_system(time: Res<Time>, assets: Res<Assets<Texture>>) {
    // Resources are automatically injected
}
```

**Astrelis**: Type-based resource storage, but manual access:

```rust
// Astrelis
let engine = Engine::builder()
    .add_plugin(AssetPlugin)
    .build();

let assets = engine.get::<AssetServer>().unwrap();
assets.load("texture.png");
```

Both use `TypeId`-based storage, but Astrelis requires explicit access instead of automatic dependency injection.

### 5. Plugins: Very Similar!

This is one area where Bevy and Astrelis are nearly identical:

**Bevy**:
```rust
struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

App::new().add_plugins(MyPlugin).run();
```

**Astrelis**:
```rust
struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, engine: &mut Engine) {
        engine.insert(MyResource::new());
    }
}

Engine::builder().add_plugin(MyPlugin).build();
```

Both support **plugin dependencies** and **bundling**:

```rust
// Astrelis (similar to Bevy)
Engine::builder()
    .add_plugin(InputPlugin)
    .add_plugin(AssetPlugin)
    .add_plugins(DefaultPlugins)  // Bundle
    .build();
```

## Asset System: Nearly Identical

Bevy and Astrelis both use **type-safe handles** for assets:

**Bevy**:
```rust
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let texture: Handle<Image> = asset_server.load("texture.png");
    commands.spawn(SpriteBundle {
        texture,
        ..default()
    });
}
```

**Astrelis**:
```rust
let assets = engine.get::<AssetServer>().unwrap();
let texture: Handle<Texture> = assets.load("texture.png");

// Later, get the actual asset
if let Some(tex) = assets.get(texture) {
    // Use texture
}
```

**Key similarity**: Both use generational handles to prevent use-after-free bugs.

**Difference**: Astrelis has built-in hot-reload support with file watching out of the box.

## Rendering: Very Different

### Bevy's Automatic Rendering

Bevy automatically renders entities with specific components:

```rust
// Bevy: Automatic sprite rendering
commands.spawn(SpriteBundle {
    texture: asset_server.load("player.png"),
    transform: Transform::from_xyz(0.0, 0.0, 0.0),
    ..default()
});

// Camera automatically renders all visible sprites
commands.spawn(Camera2dBundle::default());
```

Bevy's render graph automatically extracts entities, batches draws, and renders everything.

### Astrelis's Manual Rendering

Astrelis requires **explicit render control**:

```rust
impl App for MyGame {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        let mut frame = self.window.begin_drawing();

        // You control what gets rendered and when
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb(0.1, 0.1, 0.1),
            |pass| {
                // Manually render sprites, UI, etc.
                self.renderer.draw_sprite(&pass, texture, transform);
                self.ui.render(pass.wgpu_pass());
            },
        );

        frame.finish();
    }
}
```

**Why the difference?**: Astrelis is more low-level. You get direct GPU control but must manage rendering yourself. Bevy abstracts this away for convenience.

## UI System: Different Approaches

### Bevy UI: ECS-Based

Bevy UI uses entities and components:

```rust
// Bevy UI
commands
    .spawn(NodeBundle {
        style: Style {
            width: Val::Px(200.0),
            height: Val::Px(100.0),
            ..default()
        },
        background_color: Color::BLUE.into(),
        ..default()
    })
    .with_children(|parent| {
        parent.spawn(TextBundle::from_section(
            "Hello",
            TextStyle {
                font_size: 30.0,
                color: Color::WHITE,
                ..default()
            },
        ));
    });
```

### Astrelis UI: Declarative Retained-Mode

Astrelis UI is declarative with a builder API:

```rust
// Astrelis UI
ui.build(|root| {
    root.container()
        .width(Length::px(200.0))
        .height(Length::px(100.0))
        .background_color(Color::BLUE)
        .child(|text| {
            text.text("Hello")
                .font_size(30.0)
                .color(Color::WHITE)
                .build();
        })
        .build();
});

// Fast incremental updates
ui.update_text("greeting", "Hello, World!");
```

**Key differences**:
- Astrelis uses **widget IDs** for efficient updates
- Astrelis has **dirty flags** to skip unnecessary work (color change doesn't re-layout)
- More similar to React/SwiftUI than Bevy's ECS UI

## When to Use Astrelis vs Bevy

### Use Bevy if you want:
- **ECS-first architecture** with automatic parallelism
- **High-level abstractions** (automatic rendering, sprite batching)
- **Large community** and ecosystem
- **Opinionated structure** with best practices built-in
- **2D/3D rendering** out of the box

### Use Astrelis if you want:
- **Direct control** over the game loop and rendering
- **Lower-level GPU access** (WGPU) for custom rendering
- **Flexibility** to structure your app however you want
- **Lighter weight** (opt-in crates instead of monolithic engine)
- **Modern UI system** with fine-grained dirty tracking
- **Arc-based resource sharing** instead of ECS World

### Comparison Table

| Feature | Bevy | Astrelis |
|---------|------|----------|
| **Architecture** | ECS-first | Traditional app loop |
| **Game Loop** | Automatic system scheduling | Manual control |
| **Parallelism** | Automatic | Manual (Rayon, async) |
| **Rendering** | High-level (sprites, meshes) | Low-level (render passes) |
| **UI** | ECS-based UI | Declarative retained-mode |
| **State Management** | World + Components | Manual (structs, vecs, etc.) |
| **Learning Curve** | Steep (ECS concepts) | Moderate (traditional patterns) |
| **Flexibility** | Opinionated structure | Maximum flexibility |
| **Community** | Large, active | Growing |

## Porting Bevy Code to Astrelis

Here's a side-by-side comparison of a simple game:

### Bevy Version

```rust
// Bevy
use bevy::prelude::*;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Velocity(Vec2);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, movement)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());
    commands.spawn((
        Player,
        Velocity(Vec2::new(0.0, 0.0)),
        SpriteBundle {
            texture: asset_server.load("player.png"),
            ..default()
        },
    ));
}

fn movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Velocity, &mut Transform), With<Player>>,
) {
    for (mut velocity, mut transform) in query.iter_mut() {
        if keyboard.pressed(KeyCode::KeyW) {
            velocity.0.y = 100.0;
        }
        transform.translation += velocity.0.extend(0.0) * time.delta_seconds();
    }
}
```

### Astrelis Version

```rust
// Astrelis
use std::sync::Arc;
use astrelis_winit::{run_app, App, AppCtx};
use astrelis_render::{GraphicsContext, RenderableWindow, RenderTarget, Color};

struct Player {
    velocity: Vec2,
    transform: Transform,
    texture: Handle<Texture>,
}

struct MyGame {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    assets: AssetServer,
    player: Player,
}

impl App for MyGame {
    fn on_start(&mut self, ctx: &mut AppCtx) {
        // Load assets
        self.player.texture = self.assets.load("player.png");
    }

    fn update(&mut self, ctx: &mut AppCtx) {
        // Handle input (placeholder - input system pending)
        // if keyboard.is_key_pressed(KeyCode::W) {
        //     self.player.velocity.y = 100.0;
        // }

        // Update position
        let delta = ctx.last_frame_duration().as_secs_f32();
        self.player.transform.position += self.player.velocity * delta;
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        let mut frame = self.window.begin_drawing();
        frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
            // Render player sprite (custom rendering code needed)
        });
        frame.finish();
    }
}

fn main() {
    run_app(|ctx| {
        let graphics = GraphicsContext::new_owned_sync();
        let window = ctx.create_window(descriptor).unwrap();
        let renderable = RenderableWindow::new(window, graphics.clone());
        let assets = AssetServer::new();

        Box::new(MyGame {
            graphics,
            window: renderable,
            assets,
            player: Player {
                velocity: Vec2::ZERO,
                transform: Transform::default(),
                texture: Handle::default(),
            },
        })
    });
}
```

**Key porting steps:**
1. Replace `Commands.spawn()` with struct initialization
2. Replace `Query<T>` with direct field access
3. Replace systems with methods in `impl App`
4. Replace `Camera2d` with manual render calls
5. Replace automatic sprite rendering with custom rendering

## Common Pitfalls for Bevy Developers

### 1. No Automatic System Scheduling

**Bevy**: Systems run automatically in the correct order.

**Astrelis**: You must call your update methods manually in the right order:

```rust
impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx) {
        // Order matters! Physics before collision detection
        self.update_physics();
        self.update_collisions();
        self.update_animation();
    }
}
```

### 2. No Automatic Rendering

**Bevy**: Spawn a `SpriteBundle` and it renders automatically.

**Astrelis**: You must render it yourself:

```rust
fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    let mut frame = self.window.begin_drawing();
    frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
        // You must write rendering code
        for sprite in &self.sprites {
            self.renderer.draw_sprite(pass, sprite);
        }
    });
    frame.finish();
}
```

### 3. Arc Cloning for Shared Resources

**Bevy**: Resources are stored in the World and automatically accessible.

**Astrelis**: Use `Arc<T>` for shared ownership:

```rust
let graphics = GraphicsContext::new_owned_sync();

// Clone the Arc (cheap reference count increment)
let ui = UiSystem::new(graphics.clone(), window_manager.clone());
let renderer = CustomRenderer::new(graphics.clone());
```

### 4. No Commands for Deferred Operations

**Bevy**: Use `Commands` to spawn/despawn entities, avoiding borrow checker issues.

**Astrelis**: Mutate state directly. If you hit borrow checker issues, refactor your code or use interior mutability patterns.

## Next Steps

Now that you understand the differences, here's how to get started:

1. **[Installation Guide](01-installation.md)** - Set up Rust and create a project
2. **[Architecture Overview](02-architecture-overview.md)** - Deep dive into Astrelis design
3. **[Hello Window](03-hello-window.md)** - Your first Astrelis app
4. **[Rendering Fundamentals](04-rendering-fundamentals.md)** - Understand manual rendering

## When Will Astrelis Get ECS?

The `astrelis-ecs` crate is in development. When it's ready, it will provide:
- Entity and Component abstractions
- System scheduling (manual or automatic)
- Query-based data access
- Integration with the rest of Astrelis

Until then, you can integrate third-party ECS libraries like:
- **hecs**: Minimal ECS with good performance
- **bevy_ecs**: Standalone Bevy ECS (can be used outside Bevy)
- **specs**: Mature ECS with shred for resources

## Further Reading

- [Bevy vs Astrelis Feature Comparison](../../comparison.md) (if it exists)
- [API Reference](https://docs.rs/astrelis)
- [Plugin System Guide](../plugin-system/creating-plugins.md)

Welcome to Astrelis! Your Bevy experience will serve you well here.
