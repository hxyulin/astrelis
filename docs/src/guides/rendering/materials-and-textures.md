# Materials and Textures

This guide covers the material system in Astrelis and how to work with textures. Learn to create materials, configure textures, and optimize texture usage for performance.

## Overview

**Materials** define how objects appear by combining:
- **Shaders**: Programs that run on GPU
- **Textures**: Images applied to surfaces
- **Parameters**: Values controlling appearance (color, roughness, etc.)

**Comparison to Unity:** Similar to Unity's Material/Shader system, but with more explicit control over GPU resources.

## Material System Architecture

### Material Structure

```rust
pub struct Material {
    /// Render pipeline (shader + configuration)
    pub pipeline: wgpu::RenderPipeline,

    /// Bind groups (uniforms, textures, samplers)
    pub bind_groups: Vec<wgpu::BindGroup>,

    /// Material parameters
    pub parameters: MaterialParameters,
}
```

**Key Concept:** A material is a pipeline + bind groups + parameters.

### Material Parameters

```rust
use glam::{Vec2, Vec3, Vec4, Mat4};

pub struct MaterialParameters {
    /// Color tint
    pub color: Vec4,

    /// Metallic factor (0.0 = dielectric, 1.0 = metal)
    pub metallic: f32,

    /// Roughness factor (0.0 = smooth, 1.0 = rough)
    pub roughness: f32,

    /// Emissive color
    pub emissive: Vec3,

    /// UV offset
    pub uv_offset: Vec2,

    /// UV scale
    pub uv_scale: Vec2,
}

impl Default for MaterialParameters {
    fn default() -> Self {
        Self {
            color: Vec4::ONE,
            metallic: 0.0,
            roughness: 0.5,
            emissive: Vec3::ZERO,
            uv_offset: Vec2::ZERO,
            uv_scale: Vec2::ONE,
        }
    }
}
```

### Creating a Material

```rust
use astrelis_render::{Material, MaterialParameters};

// Load shader
let shader_source = include_str!("shaders/pbr_material.wgsl");
let shader_module = graphics.device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("PBR Material Shader"),
    source: wgpu::ShaderSource::Wgsl(shader_source.into()),
});

// Create pipeline
let pipeline = graphics.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    label: Some("PBR Pipeline"),
    layout: Some(&pipeline_layout),
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
    primitive: PrimitiveState::default(),
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

// Create material
let material = Material {
    pipeline,
    bind_groups: vec![material_bind_group],
    parameters: MaterialParameters::default(),
};
```

## Working with Textures

### Loading Textures from Files

```rust
use image::GenericImageView;

fn load_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &str,
) -> Result<wgpu::Texture, Box<dyn std::error::Error>> {
    // Load image
    let img = image::open(path)?;
    let rgba = img.to_rgba8();
    let dimensions = img.dimensions();

    // Create texture
    let size = wgpu::Extent3d {
        width: dimensions.0,
        height: dimensions.1,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(path),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Upload image data
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * dimensions.0),
            rows_per_image: Some(dimensions.1),
        },
        size,
    );

    Ok(texture)
}

// Usage
let albedo_texture = load_texture(&graphics.device, &graphics.queue, "textures/albedo.png")?;
```

### Creating Textures from Memory

```rust
fn create_texture_from_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    data: &[u8],
) -> wgpu::Texture {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Texture From Bytes"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );

    texture
}
```

### Procedural Textures

Generate textures algorithmically:

```rust
fn create_checker_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: u32,
    checker_size: u32,
) -> wgpu::Texture {
    let mut data = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let checker_x = (x / checker_size) % 2;
            let checker_y = (y / checker_size) % 2;
            let is_white = (checker_x + checker_y) % 2 == 0;

            let color = if is_white { 255 } else { 0 };

            let index = ((y * size + x) * 4) as usize;
            data[index] = color;     // R
            data[index + 1] = color; // G
            data[index + 2] = color; // B
            data[index + 3] = 255;   // A
        }
    }

    create_texture_from_bytes(device, queue, size, size, &data)
}

// Usage
let checker_texture = create_checker_texture(&graphics.device, &graphics.queue, 512, 64);
```

## Texture Formats

### Common Formats

