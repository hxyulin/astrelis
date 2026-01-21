# GPU Instancing

This guide explains how to use GPU instancing in Astrelis to render thousands of objects efficiently. Learn to leverage instanced rendering for massive performance gains.

## Overview

**GPU instancing** allows rendering many copies of the same mesh with a single draw call. Instead of:

```rust
// Slow: 1000 draw calls
for object in objects {
    render_pass.draw(0..vertex_count, 0..1);
}
```

You do:

```rust
// Fast: 1 draw call for 1000 instances
render_pass.draw(0..vertex_count, 0..1000);
```

**Performance:**
- **Without instancing:** 1000 objects = 1000 draw calls = ~15ms
- **With instancing:** 1000 objects = 1 draw call = ~0.5ms

**Use Cases:**
- Particle systems (1000s of particles)
- Foliage rendering (trees, grass)
- Crowd rendering (NPCs)
- Repeated objects (buildings, rocks)

**Comparison to Unity:** Similar to `Graphics.DrawMeshInstanced()` or `Graphics.DrawMeshInstancedIndirect()`.

## How Instancing Works

### Regular Rendering

```text
Draw Call 1: Mesh A, Transform 1
Draw Call 2: Mesh A, Transform 2
Draw Call 3: Mesh A, Transform 3
...
```

Each draw call has CPU overhead and GPU state changes.

### Instanced Rendering

```text
Draw Call 1: Mesh A, Transforms [1, 2, 3, ..., 1000]
```

GPU automatically runs vertex shader for each instance with different per-instance data.

**Key Concept:** Vertex attributes can be **per-vertex** (positions, normals) or **per-instance** (transforms, colors).

## Basic Instancing Example

### Step 1: Create Vertex Data (Per-Vertex)

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
}

// Create a cube mesh (shared by all instances)
let vertices = create_cube_vertices();

let vertex_buffer = graphics.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label: Some("Vertex Buffer"),
    contents: bytemuck::cast_slice(&vertices),
    usage: wgpu::BufferUsages::VERTEX,
});
```

### Step 2: Create Instance Data (Per-Instance)

```rust
use glam::{Vec3, Mat4};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    model_matrix: [[f32; 4]; 4], // 4x4 matrix
    color: [f32; 4],              // RGBA
}

// Create 1000 instances in a grid
let mut instances = Vec::new();
let grid_size = 10;

for x in 0..grid_size {
    for y in 0..grid_size {
        for z in 0..grid_size {
            let position = Vec3::new(
                x as f32 * 2.0,
                y as f32 * 2.0,
                z as f32 * 2.0,
            );

            let model_matrix = Mat4::from_translation(position);

            let color = [
                x as f32 / grid_size as f32,
                y as f32 / grid_size as f32,
                z as f32 / grid_size as f32,
                1.0,
            ];

            instances.push(Instance {
                model_matrix: model_matrix.to_cols_array_2d(),
                color,
            });
        }
    }
}

let instance_buffer = graphics.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label: Some("Instance Buffer"),
    contents: bytemuck::cast_slice(&instances),
    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
});
```

### Step 3: Configure Vertex Buffers

```rust
use wgpu::*;

// Vertex buffer layout (per-vertex data)
let vertex_buffer_layout = VertexBufferLayout {
    array_stride: std::mem::size_of::<Vertex>() as u64,
    step_mode: VertexStepMode::Vertex, // Per vertex
    attributes: &[
        // Position
        VertexAttribute {
            format: VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        },
        // Normal
        VertexAttribute {
            format: VertexFormat::Float32x3,
            offset: 12,
            shader_location: 1,
        },
    ],
};

