# Custom Shaders

This guide teaches you how to write custom shaders in WGSL (WebGPU Shading Language) for Astrelis. Learn to create custom rendering effects, materials, and visual styles.

## Overview

**Shaders** are programs that run on the GPU to render graphics. Astrelis uses **WGSL** (WebGPU Shading Language), a modern shader language designed for WebGPU.

**Shader Types:**
- **Vertex Shader**: Processes vertex data (positions, colors, UVs)
- **Fragment Shader**: Computes pixel colors
- **Compute Shader**: General-purpose GPU computation (see [Compute Shaders](compute-shaders.md))

**Comparison to Other Languages:**
- **GLSL** (OpenGL): Similar syntax, different built-ins
- **HLSL** (DirectX): Similar concepts, different naming
- **Metal Shading Language**: Similar to WGSL

## WGSL Quick Start

### For GLSL Developers

| GLSL | WGSL | Notes |
|------|------|-------|
| `attribute` | `@location(0)` | Vertex inputs |
| `varying` | `@location(0)` | Vertex outputs |
| `uniform` | `@group(0) @binding(0)` | Uniforms |
| `vec2`, `vec3`, `vec4` | `vec2f`, `vec3f`, `vec4f` | Explicit float type |
| `mat4` | `mat4x4<f32>` | Matrix type |
| `texture2D(tex, uv)` | `textureSample(tex, samp, uv)` | Separate sampler |
| `gl_Position` | `@builtin(position)` | Built-in outputs |
| `gl_FragColor` | `@location(0)` | Fragment output |

### For HLSL Developers

| HLSL | WGSL | Notes |
|------|------|-------|
| `float2`, `float3`, `float4` | `vec2f`, `vec3f`, `vec4f` | Vector types |
| `float4x4` | `mat4x4<f32>` | Matrix types |
| `Texture2D` | `texture_2d<f32>` | Texture type |
| `SamplerState` | `sampler` | Sampler type |
| `tex.Sample(samp, uv)` | `textureSample(tex, samp, uv)` | Sampling |
| `SV_Position` | `@builtin(position)` | Built-in |
| `SV_Target` | `@location(0)` | Render target |

### Basic Syntax

```wgsl
// Variables
var position: vec3f = vec3f(0.0, 0.0, 0.0);
let color: vec4f = vec4f(1.0, 0.0, 0.0, 1.0); // Immutable

// Functions
fn add(a: f32, b: f32) -> f32 {
    return a + b;
}

// Structures
struct VertexInput {
    @location(0) position: vec3f,
    @location(1) color: vec4f,
}

// Entry points
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    // Vertex shader code
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    // Fragment shader code
}
```

## Your First Shader: Colored Triangle

### Step 1: Write the Shader

Create `shaders/colored_triangle.wgsl`:

```wgsl
// Vertex shader input
struct VertexInput {
    @location(0) position: vec2f,
    @location(1) color: vec4f,
}

// Vertex shader output (fragment shader input)
struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
}

// Vertex shader
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Convert 2D position to 4D clip space
    output.position = vec4f(input.position, 0.0, 1.0);

    // Pass color to fragment shader
    output.color = input.color;

    return output;
}

// Fragment shader
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    // Output the interpolated color
    return input.color;
}
```

**Key Points:**
- `@vertex` marks the vertex shader entry point
- `@fragment` marks the fragment shader entry point
- `@location(N)` defines vertex attributes and outputs
- `@builtin(position)` is the clip-space position output

### Step 2: Create Vertex Data

```rust
use astrelis_core::math::Vec2;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ColoredVertex {
    position: Vec2,
    color: [f32; 4],
}

let vertices = [
    ColoredVertex {
        position: Vec2::new(0.0, 0.5),
        color: [1.0, 0.0, 0.0, 1.0], // Red
    },
    ColoredVertex {
        position: Vec2::new(-0.5, -0.5),
        color: [0.0, 1.0, 0.0, 1.0], // Green
    },
    ColoredVertex {
        position: Vec2::new(0.5, -0.5),
        color: [0.0, 0.0, 1.0, 1.0], // Blue
    },
];

let vertex_buffer = graphics.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label: Some("Triangle Vertex Buffer"),
    contents: bytemuck::cast_slice(&vertices),
    usage: wgpu::BufferUsages::VERTEX,
});
```

