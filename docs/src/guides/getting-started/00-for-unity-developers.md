# For Unity Developers

Welcome to Astrelis! If you're coming from Unity, this guide will help you understand how Astrelis concepts map to Unity concepts you already know. Astrelis is a modular Rust game engine that takes a different architectural approach than Unity, but many core concepts translate directly.

## Quick Concept Mapping

Here's a quick reference table to help you translate Unity concepts to Astrelis:

| Unity Concept | Astrelis Equivalent | Notes |
|---------------|---------------------|-------|
| `GameObject` | Manual state management or Entity (future) | Astrelis doesn't have a built-in GameObject hierarchy yet. You manage state in your `App` struct. |
| `MonoBehaviour` | `App` trait + state structs | Your game logic lives in the `App` trait methods instead of component scripts. |
| `Update()` | `App::update()` | Called every frame for game logic. |
| `LateUpdate()` | `App::render()` | Called after update for rendering. |
| `Start()` | `App::on_start()` | Called once when the app initializes. |
| `OnDestroy()` | `App::on_exit()` | Called when the app shuts down. |
| `Canvas UI` | `UiSystem` | Declarative UI with Flexbox layout (similar to UI Toolkit). |
| `UGUI` | `UiCore` + `UiSystem` | Retained-mode UI system with incremental updates. |
| `UI Toolkit` (USS) | Widget styling API | Style widgets with Rust code instead of USS. |
| `Resources.Load()` | `AssetServer::load()` | Type-safe async asset loading with handles. |
| `AssetBundle` | Asset system with hot-reload | Load and reload assets at runtime. |
| `Shader Graph` | WGSL custom shaders | Write shaders in WGSL (similar to HLSL). |
| `Material` | `Material` + `MaterialParameter` | Set shader parameters programmatically. |
| `Camera` | `RenderableWindow` + render passes | Manual render pass control instead of automatic camera rendering. |
| `Camera.Render()` | `FrameContext` + `clear_and_render()` | Explicit frame rendering with RAII. |
| `RenderTexture` | `Framebuffer` + `RenderTarget` | Render-to-texture for post-processing. |
| `Coroutine` | `TaskPool` + async/await | Use Rust's async/await instead of coroutines. |
| `Input.GetKey()` | `astrelis-input` crate | Query keyboard/mouse state. |
| `Physics2D/3D` | Not built-in (use Rapier) | Integrate third-party physics libraries. |
| `AudioSource` | `astrelis-audio` (WIP) | Audio system is in development. |
| `Scene` | `astrelis-scene` (WIP) | Scene management is in development. |

## Architecture Comparison

### Unity's Scene-Based Architecture

In Unity, you organize your game into **Scenes** containing **GameObjects** with **Components**:

```csharp
// Unity C#
public class PlayerController : MonoBehaviour {
    public float speed = 5.0f;

    void Update() {
        float h = Input.GetAxis("Horizontal");
        transform.position += Vector3.right * h * speed * Time.deltaTime;
    }
}
```

Unity automatically calls `Update()` on all components, manages the GameObject hierarchy, and handles rendering through Cameras.

### Astrelis's Modular Architecture

Astrelis uses a **modular crate** architecture where you choose which systems you need:

```rust
// Astrelis Rust
use std::sync::Arc;
use astrelis_winit::{run_app, App, AppCtx};
use astrelis_render::{GraphicsContext, RenderableWindow};

struct MyGame {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    player_x: f32,
}

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx) {
        // Handle input and update game state
        self.player_x += 0.1;
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Render the frame
        let mut frame = self.window.begin_drawing();
        frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
            // Render calls go here
        });
        frame.finish();
    }
}
```

In Astrelis:
- You **explicitly manage state** in your app struct instead of a GameObject hierarchy
- You **manually control the render loop** instead of relying on automatic Camera rendering
- You **choose which crates to include** (rendering, UI, assets, etc.)

## Key Architectural Differences

### 1. Manual vs Automatic Lifecycle

**Unity**: Automatic component lifecycle. Unity calls `Update()`, `LateUpdate()`, etc. on all active components.

**Astrelis**: Manual lifecycle control via the `App` trait. You decide what gets updated and when.

```rust
impl App for MyGame {
    fn on_start(&mut self, ctx: &mut AppCtx) {
        // Called once on startup (like Start())
    }

    fn update(&mut self, ctx: &mut AppCtx) {
        // Called every frame (like Update())
        // Update game logic here
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Called every frame after update (like LateUpdate())
        // Rendering happens here
    }

    fn on_exit(&mut self, ctx: &mut AppCtx) {
        // Called on shutdown (like OnDestroy())
    }
}
```

### 2. Composition vs Inheritance

**Unity**: Component-based composition. You attach components to GameObjects.

**Astrelis**: Rust struct composition. You compose your app state with regular structs:

```rust
struct PlayerState {
    position: Vec2,
    velocity: Vec2,
    health: f32,
}

struct GameState {
    player: PlayerState,
    enemies: Vec<EnemyState>,
    score: u32,
}

struct MyGame {
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    ui: UiSystem,
    game: GameState,  // Your game state here
}
```