// Instance buffer layout (per-instance data)
let instance_buffer_layout = VertexBufferLayout {
    array_stride: std::mem::size_of::<Instance>() as u64,
    step_mode: VertexStepMode::Instance, // Per instance
    attributes: &[
        // Model matrix column 0
        VertexAttribute {
            format: VertexFormat::Float32x4,
            offset: 0,
            shader_location: 2,
        },
        // Model matrix column 1
        VertexAttribute {
            format: VertexFormat::Float32x4,
            offset: 16,
            shader_location: 3,
        },
        // Model matrix column 2
        VertexAttribute {
            format: VertexFormat::Float32x4,
            offset: 32,
            shader_location: 4,
        },
        // Model matrix column 3
        VertexAttribute {
            format: VertexFormat::Float32x4,
            offset: 48,
            shader_location: 5,
        },
        // Color
        VertexAttribute {
            format: VertexFormat::Float32x4,
            offset: 64,
            shader_location: 6,
        },
    ],
};
```

**Key:** `step_mode: VertexStepMode::Instance` makes attributes per-instance.

### Step 4: Shader with Instancing

```wgsl
// Per-vertex attributes
struct VertexInput {
    @location(0) position: vec3f,
    @location(1) normal: vec3f,
}

// Per-instance attributes
struct InstanceInput {
    @location(2) model_matrix_0: vec4f,
    @location(3) model_matrix_1: vec4f,
    @location(4) model_matrix_2: vec4f,
    @location(5) model_matrix_3: vec4f,
    @location(6) color: vec4f,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) color: vec4f,
    @location(1) normal: vec3f,
}

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var output: VertexOutput;

    // Reconstruct model matrix from columns
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    // Transform position
    let world_pos = model_matrix * vec4f(vertex.position, 1.0);
    output.clip_position = uniforms.view_proj * world_pos;

    // Transform normal
    output.normal = (model_matrix * vec4f(vertex.normal, 0.0)).xyz;

    // Pass instance color
    output.color = instance.color;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    // Simple lighting
    let light_dir = normalize(vec3f(1.0, 1.0, 1.0));
    let diffuse = max(dot(normalize(input.normal), light_dir), 0.0);

    let lighting = 0.3 + diffuse * 0.7; // Ambient + diffuse
    let final_color = input.color.rgb * lighting;

    return vec4f(final_color, input.color.a);
}
```

### Step 5: Create Pipeline

```rust
let pipeline = graphics.device.create_render_pipeline(&RenderPipelineDescriptor {
    label: Some("Instanced Pipeline"),
    layout: Some(&pipeline_layout),
    vertex: VertexState {
        module: &shader_module,
        entry_point: "vs_main",
        buffers: &[
            vertex_buffer_layout,   // Buffer 0: per-vertex
            instance_buffer_layout, // Buffer 1: per-instance
        ],
    },
    fragment: Some(FragmentState {
        module: &shader_module,
        entry_point: "fs_main",
        targets: &[Some(ColorTargetState {
            format: surface_format,
            blend: Some(BlendState::ALPHA_BLENDING),
            write_mask: ColorWrites::ALL,
        })],
    }),
    primitive: PrimitiveState {
        topology: PrimitiveTopology::TriangleList,
        cull_mode: Some(Face::Back),
        ..Default::default()
    },
    depth_stencil: Some(DepthStencilState {
        format: TextureFormat::Depth32Float,
        depth_write_enabled: true,
        depth_compare: CompareFunction::Less,
        stencil: StencilState::default(),
        bias: DepthBiasState::default(),
    }),
    multisample: MultisampleState::default(),
    multiview: None,
});
```

### Step 6: Render with Instancing

```rust
render_pass.set_pipeline(&pipeline);
render_pass.set_bind_group(0, &uniform_bind_group, &[]);
render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));   // Per-vertex
render_pass.set_vertex_buffer(1, instance_buffer.slice(..)); // Per-instance

let vertex_count = vertices.len() as u32;
let instance_count = instances.len() as u32;