**Important:** Vertex layout must match shader `@location` attributes.

### Step 3: Load the Shader

```rust
use std::fs;

let shader_source = fs::read_to_string("shaders/colored_triangle.wgsl")
    .expect("Failed to read shader file");

let shader_module = graphics.device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("Colored Triangle Shader"),
    source: wgpu::ShaderSource::Wgsl(shader_source.into()),
});
```

### Step 4: Create Render Pipeline

```rust
use wgpu::*;

// Vertex buffer layout
let vertex_buffer_layout = VertexBufferLayout {
    array_stride: std::mem::size_of::<ColoredVertex>() as u64,
    step_mode: VertexStepMode::Vertex,
    attributes: &[
        // Position (location = 0)
        VertexAttribute {
            format: VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        },
        // Color (location = 1)
        VertexAttribute {
            format: VertexFormat::Float32x4,
            offset: 8, // After 2 floats (position)
            shader_location: 1,
        },
    ],
};

let pipeline = graphics.device.create_render_pipeline(&RenderPipelineDescriptor {
    label: Some("Colored Triangle Pipeline"),
    layout: None, // Automatic layout
    vertex: VertexState {
        module: &shader_module,
        entry_point: "vs_main",
        buffers: &[vertex_buffer_layout],
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
        ..Default::default()
    },
    depth_stencil: None,
    multisample: MultisampleState::default(),
    multiview: None,
});
```

### Step 5: Render

```rust
let mut frame = renderable_window.begin_drawing();

frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        let render_pass = pass.wgpu_pass();
        render_pass.set_pipeline(&pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..3, 0..1); // 3 vertices, 1 instance
    },
);

frame.finish();
```

**Result:** A triangle with red, green, and blue vertices (colors interpolated).

## Uniforms and Bind Groups

### Adding Uniform Data

Uniforms pass constant data to shaders (transforms, colors, time, etc.).

**Shader with uniform:**

```wgsl
// Uniform buffer structure
struct Uniforms {
    transform: mat4x4<f32>,
    color_tint: vec4f,
}

// Bind group with uniform
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Apply transform matrix
    let position_4d = vec4f(input.position, 0.0, 1.0);
    output.position = uniforms.transform * position_4d;

    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    // Apply color tint
    return input.color * uniforms.color_tint;
}
```

**Rust uniform struct:**

```rust
use glam::{Mat4, Vec4};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    transform: Mat4,
    color_tint: Vec4,
}

// Create uniform buffer
let uniforms = Uniforms {
    transform: Mat4::IDENTITY,
    color_tint: Vec4::new(1.0, 1.0, 1.0, 1.0),
};

let uniform_buffer = graphics.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label: Some("Uniform Buffer"),
    contents: bytemuck::cast_slice(&[uniforms]),
    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
});
```

**Create bind group:**

```rust
let bind_group_layout = graphics.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    label: Some("Uniform Bind Group Layout"),
    entries: &[
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
    ],
});

let bind_group = graphics.device.create_bind_group(&wgpu::BindGroupDescriptor {
    label: Some("Uniform Bind Group"),
    layout: &bind_group_layout,
    entries: &[
        wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        },
    ],
});
```

**Use in render pass:**

```rust
render_pass.set_pipeline(&pipeline);
render_pass.set_bind_group(0, &bind_group, &[]); // group(0) in shader
render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
render_pass.draw(0..3, 0..1);
```

**Update uniforms per frame:**

```rust
// Update uniform data
let new_uniforms = Uniforms {
    transform: Mat4::from_rotation_z(time * 0.5), // Rotate over time
    color_tint: Vec4::new(1.0, 0.5, 0.5, 1.0), // Red tint
};

graphics.queue.write_buffer(
    &uniform_buffer,
    0,
    bytemuck::cast_slice(&[new_uniforms]),
);
```

