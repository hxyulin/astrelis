# Compute Shaders

This guide explains how to use compute shaders in Astrelis for general-purpose GPU computation. Learn to leverage GPU parallelism for particle systems, physics, image processing, and more.

## Overview

**Compute shaders** are GPU programs for parallel computation that don't directly produce rendered output. They're perfect for:

- **Particle systems**: Simulate thousands of particles
- **Physics**: Collision detection, fluid simulation
- **Image processing**: Blur, edge detection, filters
- **Procedural generation**: Terrain, textures
- **Data processing**: Sorting, culling, animation

**Key Advantage:** Massively parallel execution (thousands of threads).

**Comparison to Graphics Shaders:**
- Graphics: vertex → fragment pipeline
- Compute: arbitrary parallel computation

## Compute Shader Basics

### Workgroups and Threads

Compute shaders execute in a hierarchy:

```text
Dispatch (e.g., 100x100x1)
├─ Workgroup 0,0,0 (e.g., 8x8x1 threads)
│  ├─ Thread 0,0,0
│  ├─ Thread 0,1,0
│  └─ ... (64 threads total)
├─ Workgroup 0,1,0
│  └─ ... (64 threads)
└─ ... (10,000 workgroups)
```

**Terminology:**
- **Workgroup**: Group of threads that can share memory
- **Thread**: Single execution unit
- **Dispatch**: Launch N workgroups

### WGSL Compute Shader Structure

```wgsl
// Define workgroup size (threads per workgroup)
@compute @workgroup_size(8, 8, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
) {
    // global_id: Unique ID across all threads
    // local_id: ID within workgroup (0..7 in each dimension)
    // workgroup_id: Workgroup ID

    // Compute work here
}
```

**Built-ins:**
- `global_invocation_id`: Global thread index (0..N-1)
- `local_invocation_id`: Local thread index within workgroup
- `workgroup_id`: Workgroup index
- `num_workgroups`: Total number of workgroups dispatched

## Your First Compute Shader: Array Doubling

### Step 1: Write the Compute Shader

Create `shaders/double_array.wgsl`:

```wgsl
// Input buffer (read-only)
@group(0) @binding(0)
var<storage, read> input: array<f32>;

// Output buffer (write-only)
@group(0) @binding(1)
var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    // Bounds check
    if (index >= arrayLength(&input)) {
        return;
    }

    // Double the value
    output[index] = input[index] * 2.0;
}
```

**Key Points:**
- `@group(0) @binding(N)` binds storage buffers
- `var<storage, read>` for read-only buffers
- `var<storage, read_write>` for read-write buffers
- `arrayLength()` gets buffer element count
- Always bounds-check indices

### Step 2: Create Buffers

```rust
use wgpu::util::DeviceExt;

let input_data: Vec<f32> = (0..1000).map(|i| i as f32).collect();

// Input buffer (GPU can read)
let input_buffer = graphics.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label: Some("Input Buffer"),
    contents: bytemuck::cast_slice(&input_data),
    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
});

// Output buffer (GPU can write)
let output_buffer = graphics.device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("Output Buffer"),
    size: (input_data.len() * std::mem::size_of::<f32>()) as u64,
    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    mapped_at_creation: false,
});

// Staging buffer (for reading results back to CPU)
let staging_buffer = graphics.device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("Staging Buffer"),
    size: (input_data.len() * std::mem::size_of::<f32>()) as u64,
    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
    mapped_at_creation: false,
});
```

### Step 3: Create Compute Pipeline

```rust
// Load shader
let shader_source = std::fs::read_to_string("shaders/double_array.wgsl")?;
let shader_module = graphics.device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("Double Array Compute Shader"),
    source: wgpu::ShaderSource::Wgsl(shader_source.into()),
});

// Create bind group layout
let bind_group_layout = graphics.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    label: Some("Compute Bind Group Layout"),
    entries: &[
        // Input buffer (binding 0)
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        // Output buffer (binding 1)
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
    ],
});

// Create pipeline layout
let pipeline_layout = graphics.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
    label: Some("Compute Pipeline Layout"),
    bind_group_layouts: &[&bind_group_layout],
    push_constant_ranges: &[],
});

// Create compute pipeline
let compute_pipeline = graphics.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
    label: Some("Double Array Pipeline"),
    layout: Some(&pipeline_layout),
    module: &shader_module,
    entry_point: "main",
});
```