render_pass.draw(0..vertex_count, 0..instance_count); // Draw all instances!
```

**Result:** 1000 cubes rendered with 1 draw call.

## Dynamic Instancing

Update instance data each frame:

```rust
// Update instances (e.g., animate positions)
for (i, instance) in instances.iter_mut().enumerate() {
    let time = current_time + i as f32 * 0.1;
    let y_offset = (time * 2.0).sin() * 2.0;

    let position = original_positions[i] + Vec3::new(0.0, y_offset, 0.0);
    let model_matrix = Mat4::from_translation(position);

    instance.model_matrix = model_matrix.to_cols_array_2d();
}

// Upload updated instances to GPU
graphics.queue.write_buffer(
    &instance_buffer,
    0,
    bytemuck::cast_slice(&instances),
);

// Render (same as before)
render_pass.draw(0..vertex_count, 0..instance_count);
```

**Performance:** Uploading 1000 instances = ~0.1ms (fast).

## Per-Instance Data Patterns

### Transform Only

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TransformInstance {
    model_matrix: [[f32; 4]; 4],
}
```

**Use Case:** All instances use same material.

### Transform + Color

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ColoredInstance {
    model_matrix: [[f32; 4]; 4],
    color: [f32; 4],
}
```

**Use Case:** Different colored objects.

### Transform + UV Offset

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SpriteInstance {
    position: [f32; 2],
    size: [f32; 2],
    uv_offset: [f32; 2],
    uv_scale: [f32; 2],
    color: [f32; 4],
}
```

**Use Case:** Sprite sheets, texture atlases.

### Transform + Material Index

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialInstance {
    model_matrix: [[f32; 4]; 4],
    material_index: u32, // Index into texture array
}
```

**Shader:**
```wgsl
@group(1) @binding(0)
var texture_array: texture_2d_array<f32>;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    let texture_index = input.material_index;
    return textureSample(texture_array, sampler, input.uv, texture_index);
}
```

**Use Case:** Many objects with different textures.

## Buffer Layout and Alignment

### Alignment Rules

WGPU requires proper alignment for vertex attributes:

| Type | Size | Alignment |
|------|------|-----------|
| `f32` | 4 bytes | 4 bytes |
| `vec2<f32>` | 8 bytes | 8 bytes |
| `vec3<f32>` | 12 bytes | **16 bytes** |
| `vec4<f32>` | 16 bytes | 16 bytes |
| `mat4x4<f32>` | 64 bytes | 16 bytes |

**Important:** `vec3` is padded to 16 bytes!

### Correctly Aligned Instance

```rust
#[repr(C, align(16))] // Force 16-byte alignment
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    model_matrix: [[f32; 4]; 4], // 64 bytes, 16-byte aligned
    color: [f32; 4],              // 16 bytes
    // Total: 80 bytes (multiple of 16)
}
```

### Incorrectly Aligned Instance (Don't Do This)

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BadInstance {
    position: [f32; 3], // 12 bytes, but needs 16-byte alignment!
    color: [f32; 4],    // 16 bytes
    // Total: 28 bytes (not multiple of 16)
}
// This will cause rendering errors or crashes
```

**Fix:** Add padding or use `vec4` instead of `vec3`:
```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GoodInstance {
    position: [f32; 4],  // 16 bytes (4th component unused)
    color: [f32; 4],     // 16 bytes
    // Total: 32 bytes (multiple of 16)
}
```

## Performance Optimization

### Culling

Don't render instances outside the camera view:

```rust
// Frustum culling
let frustum = Frustum::from_view_proj(view_proj_matrix);

let mut visible_instances = Vec::new();
for instance in &all_instances {
    let bounding_sphere = instance.get_bounding_sphere();
    if frustum.contains_sphere(bounding_sphere) {
        visible_instances.push(*instance);
    }
}

// Update instance buffer with only visible instances
graphics.queue.write_buffer(
    &instance_buffer,
    0,
    bytemuck::cast_slice(&visible_instances),
);

// Render only visible instances
render_pass.draw(0..vertex_count, 0..visible_instances.len() as u32);
```

**Benefit:** Avoid rendering off-screen objects (GPU savings).

### Level of Detail (LOD)

Use lower-poly meshes for distant instances:

```rust
struct InstanceBatch {
    instances: Vec<Instance>,
    mesh: Mesh,
}

// Group instances by distance
let close_instances = instances.iter().filter(|i| distance(i, camera) < 10.0);
let far_instances = instances.iter().filter(|i| distance(i, camera) >= 10.0);

// Render close instances with high-poly mesh
render_pass.set_vertex_buffer(0, high_poly_mesh.vertex_buffer.slice(..));
render_pass.draw(0..high_poly_mesh.vertex_count, 0..close_instances.len() as u32);

// Render far instances with low-poly mesh
render_pass.set_vertex_buffer(0, low_poly_mesh.vertex_buffer.slice(..));
render_pass.draw(0..low_poly_mesh.vertex_count, 0..far_instances.len() as u32);
```

### Batching by Material

Group instances by material to minimize state changes:

```rust
// Sort instances by material ID
instances.sort_by_key(|i| i.material_id);

// Render in batches
let mut current_material = None;
let mut batch_start = 0;

for (i, instance) in instances.iter().enumerate() {
    if current_material != Some(instance.material_id) {
        if let Some(mat_id) = current_material {
            // Render previous batch
            render_pass.draw(0..vertex_count, batch_start..i as u32);
        }

        // Switch material
        render_pass.set_bind_group(1, &materials[instance.material_id].bind_group, &[]);
        current_material = Some(instance.material_id);
        batch_start = i as u32;
    }
}

// Render final batch
if batch_start < instances.len() as u32 {
    render_pass.draw(0..vertex_count, batch_start..instances.len() as u32);
}
```

## Indirect Instancing (Advanced)

For GPU-driven culling and LOD:

```rust
// Indirect draw buffer (computed on GPU)
let indirect_buffer = graphics.device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("Indirect Draw Buffer"),
    size: std::mem::size_of::<wgpu::util::DrawIndexedIndirect>() as u64,
    usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::STORAGE,
    mapped_at_creation: false,
});

// Compute shader generates draw commands
compute_pass.dispatch_workgroups(num_workgroups, 1, 1);

// Execute indirect draw
render_pass.draw_indexed_indirect(&indirect_buffer, 0);
```

**Use Case:** Dynamic culling on GPU, large-scale rendering (100,000+ instances).

## Troubleshooting

### Instances Not Visible

**Cause:** Incorrect instance buffer layout or alignment.

**Fix:** Verify alignment with `#[repr(C, align(16))]` and check buffer stride.

### Some Instances Render Incorrectly

**Cause:** Buffer size mismatch.

**Fix:** Ensure buffer size matches instance count:
```rust
let buffer_size = (instances.len() * std::mem::size_of::<Instance>()) as u64;
```

### Performance Not Improved

**Cause:** CPU bottleneck (updating instance buffer every frame).

**Fix:** Only update instances that changed, or use compute shaders for updates.

## Comparison to Unity

| Unity | Astrelis | Notes |
|-------|----------|-------|
| `Graphics.DrawMeshInstanced()` | `render_pass.draw(..., 0..N)` | Same concept |
| `MaterialPropertyBlock` | Instance buffer | Per-instance data |
| `Matrix4x4[]` | `[[f32; 4]; 4]` | Transform matrices |
| `ComputeBuffer` | Indirect buffer | GPU-driven rendering |

## Next Steps

- **Practice:** Try the `instanced_rendering` example (when added)
- **Learn More:** [Compute Shaders](compute-shaders.md) for GPU-driven culling
- **Advanced:** Indirect rendering for massive scale (1M+ instances)
- **Examples:** `performance_benchmark` uses instancing

## See Also

- [Custom Shaders](custom-shaders.md) - Writing instancing shaders
- [Compute Shaders](compute-shaders.md) - GPU-driven instancing
- [Materials and Textures](materials-and-textures.md) - Material batching
- API Reference: [`wgpu::RenderPass::draw`](https://docs.rs/wgpu/latest/wgpu/struct.RenderPass.html#method.draw)