### 3. Automatic vs Explicit Rendering

**Unity**: Cameras automatically render the scene. You rarely think about render passes or command buffers unless doing advanced rendering.

**Astrelis**: You explicitly control rendering with render passes and frame contexts:

```rust
fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    // Begin the frame (acquires surface texture)
    let mut frame = self.window.begin_drawing();

    // Clear and render (automatic pass scoping)
    frame.clear_and_render(
        RenderTarget::Surface,
        Color::from_rgb(0.1, 0.1, 0.1),
        |pass| {
            // All rendering happens inside this closure
            self.ui.render(pass.wgpu_pass());
        },
    );

    // Finish automatically submits commands and presents
    frame.finish();
}
```

The `clear_and_render()` method handles render pass lifecycle automatically using Rust's RAII pattern.

### 4. Scene Hierarchy vs Flat State

**Unity**: GameObject hierarchy with parent/child relationships. `transform.parent`, `GetComponentInChildren()`, etc.

**Astrelis**: Currently no built-in hierarchy. You manage state however you like:

```rust
struct GameState {
    // Option 1: Flat arrays
    players: Vec<Player>,
    bullets: Vec<Bullet>,

    // Option 2: Manual hierarchy
    root_entities: Vec<Entity>,

    // Option 3: Use a library like hecs or bevy_ecs
    world: hecs::World,
}
```

The `astrelis-ecs` crate will provide ECS (Entity Component System) support in the future, similar to Unity DOTS or Bevy's ECS.

## Asset Loading Comparison

### Unity

```csharp
// Unity: Resources.Load
Texture2D texture = Resources.Load<Texture2D>("Textures/Player");

// Unity: Async loading
var request = Resources.LoadAsync<Texture2D>("Textures/Player");
yield return request;
Texture2D texture = request.asset as Texture2D;
```

### Astrelis

```rust
// Astrelis: Synchronous handle creation (async loading in background)
let texture_handle: Handle<Texture> = assets.load("textures/player.png");

// Later, check if loaded
if let Some(texture) = assets.get(texture_handle) {
    // Texture is ready to use
}

// Or use events
for event in assets.drain_events::<Texture>() {
    match event {
        AssetEvent::Loaded { handle } => {
            println!("Texture loaded: {:?}", handle);
        }
        AssetEvent::Modified { handle } => {
            println!("Texture hot-reloaded: {:?}", handle);
        }
    }
}
```