### Step 4: Create Bind Group

```rust
let bind_group = graphics.device.create_bind_group(&wgpu::BindGroupDescriptor {
    label: Some("Compute Bind Group"),
    layout: &bind_group_layout,
    entries: &[
        wgpu::BindGroupEntry {
            binding: 0,
            resource: input_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: output_buffer.as_entire_binding(),
        },
    ],
});
```

### Step 5: Dispatch Compute Shader

```rust
// Create command encoder
let mut encoder = graphics.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
    label: Some("Compute Encoder"),
});

{
    let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("Compute Pass"),
    });

    compute_pass.set_pipeline(&compute_pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);

    // Dispatch workgroups
    let workgroup_size = 256;
    let num_elements = input_data.len() as u32;
    let num_workgroups = (num_elements + workgroup_size - 1) / workgroup_size;

    compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
}

// Copy output to staging buffer
encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_buffer.size());

// Submit commands
graphics.queue.submit(Some(encoder.finish()));
```

### Step 6: Read Results

```rust
// Map staging buffer for reading
let buffer_slice = staging_buffer.slice(..);
let (tx, rx) = futures::channel::oneshot::channel();

buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
    tx.send(result).unwrap();
});

// Wait for GPU to finish
graphics.device.poll(wgpu::Maintain::Wait);

// Receive result
rx.await.unwrap().unwrap();

// Read data
let data = buffer_slice.get_mapped_range();
let result: &[f32] = bytemuck::cast_slice(&data);

println!("First 10 results: {:?}", &result[0..10]);
// Output: [0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0]

// Unmap buffer
drop(data);
staging_buffer.unmap();
```

## Workgroup Sizing

### Choosing Workgroup Size

| Workgroup Size | Threads | Use Case |
|----------------|---------|----------|
| `@workgroup_size(1)` | 1 | Sequential work |
| `@workgroup_size(64)` | 64 | Small workloads |
| `@workgroup_size(256)` | 256 | General purpose |
| `@workgroup_size(8, 8)` | 64 | 2D data (images) |
| `@workgroup_size(4, 4, 4)` | 64 | 3D data (volumes) |

**Guidelines:**
- Use powers of 2 (64, 128, 256)
- Desktop: 256 threads typical
- Mobile: 64-128 threads typical
- Match data dimensions for images

### Calculating Dispatch Count

```rust
fn calculate_dispatch(
    element_count: u32,
    workgroup_size: u32,
) -> u32 {
    (element_count + workgroup_size - 1) / workgroup_size
}

// Example: 1000 elements, workgroup size 256
let num_workgroups = calculate_dispatch(1000, 256);
// Result: 4 workgroups (256 + 256 + 256 + 232)

compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
```

### 2D Dispatch (Image Processing)

```rust
let image_width = 1920u32;
let image_height = 1080u32;
let workgroup_size = 8u32; // 8x8 = 64 threads

let workgroups_x = (image_width + workgroup_size - 1) / workgroup_size;
let workgroups_y = (image_height + workgroup_size - 1) / workgroup_size;

compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
```

**Shader:**
```wgsl
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_x = global_id.x;
    let pixel_y = global_id.y;

    // Process pixel at (pixel_x, pixel_y)
}
```

## Storage Buffers

### Read-Only Buffers

```wgsl
@group(0) @binding(0)
var<storage, read> input: array<vec4f>;

// Access
let value = input[index];
```

### Read-Write Buffers

```wgsl
@group(0) @binding(1)
var<storage, read_write> output: array<vec4f>;

// Access
output[index] = vec4f(1.0, 0.0, 0.0, 1.0);
```

### Structured Buffers

```wgsl
struct Particle {
    position: vec3f,
    velocity: vec3f,
    life: f32,
}

@group(0) @binding(0)
var<storage, read_write> particles: array<Particle>;

// Access
particles[index].position += particles[index].velocity * delta_time;
```