| Format | Description | Use Case | Size (RGBA) |
|--------|-------------|----------|-------------|
| `Rgba8UnormSrgb` | 8-bit sRGB | Albedo/color textures | 4 bytes |
| `Rgba8Unorm` | 8-bit linear | Normal maps, masks | 4 bytes |
| `Rgba16Float` | 16-bit float HDR | HDR textures | 8 bytes |
| `Rgba32Float` | 32-bit float | High-precision data | 16 bytes |
| `R8Unorm` | Single channel 8-bit | Masks, height maps | 1 byte |
| `Rg8Unorm` | Two channel 8-bit | Normal maps (RG only) | 2 bytes |
| `Bc1RgbaUnorm` | BC1/DXT1 compression | Opaque textures | 0.5 bytes |
| `Bc3RgbaUnorm` | BC3/DXT5 compression | Transparent textures | 1 byte |

### Choosing the Right Format

**Albedo/Diffuse:**
```rust
format: wgpu::TextureFormat::Rgba8UnormSrgb, // sRGB for color
```

**Normal Maps:**
```rust
format: wgpu::TextureFormat::Rgba8Unorm, // Linear, not sRGB
```

**Metallic/Roughness:**
```rust
format: wgpu::TextureFormat::Rg8Unorm, // Only need 2 channels
```

**HDR Environment Maps:**
```rust
format: wgpu::TextureFormat::Rgba16Float, // HDR
```

### Compressed Textures

Use compression to save memory:

```rust
// Desktop: BC compression
format: wgpu::TextureFormat::Bc1RgbaUnorm, // DXT1
format: wgpu::TextureFormat::Bc3RgbaUnorm, // DXT5

// Mobile: ASTC compression (if supported)
format: wgpu::TextureFormat::Astc {
    block: AstcBlock::B4x4,
    channel: AstcChannel::UnormSrgb,
}
```

**Note:** Compressed textures require pre-processing (use tools like `crunch` or `compressonator`).

## Texture Samplers

### Creating Samplers

```rust
let sampler = graphics.device.create_sampler(&wgpu::SamplerDescriptor {
    label: Some("Material Sampler"),
    address_mode_u: wgpu::AddressMode::Repeat,
    address_mode_v: wgpu::AddressMode::Repeat,
    address_mode_w: wgpu::AddressMode::Repeat,
    mag_filter: wgpu::FilterMode::Linear,
    min_filter: wgpu::FilterMode::Linear,
    mipmap_filter: wgpu::FilterMode::Linear,
    lod_min_clamp: 0.0,
    lod_max_clamp: 100.0,
    compare: None,
    anisotropy_clamp: 16, // Anisotropic filtering
    border_color: None,
});
```

### Address Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `Repeat` | Tile texture | Repeating patterns |
| `ClampToEdge` | Clamp to edge pixels | UI, single images |
| `MirrorRepeat` | Mirror on repeat | Seamless tiling |
| `ClampToBorder` | Use border color | Shadows, decals |

**Example:**
```rust
// Repeating brick texture
address_mode_u: AddressMode::Repeat,
address_mode_v: AddressMode::Repeat,

// UI sprite (no tiling)
address_mode_u: AddressMode::ClampToEdge,
address_mode_v: AddressMode::ClampToEdge,
```

### Filtering Modes

| Mode | Description | Quality | Performance |
|------|-------------|---------|-------------|
| `Nearest` | No filtering | Pixelated | Fastest |
| `Linear` | Bilinear filtering | Smooth | Fast |
| `Anisotropic` | Anisotropic filtering | Best | Slower |

**Configuration:**
```rust
// Pixelated (retro style)
mag_filter: FilterMode::Nearest,
min_filter: FilterMode::Nearest,
mipmap_filter: FilterMode::Nearest,

// Smooth (default)
mag_filter: FilterMode::Linear,
min_filter: FilterMode::Linear,
mipmap_filter: FilterMode::Linear,
anisotropy_clamp: 16, // Anisotropic filtering
```

### Mipmaps

Generate mip levels for better quality and performance:

```rust
// Calculate mip level count
let mip_level_count = (texture_width.max(texture_height) as f32).log2().floor() as u32 + 1;

let texture = device.create_texture(&wgpu::TextureDescriptor {
    label: Some("Texture with Mipmaps"),
    size: wgpu::Extent3d {
        width: texture_width,
        height: texture_height,
        depth_or_array_layers: 1,
    },
    mip_level_count, // Multiple mip levels
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format: wgpu::TextureFormat::Rgba8UnormSrgb,
    usage: wgpu::TextureUsages::TEXTURE_BINDING
         | wgpu::TextureUsages::COPY_DST
         | wgpu::TextureUsages::RENDER_ATTACHMENT, // For mip generation
    view_formats: &[],
});

// Generate mipmaps (manual or using library)
generate_mipmaps(&device, &queue, &texture);
```