## Texture Sampling

### Shader with Texture

```wgsl
// Texture and sampler bindings
@group(0) @binding(0)
var texture: texture_2d<f32>;

@group(0) @binding(1)
var texture_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2f,
    @location(1) uv: vec2f,
}

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4f(input.position, 0.0, 1.0);
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    // Sample texture at UV coordinate
    return textureSample(texture, texture_sampler, input.uv);
}
```

### Create Texture and Sampler

```rust
// Load image (using image crate)
let img = image::open("texture.png")?.to_rgba8();
let dimensions = img.dimensions();

let texture_size = wgpu::Extent3d {
    width: dimensions.0,
    height: dimensions.1,
    depth_or_array_layers: 1,
};

let texture = graphics.device.create_texture(&wgpu::TextureDescriptor {
    label: Some("Texture"),
    size: texture_size,
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format: wgpu::TextureFormat::Rgba8UnormSrgb,
    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    view_formats: &[],
});

// Upload image data
graphics.queue.write_texture(
    wgpu::ImageCopyTexture {
        texture: &texture,
        mip_level: 0,
        origin: wgpu::Origin3d::ZERO,
        aspect: wgpu::TextureAspect::All,
    },
    &img,
    wgpu::ImageDataLayout {
        offset: 0,
        bytes_per_row: Some(4 * dimensions.0),
        rows_per_image: Some(dimensions.1),
    },
    texture_size,
);

let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

// Create sampler
let sampler = graphics.device.create_sampler(&wgpu::SamplerDescriptor {
    label: Some("Texture Sampler"),
    address_mode_u: wgpu::AddressMode::Repeat,
    address_mode_v: wgpu::AddressMode::Repeat,
    address_mode_w: wgpu::AddressMode::Repeat,
    mag_filter: wgpu::FilterMode::Linear,
    min_filter: wgpu::FilterMode::Linear,
    mipmap_filter: wgpu::FilterMode::Linear,
    ..Default::default()
});
```

### Bind Texture and Sampler

```rust
let bind_group_layout = graphics.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    entries: &[
        // Texture (binding = 0)
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        // Sampler (binding = 1)
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ],
    label: Some("Texture Bind Group Layout"),
});

let bind_group = graphics.device.create_bind_group(&wgpu::BindGroupDescriptor {
    layout: &bind_group_layout,
    entries: &[
        wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&texture_view),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(&sampler),
        },
    ],
    label: Some("Texture Bind Group"),
});
```

## Complete Example: Gradient Shader

A shader that creates a gradient based on screen position:

```wgsl
struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) screen_pos: vec2f,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Full-screen triangle positions
    let x = f32(i32(vertex_index) - 1);
    let y = f32(i32(vertex_index & 1u) * 2 - 1);

    output.position = vec4f(x, y, 0.0, 1.0);
    output.screen_pos = vec2f(x, y);

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    // Map screen position to 0..1 range
    let uv = input.screen_pos * 0.5 + 0.5;

    // Create gradient
    let color_top = vec3f(0.2, 0.5, 0.9);    // Blue
    let color_bottom = vec3f(0.9, 0.3, 0.2); // Red

    let color = mix(color_bottom, color_top, uv.y);

    return vec4f(color, 1.0);
}
```

**Usage:**
```rust
// No vertex buffer needed - generates positions in shader
render_pass.set_pipeline(&gradient_pipeline);
render_pass.draw(0..3, 0..1); // 3 vertices for full-screen triangle
```

## Complete Example: Textured Mesh Shader

A complete shader for rendering textured 3D meshes:

```wgsl
// Uniforms
struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var texture: texture_2d<f32>;

@group(1) @binding(1)
var texture_sampler: sampler;

// Vertex input
struct VertexInput {
    @location(0) position: vec3f,
    @location(1) normal: vec3f,
    @location(2) uv: vec2f,
}

// Vertex output
struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) world_position: vec3f,
    @location(1) world_normal: vec3f,
    @location(2) uv: vec2f,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Transform position to world space
    let world_pos = uniforms.model * vec4f(input.position, 1.0);
    output.world_position = world_pos.xyz;

    // Transform to clip space
    output.clip_position = uniforms.view_proj * world_pos;

    // Transform normal to world space (assumes uniform scale)
    output.world_normal = (uniforms.model * vec4f(input.normal, 0.0)).xyz;

    // Pass through UV
    output.uv = input.uv;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    // Sample texture
    let texture_color = textureSample(texture, texture_sampler, input.uv);

    // Simple directional lighting
    let light_dir = normalize(vec3f(1.0, 1.0, 1.0));
    let normal = normalize(input.world_normal);
    let diffuse = max(dot(normal, light_dir), 0.0);

    // Ambient + diffuse
    let ambient = 0.3;
    let lighting = ambient + diffuse * 0.7;

    // Apply lighting to texture
    let final_color = texture_color.rgb * lighting;

    return vec4f(final_color, texture_color.a);
}
```

## Advanced Techniques

### Alpha Blending

Configure blend state for transparency:

```rust
let color_target = ColorTargetState {
    format: surface_format,
    blend: Some(BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    }),
    write_mask: ColorWrites::ALL,
};
```

**Common blend modes:**
- `BlendState::ALPHA_BLENDING`: Standard alpha blending
- `BlendState::PREMULTIPLIED_ALPHA_BLENDING`: Premultiplied alpha
- `BlendState::REPLACE`: No blending (opaque)

### Depth Testing

Enable depth testing for 3D rendering:

```rust
let pipeline = graphics.device.create_render_pipeline(&RenderPipelineDescriptor {
    // ... other fields
    depth_stencil: Some(DepthStencilState {
        format: TextureFormat::Depth32Float,
        depth_write_enabled: true,
        depth_compare: CompareFunction::Less, // Closer objects win
        stencil: StencilState::default(),
        bias: DepthBiasState::default(),
    }),
    // ...
});
```

### Culling

Configure face culling:

```rust
primitive: PrimitiveState {
    topology: PrimitiveTopology::TriangleList,
    front_face: FrontFace::Ccw, // Counter-clockwise front faces
    cull_mode: Some(Face::Back),  // Cull back faces
    ..Default::default()
},
```

### Push Constants (Not in WGPU)

WGPU doesn't support push constants. Use small uniform buffers instead:

```rust
// Small frequent updates: use uniform buffer
let push_constant_buffer = graphics.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    contents: bytemuck::cast_slice(&[object_id, time]),
    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    label: Some("Push Constants"),
});

// Update every draw call
graphics.queue.write_buffer(&push_constant_buffer, 0, bytemuck::cast_slice(&[new_data]));
```

## Shader Compilation and Debugging

### Compilation Errors

WGSL compilation errors are reported at pipeline creation:

```rust
let pipeline = graphics.device.create_render_pipeline(&descriptor);
// If shader has errors, this will panic with error message
```

**Common errors:**

1. **Type mismatch:**
```wgsl
// ERROR: Can't assign vec3f to vec4f
let pos: vec4f = vec3f(1.0, 2.0, 3.0);

// FIX:
let pos: vec4f = vec4f(1.0, 2.0, 3.0, 1.0);
```

2. **Undefined variable:**
```wgsl
// ERROR: 'missing_var' is undefined
let x = missing_var;

// FIX: Define it
let missing_var = 5.0;
let x = missing_var;
```

3. **Wrong entry point:**
```rust
// Shader has @vertex fn vs_main()
// But pipeline says:
entry_point: "vertex_main", // ERROR: Wrong name

// FIX:
entry_point: "vs_main",
```

### Debugging Techniques

**1. Output debug colors:**
```wgsl
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    // Debug: Output UV as color
    return vec4f(input.uv, 0.0, 1.0);

    // Debug: Output normal as color
    // return vec4f(input.normal * 0.5 + 0.5, 1.0);

    // Debug: Output constant color
    // return vec4f(1.0, 0.0, 0.0, 1.0); // Red
}
```