## Shared Memory (Workgroup Memory)

Threads in a workgroup can share memory:

```wgsl
var<workgroup> shared_data: array<f32, 256>;

@compute @workgroup_size(256)
fn main(
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
) {
    let local_index = local_id.x;

    // Load data into shared memory
    shared_data[local_index] = input[global_id.x];

    // Synchronize workgroup threads
    workgroupBarrier();

    // All threads can now access shared_data
    let neighbor_value = shared_data[(local_index + 1u) % 256u];
}
```

**Key Functions:**
- `workgroupBarrier()`: Wait for all threads in workgroup
- `storageBarrier()`: Synchronize storage access

**Use Cases:** Reduction operations, prefix sums, convolution filters

## Complete Example: Particle System

Simulate 10,000 particles on GPU:

```wgsl
struct Particle {
    position: vec2f,
    velocity: vec2f,
    color: vec4f,
    life: f32,
}

struct SimulationParams {
    delta_time: f32,
    gravity: vec2f,
}

@group(0) @binding(0)
var<storage, read_write> particles: array<Particle>;

@group(0) @binding(1)
var<uniform> params: SimulationParams;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;

    if (index >= arrayLength(&particles)) {
        return;
    }

    var particle = particles[index];

    // Apply gravity
    particle.velocity += params.gravity * params.delta_time;

    // Update position
    particle.position += particle.velocity * params.delta_time;

    // Bounce off ground
    if (particle.position.y < 0.0) {
        particle.position.y = 0.0;
        particle.velocity.y *= -0.8; // Damping
    }

    // Age particle
    particle.life -= params.delta_time;

    // Fade out
    particle.color.a = max(particle.life, 0.0);

    // Write back
    particles[index] = particle;
}
```

**Rust setup:**
```rust
// Create particle buffer
let particle_count = 10_000;
let mut particles = vec![Particle::default(); particle_count];

// Initialize particles...

let particle_buffer = graphics.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label: Some("Particle Buffer"),
    contents: bytemuck::cast_slice(&particles),
    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
});

// Update each frame
let params = SimulationParams {
    delta_time: frame_time,
    gravity: Vec2::new(0.0, -9.8),
};

graphics.queue.write_buffer(&param_buffer, 0, bytemuck::cast_slice(&[params]));

// Dispatch compute
let workgroups = (particle_count as u32 + 255) / 256;
compute_pass.dispatch_workgroups(workgroups, 1, 1);

// Render particles as instanced points
render_pass.set_vertex_buffer(0, particle_buffer.slice(..));
render_pass.draw(0..4, 0..particle_count as u32); // Instanced quads
```

## Complete Example: Image Blur

Apply Gaussian blur to an image:

```wgsl
@group(0) @binding(0)
var input_texture: texture_2d<f32>;

@group(0) @binding(1)
var output_texture: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_coords = vec2<i32>(global_id.xy);
    let texture_size = textureDimensions(input_texture);

    // Bounds check
    if (pixel_coords.x >= i32(texture_size.x) || pixel_coords.y >= i32(texture_size.y)) {
        return;
    }

    // 3x3 Gaussian kernel
    let weights = array<f32, 9>(
        1.0, 2.0, 1.0,
        2.0, 4.0, 2.0,
        1.0, 2.0, 1.0,
    );

    let offsets = array<vec2<i32>, 9>(
        vec2(-1, -1), vec2(0, -1), vec2(1, -1),
        vec2(-1,  0), vec2(0,  0), vec2(1,  0),
        vec2(-1,  1), vec2(0,  1), vec2(1,  1),
    );

    var color = vec4f(0.0);
    var weight_sum = 0.0;

    // Sample neighbors
    for (var i = 0u; i < 9u; i++) {
        let sample_coords = pixel_coords + offsets[i];

        // Clamp to texture bounds
        if (sample_coords.x >= 0 && sample_coords.x < i32(texture_size.x) &&
            sample_coords.y >= 0 && sample_coords.y < i32(texture_size.y)) {

            let sample_color = textureLoad(input_texture, sample_coords, 0);
            color += sample_color * weights[i];
            weight_sum += weights[i];
        }
    }

    // Normalize
    color /= weight_sum;

    // Write output
    textureStore(output_texture, pixel_coords, color);
}
```