**Key differences:**
- Astrelis uses **type-safe handles** with generation counters (prevents use-after-free)
- Loading is **async by default** (doesn't block the main thread)
- Built-in **hot-reload support** with file watching

## UI System Comparison

### Unity UGUI

```csharp
// Unity UGUI
public class CounterUI : MonoBehaviour {
    public Text counterText;
    private int count = 0;

    void Start() {
        Button btn = GetComponent<Button>();
        btn.onClick.AddListener(OnClick);
    }

    void OnClick() {
        count++;
        counterText.text = $"Count: {count}";
    }
}
```

### Astrelis UI

```rust
// Astrelis: Declarative UI
let mut ui = UiSystem::new(graphics.clone(), window_manager.clone());

// Build UI once
ui.build(|root| {
    root.column()
        .child(|button| {
            button.text("Click Me")
                .id("btn")
                .on_click(|_| {
                    println!("Button clicked!");
                })
                .build();
        })
        .child(|text| {
            text.text("Count: 0")
                .id("counter")
                .build();
        })
        .build();
});

// Fast incremental update (doesn't rebuild entire tree)
let count = 1;
ui.update_text("counter", format!("Count: {}", count));
```

**Key differences:**
- **Declarative** API (more like React or SwiftUI than UGUI)
- **Retained-mode** with **dirty flags** for efficient updates
- **Flexbox layout** (similar to Unity's UI Toolkit, not UGUI)
- **Type-safe widget IDs** for updates

## Shader Workflow Comparison

### Unity Shader Graph

Unity's Shader Graph is a visual node-based shader editor. For code, you write HLSL in ShaderLab:

```hlsl
// Unity ShaderLab
Shader "Custom/MyShader" {
    Properties {
        _MainTex ("Texture", 2D) = "white" {}
        _Color ("Color", Color) = (1,1,1,1)
    }
    SubShader {
        Pass {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag

            sampler2D _MainTex;
            float4 _Color;

            v2f vert(appdata v) { ... }
            fixed4 frag(v2f i) : SV_Target {
                return tex2D(_MainTex, i.uv) * _Color;
            }
            ENDCG
        }
    }
}
```

### Astrelis WGSL Shaders

Astrelis uses WGSL (WebGPU Shading Language), similar to HLSL:

```wgsl
// Astrelis WGSL
@group(0) @binding(0) var t_texture: texture_2d<f32>;
@group(0) @binding(1) var s_sampler: sampler;

struct Uniforms {
    color: vec4<f32>,
}

@group(0) @binding(2) var<uniform> uniforms: Uniforms;

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_texture, s_sampler, uv);
    return tex_color * uniforms.color;
}
```

**Key differences:**
- **WGSL syntax** instead of HLSL (but very similar)
- **Explicit bind groups** for resources (like Vulkan descriptors)
- **No ShaderLab wrapper** (pure shader code)
- **Manual pipeline creation** in Rust code

## Rendering Pipeline Comparison

### Unity Rendering

```csharp
// Unity: Camera automatically renders
// You rarely touch rendering code unless doing advanced stuff

// SRP (Scriptable Render Pipeline) for custom rendering
void Render(ScriptableRenderContext context, Camera camera) {
    CommandBuffer cmd = CommandBufferPool.Get("MyRenderPass");
    cmd.Blit(source, destination, material);
    context.ExecuteCommandBuffer(cmd);
    context.Submit();
}
```

### Astrelis Rendering

```rust
// Astrelis: Manual render pass control
fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    let mut frame = self.window.begin_drawing();

    // Render to screen
    frame.clear_and_render(
        RenderTarget::Surface,
        Color::BLACK,
        |pass| {
            // Render calls
        },
    );

    // Or render to texture for post-processing
    frame.clear_and_render(
        RenderTarget::Framebuffer(&self.my_framebuffer),
        Color::BLACK,
        |pass| {
            // Render scene to texture
        },
    );

    frame.finish();
}
```

**Key concepts:**
- `FrameContext` = Unity's per-frame command buffer
- `RenderPass` = Unity's render pass in SRP
- `RenderTarget` = Surface (screen) or Framebuffer (texture)
- RAII: `frame.finish()` automatically submits and presents

## Plugin System vs Unity Packages

### Unity Packages

Unity uses the Package Manager for extensions:

```json
// manifest.json
{
  "dependencies": {
    "com.unity.textmeshpro": "3.0.6"
  }
}
```

### Astrelis Plugins

Astrelis uses a **Rust-based plugin system**:

```rust
let engine = Engine::builder()
    .add_plugin(InputPlugin)
    .add_plugin(AssetPlugin)
    .add_plugin(RenderPlugin)
    .add_plugins(DefaultPlugins)  // Bundle of common plugins
    .build();

// Access resources added by plugins
let assets = engine.get::<AssetServer>().unwrap();
```

**Key differences:**
- **Compile-time** plugin integration (no dynamic loading by default)
- **Type-safe** resource access
- **Dependency resolution** with topological sorting
- Similar to **Bevy's plugin system**

## Common Pitfalls for Unity Developers

### 1. No Automatic Rendering

**Unity**: Cameras automatically render every frame.

**Astrelis**: You must call render methods explicitly. If you see a black screen, check if you're calling `frame.clear_and_render()` and `frame.finish()`.

### 2. Arc Cloning for Shared Ownership

**Unity**: C# uses garbage collection. Objects are automatically shared.

**Astrelis**: Rust uses manual memory management. Use `Arc<T>` for shared ownership:

```rust
// Create Arc once
let graphics = GraphicsContext::new_owned_sync();

// Clone the Arc (cheap, just increments reference count)
let ui = UiSystem::new(graphics.clone(), window_manager.clone());
let renderer = CustomRenderer::new(graphics.clone());
```

### 3. RAII Resource Management

**Unity**: Resources are garbage collected.

**Astrelis**: Resources are automatically cleaned up when they go out of scope. Always ensure render passes are dropped before `frame.finish()`:

```rust
// Good: Pass is dropped at end of closure
frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
    self.ui.render(pass.wgpu_pass());
}); // pass dropped here

// Bad: Manual pass management without drop
let mut pass = RenderPassBuilder::new().build(&mut frame);
self.ui.render(pass.wgpu_pass());
// Forgot to drop pass before finish - ERROR!
frame.finish();
```

### 4. No GameObject Hierarchy (Yet)

**Unity**: `transform.parent`, `transform.Find()`, etc.

**Astrelis**: Manage hierarchy manually or wait for `astrelis-ecs` to stabilize:

```rust
// Manual hierarchy
struct Entity {
    transform: Transform,
    parent: Option<EntityId>,
    children: Vec<EntityId>,
}

// Or use a library
let mut world = hecs::World::new();
let entity = world.spawn((
    Transform::default(),
    Mesh { /* */ },
));
```

## Next Steps

Now that you understand the key differences, you're ready to start building with Astrelis:

1. **[Installation Guide](01-installation.md)** - Set up your Rust environment
2. **[Architecture Overview](02-architecture-overview.md)** - Deeper dive into Astrelis's design
3. **[Hello Window](03-hello-window.md)** - Your first Astrelis app
4. **[First UI](05-first-ui.md)** - Build interactive UI

## Further Reading

- [Astrelis vs Unity vs Bevy - Feature Comparison](../../comparison.md) (if it exists)
- [API Reference](https://docs.rs/astrelis) - Complete API documentation
- [Examples](../../examples-index.md) - 50+ working examples

## Getting Help

- **GitHub Issues**: [Report bugs or ask questions](https://github.com/yourusername/astrelis/issues)
- **Discussions**: [Community discussions](https://github.com/yourusername/astrelis/discussions)
- **Discord**: [Join the community](https://discord.gg/astrelis) (if available)

Welcome to Astrelis! We're excited to have Unity developers in the community.