**2. Use validation layers:**
```rust
let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
    backends: wgpu::Backends::all(),
    dx12_shader_compiler: Default::default(),
});

// Enable validation in debug builds
let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
    power_preference: wgpu::PowerPreference::HighPerformance,
    force_fallback_adapter: false,
    compatible_surface: Some(&surface),
}).await.unwrap();
```

**3. RenderDoc integration:**

Capture frames with RenderDoc for detailed GPU debugging.

## Performance Tips

### Minimize Texture Samples

```wgsl
// SLOW: Multiple samples
let color1 = textureSample(tex, samp, uv);
let color2 = textureSample(tex, samp, uv + offset1);
let color3 = textureSample(tex, samp, uv + offset2);

// FASTER: Single sample
let color = textureSample(tex, samp, uv);
```

### Use Appropriate Precision

```wgsl
// Desktop: f32 is fine
var position: vec3<f32>;

// Mobile: Consider f16 for some values (when supported)
// var color: vec4<f16>; // Not universally supported yet
```

### Avoid Branches in Fragment Shaders

```wgsl
// SLOW: Branching
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    if (input.uv.x > 0.5) {
        return vec4f(1.0, 0.0, 0.0, 1.0);
    } else {
        return vec4f(0.0, 0.0, 1.0, 1.0);
    }
}

// FASTER: Branchless
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    let t = step(0.5, input.uv.x);
    return mix(vec4f(0.0, 0.0, 1.0, 1.0), vec4f(1.0, 0.0, 0.0, 1.0), t);
}
```

### Precompute When Possible

```wgsl
// Move calculations to vertex shader when possible
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // GOOD: Compute once per vertex
    output.lighting = compute_lighting(input.position, input.normal);

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    // BAD: Would compute per pixel
    // let lighting = compute_lighting(...);

    // GOOD: Interpolated from vertex shader
    return vec4f(input.lighting, 1.0);
}
```

## Built-in Functions

### Math Functions

```wgsl
// Trigonometry
sin(x), cos(x), tan(x)
asin(x), acos(x), atan(x), atan2(y, x)

// Exponential
pow(x, y), exp(x), log(x), exp2(x), log2(x)
sqrt(x), inverseSqrt(x)

// Common
abs(x), sign(x)
floor(x), ceil(x), round(x), fract(x)
min(a, b), max(a, b), clamp(x, min, max)
mix(a, b, t) // Linear interpolation
step(edge, x), smoothstep(edge0, edge1, x)

// Vector
length(v), distance(a, b), normalize(v)
dot(a, b), cross(a, b)
reflect(v, n), refract(v, n, eta)
```

### Texture Functions

```wgsl
// Sampling
textureSample(tex, sampler, uv) -> vec4f
textureSampleLevel(tex, sampler, uv, level) -> vec4f // Explicit mip level
textureSampleBias(tex, sampler, uv, bias) -> vec4f   // Mip level bias

// Queries
textureDimensions(tex) -> vec2<u32>
textureNumLayers(tex) -> u32
textureNumLevels(tex) -> u32
```

## Next Steps

- **Practice:** Modify the `colored_triangle` example (when added)
- **Learn More:** [Materials and Textures](materials-and-textures.md) for material systems
- **Advanced:** [Compute Shaders](compute-shaders.md) for GPU computation
- **Examples:** `material_system`, `textured_window` for shader usage

## See Also

- [WGSL Specification](https://www.w3.org/TR/WGSL/) - Official language spec
- [WebGPU Shading Language](https://gpuweb.github.io/gpuweb/wgsl/) - Reference
- [Materials and Textures](materials-and-textures.md) - Material system integration
- [Render Passes](render-passes.md) - Using shaders in render passes
- API Reference: [`wgpu::ShaderModule`](https://docs.rs/wgpu/latest/wgpu/struct.ShaderModule.html)