**Benefit:** Better performance and quality at distance.

## Material Binding

### Setting Up Bind Groups

```rust
// Create bind group layout
let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    label: Some("Material Bind Group Layout"),
    entries: &[
        // Uniform buffer (material parameters)
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        // Albedo texture
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        // Sampler
        wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ],
});

// Create bind group
let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
    label: Some("Material Bind Group"),
    layout: &bind_group_layout,
    entries: &[
        wgpu::BindGroupEntry {
            binding: 0,
            resource: parameter_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::TextureView(&albedo_view),
        },
        wgpu::BindGroupEntry {
            binding: 2,
            resource: wgpu::BindingResource::Sampler(&sampler),
        },
    ],
});
```

### Using Materials in Shaders

```wgsl
// Material parameters
struct MaterialParams {
    color: vec4f,
    metallic: f32,
    roughness: f32,
    emissive: vec3f,
    uv_offset: vec2f,
    uv_scale: vec2f,
}

@group(1) @binding(0)
var<uniform> material: MaterialParams;

@group(1) @binding(1)
var albedo_texture: texture_2d<f32>;

@group(1) @binding(2)
var material_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    // Apply UV transform
    let uv = input.uv * material.uv_scale + material.uv_offset;

    // Sample albedo
    let albedo = textureSample(albedo_texture, material_sampler, uv);

    // Apply color tint
    let final_color = albedo * material.color;

    return final_color;
}
```

## Material Instancing

### Sharing Materials

Multiple objects can share the same material:

```rust
struct MeshRenderer {
    mesh: Mesh,
    material: Arc<Material>, // Shared reference
}

// Create material once
let material = Arc::new(create_pbr_material(&graphics));

// Share across multiple objects
let sphere = MeshRenderer {
    mesh: create_sphere_mesh(),
    material: material.clone(),
};

let cube = MeshRenderer {
    mesh: create_cube_mesh(),
    material: material.clone(),
};
```

**Benefit:** Reduces GPU memory and binding overhead.

### Per-Object Parameters

Use dynamic offsets or separate buffers for per-object data:

```rust
// Per-object data (transform, etc.)
struct ObjectData {
    model_matrix: Mat4,
    normal_matrix: Mat4,
}

// Use separate bind group (group 0) for per-object data
@group(0) @binding(0)
var<uniform> object: ObjectData;

// Material bind group (group 1)
@group(1) @binding(0)
var<uniform> material: MaterialParams;
```

**Rendering:**
```rust
for object in objects {
    render_pass.set_bind_group(0, &object.object_bind_group, &[]);
    render_pass.set_bind_group(1, &material.bind_group, &[]);
    render_pass.draw_indexed(0..object.index_count, 0, 0..1);
}
```

## Texture Atlases

### Creating Sprite Sheets

Combine multiple textures into one atlas:

```rust
struct SpriteAtlas {
    texture: wgpu::Texture,
    sprite_uvs: Vec<UvRect>,
}

struct UvRect {
    min: Vec2, // (u_min, v_min)
    max: Vec2, // (u_max, v_max)
}

impl SpriteAtlas {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sprites: &[&str],
        atlas_size: u32,
    ) -> Self {
        // Pack sprites into atlas (simplified)
        let mut atlas_data = vec![0u8; (atlas_size * atlas_size * 4) as usize];
        let mut sprite_uvs = Vec::new();

        let sprites_per_row = (sprites.len() as f32).sqrt().ceil() as u32;
        let sprite_size = atlas_size / sprites_per_row;

        for (i, sprite_path) in sprites.iter().enumerate() {
            let img = image::open(sprite_path).unwrap().to_rgba8();

            let x = (i as u32 % sprites_per_row) * sprite_size;
            let y = (i as u32 / sprites_per_row) * sprite_size;

            // Copy sprite to atlas (simplified)
            // ... copy pixel data ...

            // Calculate UVs
            let u_min = x as f32 / atlas_size as f32;
            let v_min = y as f32 / atlas_size as f32;
            let u_max = (x + sprite_size) as f32 / atlas_size as f32;
            let v_max = (y + sprite_size) as f32 / atlas_size as f32;

            sprite_uvs.push(UvRect {
                min: Vec2::new(u_min, v_min),
                max: Vec2::new(u_max, v_max),
            });
        }

        let texture = create_texture_from_bytes(device, queue, atlas_size, atlas_size, &atlas_data);

        Self { texture, sprite_uvs }
    }

    fn get_sprite_uv(&self, sprite_index: usize) -> UvRect {
        self.sprite_uvs[sprite_index]
    }
}
```

