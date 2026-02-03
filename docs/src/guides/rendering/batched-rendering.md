# Batched Rendering System

The batched renderer provides high-performance 2D rendering using GPU instancing. It automatically detects GPU capabilities and selects the optimal rendering tier.

## Overview

Batched rendering minimizes draw calls by grouping similar geometry into a single GPU draw command. Astrelis supports three rendering tiers, automatically selected based on GPU capabilities:

1. **Direct** - Basic instancing with bind group per texture
2. **Indirect** - GPU-driven draw calls with indirect buffers
3. **Bindless** - Texture arrays for reduced state changes (fastest)

## Architecture

### Rendering Tiers

#### Tier 1: Direct Rendering

**Requirements:** Any GPU supporting instancing (all modern GPUs)

**Characteristics:**
- One draw call per texture
- CPU prepares instance data
- Bind group created per texture batch

**Use when:**
- Basic GPU without advanced features
- Small number of unique textures (<10)

```rust
// Direct rendering is always available
let renderer = BatchedRenderer::new(graphics.clone());
```

#### Tier 2: Indirect Rendering

**Requirements:** `INDIRECT_FIRST_INSTANCE` feature

**Characteristics:**
- GPU-driven draw calls via `drawIndirect()`
- Single instance buffer for all geometry
- Reduced CPU overhead for command preparation

**Use when:**
- Drawing thousands of instances
- GPU supports indirect rendering

```rust
if graphics.supports_indirect_rendering() {
    // Indirect rendering automatically enabled
}
```

#### Tier 3: Bindless Rendering

**Requirements:**
- Texture arrays
- Dynamic indexing in shaders
- `SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING`

**Characteristics:**
- All textures in a single array
- One draw call for entire batch
- Texture index passed per instance
- Lowest CPU overhead, highest throughput

**Use when:**
- Many unique textures (10-1000s)
- Modern GPU (Vulkan 1.2+, DX12, Metal 2.3+)

```rust
if graphics.supports_bindless_rendering() {
    // Bindless rendering automatically enabled
}
```

### Capability Detection

The renderer automatically detects capabilities on creation:

```rust
use astrelis_render::batched::BatchedRenderer;

let renderer = BatchedRenderer::new(graphics.clone());

println!("Rendering tier: {:?}", renderer.tier());
// Output: Direct, Indirect, or Bindless
```

## Usage

### Basic Quad Rendering

```rust
use astrelis_render::batched::{BatchedRenderer, QuadInstance};
use astrelis_render::{Color, Transform};

let mut renderer = BatchedRenderer::new(graphics.clone());

// Create quad instances
let instances = vec![
    QuadInstance {
        transform: Transform::from_translation(Vec2::new(100.0, 100.0)),
        color: Color::RED,
        texture_index: 0,  // Use first texture in atlas
    },
    QuadInstance {
        transform: Transform::from_translation(Vec2::new(200.0, 100.0)),
        color: Color::BLUE,
        texture_index: 0,
    },
];

// Render all instances in a single draw call
renderer.render(&mut pass, &instances);
```

### Texture Management

#### Direct/Indirect Tiers

Textures are batched by bind group:

```rust
// Load textures
let texture1 = load_texture("sprite1.png");
let texture2 = load_texture("sprite2.png");

// Renderer batches by texture automatically
renderer.add_texture(texture1);  // Batch 0
renderer.add_texture(texture2);  // Batch 1

// Instances with same texture are drawn together
```

#### Bindless Tier

All textures are stored in a single array:

```rust
// Add textures to array
let idx1 = renderer.add_texture(texture1);  // Index 0
let idx2 = renderer.add_texture(texture2);  // Index 1

// Reference by index in instance data
instances.push(QuadInstance {
    texture_index: idx1,
    // ...
});
```

Maximum textures: 2048 (configurable via `MAX_TEXTURE_ARRAY_SIZE`)

### Transform Batching

Transforms are encoded as 2D affine matrices:

```rust
use astrelis_render::Transform;

// Translation
let t1 = Transform::from_translation(Vec2::new(100.0, 50.0));

// Rotation
let t2 = Transform::from_rotation(std::f32::consts::PI / 4.0);

// Scale
let t3 = Transform::from_scale(Vec2::new(2.0, 2.0));

// Combined
let t4 = Transform::from_translation(Vec2::new(100.0, 50.0))
    .rotate(std::f32::consts::PI / 4.0)
    .scale(Vec2::new(2.0, 2.0));
```

## Performance Optimization

### Instance Sorting

Sort instances by texture to minimize state changes:

```rust
instances.sort_by_key(|inst| inst.texture_index);
renderer.render(&mut pass, &instances);
```

**Direct tier:** Reduces bind group changes
**Indirect tier:** Improves batch coherency
**Bindless tier:** Less critical (single draw call)

### Buffer Management

The renderer uses a dynamic instance buffer that grows as needed:

```rust
// Pre-allocate for known instance count
renderer.reserve_instances(10000);

// Render with automatic buffer growth
renderer.render(&mut pass, &instances);
```

### Culling

Cull offscreen instances before rendering:

```rust
let visible_instances: Vec<_> = instances
    .iter()
    .filter(|inst| viewport.intersects(inst.bounds()))
    .copied()
    .collect();

renderer.render(&mut pass, &visible_instances);
```

### Texture Atlas

Combine small textures into an atlas to reduce texture count:

```rust
use astrelis_render::Atlas;

let mut atlas = Atlas::new(2048, 2048);

// Pack textures
let sprite1_uv = atlas.pack(sprite1);
let sprite2_uv = atlas.pack(sprite2);

// Adjust UV coordinates in instances
instances.push(QuadInstance {
    uv_min: sprite1_uv.min,
    uv_max: sprite1_uv.max,
    // ...
});
```

## Render Pipeline Customization

### Custom Shaders

Override the default quad shader:

```rust
let shader = graphics.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("Custom Quad Shader"),
    source: wgpu::ShaderSource::Wgsl(include_str!("custom_quad.wgsl").into()),
});

renderer.set_shader(shader);
```

**Shader interface:**
- Vertex input: `[[builtin(vertex_index)]] vertex_idx: u32`
- Instance input: `@location(0-7)` instance data
- Fragment output: `@location(0) color: vec4<f32>`

### Blend Modes

Configure alpha blending:

```rust
use astrelis_render::BlendMode;

renderer.set_blend_mode(BlendMode::Alpha);      // Standard alpha blending
renderer.set_blend_mode(BlendMode::Additive);   // Additive blending
renderer.set_blend_mode(BlendMode::Multiply);   // Multiply blending
renderer.set_blend_mode(BlendMode::None);       // Opaque rendering
```

### Depth Testing

Enable depth testing for 3D-like layering:

```rust
renderer.enable_depth_test(true);

// Set depth per instance
instances.push(QuadInstance {
    depth: 0.5,  // 0.0 (near) to 1.0 (far)
    // ...
});
```

## Pipeline Specialization

Create specialized pipelines for specific use cases:

```rust
// UI rendering pipeline (no depth, alpha blending)
let ui_pipeline = renderer.create_pipeline(PipelineConfig {
    depth_test: false,
    blend_mode: BlendMode::Alpha,
    cull_mode: None,
});

// Particle pipeline (additive blending)
let particle_pipeline = renderer.create_pipeline(PipelineConfig {
    depth_test: false,
    blend_mode: BlendMode::Additive,
    cull_mode: None,
});

// Use specialized pipelines
renderer.set_pipeline(ui_pipeline);
renderer.render(&mut pass, &ui_instances);

renderer.set_pipeline(particle_pipeline);
renderer.render(&mut pass, &particle_instances);
```

## Example: Sprite Batch Rendering

```rust
use astrelis_render::batched::{BatchedRenderer, QuadInstance};

struct SpriteRenderer {
    batched: BatchedRenderer,
    sprites: Vec<Sprite>,
}

impl SpriteRenderer {
    fn render(&mut self, pass: &mut RenderPass) {
        // Convert sprites to instances
        let instances: Vec<QuadInstance> = self.sprites
            .iter()
            .map(|sprite| QuadInstance {
                transform: sprite.transform,
                color: sprite.tint,
                texture_index: sprite.texture_id,
                uv_min: sprite.uv_min,
                uv_max: sprite.uv_max,
            })
            .collect();

        // Render all sprites in minimal draw calls
        self.batched.render(pass, &instances);
    }
}
```

## Tier Comparison

| Feature                | Direct | Indirect | Bindless |
|------------------------|--------|----------|----------|
| GPU Compatibility      | All    | Modern   | Latest   |
| Draw Calls (100 textures) | 100 | 100      | 1        |
| CPU Overhead           | Medium | Low      | Lowest   |
| Max Textures           | Unlimited* | Unlimited* | 2048 |
| Memory Usage           | Low    | Medium   | High     |
| Sorting Required       | Yes    | Recommended | Optional |

*Limited by GPU bind group limits (~16-32 per pipeline)

## Debugging

Enable batched renderer debugging:

```rust
renderer.set_debug_mode(true);

// Logs draw call count, instance count, and tier info
renderer.render(&mut pass, &instances);
// Output: "Batched render: 1 draw call, 1000 instances, tier: Bindless"
```

## Example Application

See the `batched_renderer.rs` example:

```bash
cargo run -p astrelis-render --example batched_renderer
```

This demonstrates:
- Automatic tier detection
- Rendering 10,000 quads
- Texture atlas usage
- Performance metrics

## Next Steps

- Explore [GPU Instancing](./gpu-instancing.md) for advanced instancing techniques
- See [Custom Shaders](./custom-shaders.md) for shader programming
- Check [Performance Optimization](../ui/performance-optimization.md) for general optimization