**Rust setup:**
```rust
// Create storage texture for output
let output_texture = graphics.device.create_texture(&wgpu::TextureDescriptor {
    label: Some("Output Texture"),
    size: wgpu::Extent3d {
        width: image_width,
        height: image_height,
        depth_or_array_layers: 1,
    },
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format: wgpu::TextureFormat::Rgba8Unorm,
    usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
    view_formats: &[],
});

// Dispatch
let workgroups_x = (image_width + 7) / 8;
let workgroups_y = (image_height + 7) / 8;
compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
```

## Synchronization

### Barriers

```wgsl
// Wait for all threads in workgroup
workgroupBarrier();

// Wait for storage writes
storageBarrier();
```

**Use Cases:**
- After writing shared memory, before reading
- After writing storage buffer, before reading from it

### Read-After-Write Hazards

```rust
// WRONG: Read output buffer immediately after write
compute_pass.dispatch_workgroups(workgroups, 1, 1);
// ... compute writes to output_buffer
render_pass.set_vertex_buffer(0, output_buffer.slice(..)); // HAZARD!

// CORRECT: Use separate command buffers or barriers
let mut compute_encoder = device.create_command_encoder(...);
// ... compute pass
queue.submit(Some(compute_encoder.finish()));

let mut render_encoder = device.create_command_encoder(...);
// ... render pass uses output_buffer
queue.submit(Some(render_encoder.finish()));
```

## Performance Tips

### Optimize Memory Access

```wgsl
// BAD: Strided access (slow)
for (var i = 0u; i < 1000u; i++) {
    let value = data[global_id.x * 1000u + i]; // Thread 0: 0, 1000, 2000, ...
}

// GOOD: Coalesced access (fast)
for (var i = 0u; i < 1000u; i++) {
    let value = data[i * num_threads + global_id.x]; // Thread 0: 0, 1, 2, ...
}
```

### Use Shared Memory for Repeated Access

```wgsl
// Load once, use many times
var<workgroup> shared: array<f32, 256>;

shared[local_id.x] = global_data[global_id.x];
workgroupBarrier();

// All threads can access shared memory (much faster than global)
let sum = shared[0] + shared[1] + shared[2];
```

### Minimize Thread Divergence

```wgsl
// BAD: Threads take different paths
if (global_id.x % 2u == 0u) {
    // Even threads do work A
} else {
    // Odd threads do work B (GPU serializes this)
}

// GOOD: All threads do same work
let work_index = global_id.x;
process(work_index);
```

## Troubleshooting

### "Array index out of bounds"

**Cause:** Dispatched too many threads or forgot bounds check.

**Fix:** Always bounds-check:
```wgsl
if (index >= arrayLength(&buffer)) {
    return;
}
```

### Results are Wrong

**Cause:** Missing `workgroupBarrier()` or `storageBarrier()`.

**Fix:** Add barriers after shared memory writes:
```wgsl
shared_data[local_id.x] = value;
workgroupBarrier(); // Wait for all writes
let neighbor = shared_data[other_index];
```

### Slow Performance

**Cause:** Poor memory access patterns.

**Fix:** Profile with tools like RenderDoc or NSight, optimize memory access.

## Next Steps

- **Practice:** Try the `compute_particles` example (when added)
- **Learn More:** [GPU Instancing](gpu-instancing.md) for rendering compute results
- **Advanced:** [WGSL Spec](https://www.w3.org/TR/WGSL/) for complete language reference
- **Examples:** `performance_benchmark` uses compute shaders

## See Also

- [Custom Shaders](custom-shaders.md) - WGSL basics
- [GPU Instancing](gpu-instancing.md) - Rendering many objects
- [Render Passes](render-passes.md) - Integrating compute with rendering
- API Reference: [`wgpu::ComputePipeline`](https://docs.rs/wgpu/latest/wgpu/struct.ComputePipeline.html)