**Usage:**
```rust
// Create atlas with 16 sprites
let atlas = SpriteAtlas::new(
    &graphics.device,
    &graphics.queue,
    &["sprites/player.png", "sprites/enemy.png", /*...*/],
    1024,
);

// Render sprite #3
let uv_rect = atlas.get_sprite_uv(3);
draw_sprite(&atlas.texture, uv_rect);
```

**Benefit:** Reduces draw calls and texture switching.

## Performance Optimization

### Texture Memory Budget

| Resolution | RGBA8 | RGBA16F | Mipmaps | Total |
|------------|-------|---------|---------|-------|
| 1024x1024 | 4 MB | 8 MB | +33% | 5.3 MB / 10.6 MB |
| 2048x2048 | 16 MB | 32 MB | +33% | 21 MB / 42 MB |
| 4096x4096 | 64 MB | 128 MB | +33% | 85 MB / 170 MB |

**Optimization strategies:**
- Use compressed formats (BC1/BC3: 8x smaller)
- Limit texture resolutions (rarely need 4K)
- Use texture atlases
- Stream textures on demand

### Sampler Reuse

Create samplers once and reuse:

```rust
struct SamplerCache {
    repeat_linear: wgpu::Sampler,
    clamp_nearest: wgpu::Sampler,
    repeat_nearest: wgpu::Sampler,
}

impl SamplerCache {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            repeat_linear: device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: AddressMode::Repeat,
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                ..Default::default()
            }),
            // ... other samplers
        }
    }
}

// Use cached samplers
bind_group_entry.resource = BindingResource::Sampler(&sampler_cache.repeat_linear);
```

**Benefit:** Reduces sampler object allocations.

### Batch by Material

Group draw calls by material:

```rust
// Sort objects by material
objects.sort_by_key(|obj| obj.material_id);

// Render in batches
let mut current_material = None;
for object in objects {
    if current_material != Some(object.material_id) {
        render_pass.set_bind_group(1, &object.material.bind_group, &[]);
        current_material = Some(object.material_id);
    }

    render_pass.set_bind_group(0, &object.transform_bind_group, &[]);
    render_pass.draw_indexed(0..object.index_count, 0, 0..1);
}
```

**Benefit:** Reduces bind group switches (expensive GPU operation).

## Comparison to Unity

| Unity | Astrelis | Notes |
|-------|----------|-------|
| `Material` | `Material` | Similar concept |
| `Shader` | `wgpu::ShaderModule` | Explicit shader creation |
| `Texture2D` | `wgpu::Texture` | More explicit control |
| `Material.SetTexture()` | Bind group | Bind group updates |
| `Material.SetFloat()` | Uniform buffer | Update uniform buffer |
| Material instancing | `Arc<Material>` | Explicit sharing |
| Shader properties | Uniform struct | WGSL struct |

## Troubleshooting

### Black Textures

**Cause:** Texture not uploaded or incorrect format.

**Fix:** Verify texture data is uploaded:
```rust
println!("Texture size: {:?}", texture.size());
```

### Stretched Textures

**Cause:** Incorrect UV coordinates or aspect ratio.

**Fix:** Check UV range (should be 0..1):
```rust
// Debug UV in shader
return vec4f(input.uv, 0.0, 1.0);
```

### Texture Appears Dark

**Cause:** sRGB/linear mismatch.

**Fix:** Use correct format:
```rust
// Albedo: sRGB
format: TextureFormat::Rgba8UnormSrgb

// Normal map: Linear
format: TextureFormat::Rgba8Unorm
```

## Next Steps

- **Practice:** Try the `material_system` example
- **Learn More:** [Custom Shaders](custom-shaders.md) for shader details
- **Advanced:** [GPU Instancing](gpu-instancing.md) for rendering many objects
- **Examples:** `textured_window`, `sprite_sheet` for texture usage

## See Also

- [Custom Shaders](custom-shaders.md) - Writing shaders
- [Render Targets](render-targets.md) - Rendering to textures
- [GPU Instancing](gpu-instancing.md) - Efficient rendering
- API Reference: [`wgpu::Texture`](https://docs.rs/wgpu/latest/wgpu/struct.Texture.html)
- API Reference: [`wgpu::Sampler`](https://docs.rs/wgpu/latest/wgpu/struct.Sampler.html)
