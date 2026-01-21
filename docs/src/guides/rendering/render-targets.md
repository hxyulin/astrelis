# Render Targets

This guide explains how to use render targets in Astrelis for rendering to textures, creating post-processing effects, and building advanced rendering pipelines.

## Overview

A **render target** defines where rendering output goes:

- **Surface**: Render directly to the window (most common)
- **Framebuffer**: Render to a texture (render-to-texture)

**Use Cases:**
- **Surface**: Final output to screen
- **Framebuffer**: Post-processing, mirrors, UI backgrounds, shadow maps, deferred rendering

**Comparison to Unity:** Similar to Unity's `RenderTexture` system, but with more explicit control over attachment formats and operations.

## RenderTarget Enum

```rust
pub enum RenderTarget {
    /// Render to window surface (screen)
    Surface,

    /// Render to framebuffer (texture)
    Framebuffer(FramebufferId),
}
```

### Rendering to Surface

The default and most common case:

```rust
use astrelis_render::{RenderTarget, Color};

let mut frame = renderable_window.begin_drawing();

frame.clear_and_render(
    RenderTarget::Surface,  // Render to screen
    Color::BLACK,
    |pass| {
        scene_renderer.render(pass.descriptor());
    },
);

frame.finish();
```

**Result:** Rendered content appears in the window.

### Rendering to Framebuffer

For render-to-texture workflows:

```rust
use astrelis_render::{RenderTarget, FramebufferId};

let framebuffer_id = framebuffer_manager.create_framebuffer(config)?;

let mut frame = renderable_window.begin_drawing();

frame.clear_and_render(
    RenderTarget::Framebuffer(framebuffer_id),  // Render to texture
    Color::BLACK,
    |pass| {
        scene_renderer.render(pass.descriptor());
    },
);

frame.finish();
```

**Result:** Rendered content is stored in the framebuffer's texture, which can be used later.

## Creating Framebuffers

### Basic Framebuffer

Create a framebuffer with a single color attachment:

```rust
use astrelis_render::{Framebuffer, FramebufferConfig};

let config = FramebufferConfig {
    width: 1920,
    height: 1080,
    format: wgpu::TextureFormat::Rgba8UnormSrgb,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    sample_count: 1, // No MSAA
    label: Some("Scene Framebuffer"),
};

let framebuffer = Framebuffer::new(&graphics.device, config);
```

**Key Parameters:**
- `width` / `height`: Resolution of the render target
- `format`: Color format (Rgba8UnormSrgb for typical color)
- `usage`: How the texture will be used
  - `RENDER_ATTACHMENT`: Can be rendered to
  - `TEXTURE_BINDING`: Can be used as a texture in shaders
- `sample_count`: MSAA samples (1 = no MSAA, 4 = 4x MSAA)

### Framebuffer with Depth

For 3D rendering with depth testing:

```rust
let config = FramebufferConfig {
    width: 1920,
    height: 1080,
    format: wgpu::TextureFormat::Rgba8UnormSrgb,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    sample_count: 1,
    depth_format: Some(wgpu::TextureFormat::Depth32Float),
    label: Some("3D Scene Framebuffer"),
};

let framebuffer = Framebuffer::new(&graphics.device, config);
```

**Depth Formats:**
- `Depth32Float`: High precision depth (recommended)
- `Depth24Plus`: 24-bit depth (compatible)
- `Depth24PlusStencil8`: Depth + stencil

### MSAA Framebuffer

Enable multi-sample anti-aliasing:

```rust
let config = FramebufferConfig {
    width: 1920,
    height: 1080,
    format: wgpu::TextureFormat::Rgba8UnormSrgb,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    sample_count: 4, // 4x MSAA
    label: Some("MSAA Framebuffer"),
};

let framebuffer = Framebuffer::new(&graphics.device, config);
```

**Sample Counts:** 1 (no MSAA), 2, 4, 8 (depends on GPU support)

**Note:** MSAA framebuffers automatically resolve to a non-MSAA texture for use in shaders.

### Multiple Render Targets (MRT)

Render to multiple textures simultaneously:

```rust
let config = FramebufferConfig {
    width: 1920,
    height: 1080,
    attachments: vec![
        AttachmentConfig {
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            label: Some("Color"),
        },
        AttachmentConfig {
            format: wgpu::TextureFormat::Rgba16Float,
            label: Some("Normal"),
        },
        AttachmentConfig {
            format: wgpu::TextureFormat::Rg16Float,
            label: Some("Velocity"),
        },
    ],
    depth_format: Some(wgpu::TextureFormat::Depth32Float),
    sample_count: 1,
    label: Some("G-Buffer"),
};

let framebuffer = Framebuffer::new_mrt(&graphics.device, config);
```

**Use Case:** Deferred rendering (G-buffer), multi-pass effects

## Using Rendered Textures

### Accessing Framebuffer Textures

After rendering to a framebuffer, access its textures:

```rust
// Get color attachment
let color_texture = framebuffer.color_texture();
let color_view = framebuffer.color_view();

// Get depth attachment (if exists)
if let Some(depth_texture) = framebuffer.depth_texture() {
    let depth_view = framebuffer.depth_view().unwrap();
}
```

### Using Textures in Shaders

Bind framebuffer texture to a shader:

```rust
// Create bind group with framebuffer texture
let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
    label: Some("Scene Texture Bind Group"),
    layout: &bind_group_layout,
    entries: &[
        wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(framebuffer.color_view()),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(&sampler),
        },
    ],
});

// Use in render pass
render_pass.set_bind_group(0, &bind_group, &[]);
render_pass.draw(0..6, 0..1); // Full-screen quad
```

### Displaying Framebuffer in UI

Show a rendered texture in the UI:

```rust
use astrelis_ui::UiSystem;

// Render scene to framebuffer
frame.clear_and_render(
    RenderTarget::Framebuffer(scene_fb),
    Color::BLACK,
    |pass| {
        scene_renderer.render(pass.descriptor());
    },
);

// Display framebuffer texture in UI
ui.build(|root| {
    root.image(framebuffer.color_texture())
        .width(Length::px(640.0))
        .height(Length::px(480.0))
        .build();
});

// Render UI to surface
frame.clear_and_render(
    RenderTarget::Surface,
    Color::DARK_GRAY,
    |pass| {
        ui.render(pass.descriptor());
    },
);
```

**Use Case:** In-game cameras, minimaps, render previews

## Render-to-Texture Workflow

### Basic Post-Processing

Apply a full-screen effect:

```rust
let mut frame = renderable_window.begin_drawing();

// Pass 1: Render scene to framebuffer
frame.clear_and_render(
    RenderTarget::Framebuffer(scene_fb),
    Color::BLACK,
    |pass| {
        scene_renderer.render(pass.descriptor());
    },
);

// Pass 2: Apply post-process and render to surface
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        post_process.render(
            pass.descriptor(),
            framebuffer.color_texture(),
        );
    },
);

frame.finish();
```

**Effect Examples:** Blur, bloom, color grading, vignette, film grain

### Multi-Pass Post-Processing

Chain multiple effects:

```rust
let mut frame = renderable_window.begin_drawing();

// Pass 1: Render scene
frame.clear_and_render(
    RenderTarget::Framebuffer(scene_fb),
    Color::BLACK,
    |pass| {
        scene_renderer.render(pass.descriptor());
    },
);

// Pass 2: Horizontal blur
frame.clear_and_render(
    RenderTarget::Framebuffer(blur_h_fb),
    Color::BLACK,
    |pass| {
        blur.render_horizontal(pass.descriptor(), scene_fb.color_texture());
    },
);

// Pass 3: Vertical blur
frame.clear_and_render(
    RenderTarget::Framebuffer(blur_v_fb),
    Color::BLACK,
    |pass| {
        blur.render_vertical(pass.descriptor(), blur_h_fb.color_texture());
    },
);

// Pass 4: Composite to surface
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        composite.render(
            pass.descriptor(),
            scene_fb.color_texture(),
            blur_v_fb.color_texture(),
        );
    },
);

frame.finish();
```

**Performance:** Use half-resolution framebuffers for blur passes to save bandwidth.

### Ping-Pong Rendering

Alternate between two framebuffers:

```rust
let mut source_fb = framebuffer_a;
let mut dest_fb = framebuffer_b;

for _ in 0..iterations {
    frame.clear_and_render(
        RenderTarget::Framebuffer(dest_fb),
        Color::BLACK,
        |pass| {
            effect.render(pass.descriptor(), source_fb.color_texture());
        },
    );

    // Swap
    std::mem::swap(&mut source_fb, &mut dest_fb);
}

// Final output
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        blit.render(pass.descriptor(), source_fb.color_texture());
    },
);
```

**Use Case:** Iterative effects (blur, diffusion, fluid simulation)

## Advanced Techniques

### Shadow Mapping

Render depth from light's perspective:

```rust
// Create shadow map framebuffer
let shadow_config = FramebufferConfig {
    width: 2048,
    height: 2048,
    format: wgpu::TextureFormat::R32Float, // Depth as color
    depth_format: Some(wgpu::TextureFormat::Depth32Float),
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    sample_count: 1,
    label: Some("Shadow Map"),
};

let shadow_fb = Framebuffer::new(&graphics.device, shadow_config);

// Render shadow map
frame.clear_and_render(
    RenderTarget::Framebuffer(shadow_fb),
    Color::WHITE, // Far plane
    |pass| {
        scene_renderer.render_from_light(pass.descriptor(), light_view_proj);
    },
);

// Render scene with shadows
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        scene_renderer.render_with_shadows(
            pass.descriptor(),
            shadow_fb.color_texture(),
        );
    },
);
```

### Deferred Rendering

Render geometry properties to multiple textures:

```rust
// Create G-buffer (Multiple Render Targets)
let gbuffer_config = FramebufferConfig {
    width: 1920,
    height: 1080,
    attachments: vec![
        AttachmentConfig {
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            label: Some("Albedo"),
        },
        AttachmentConfig {
            format: wgpu::TextureFormat::Rgba16Float,
            label: Some("Normal"),
        },
        AttachmentConfig {
            format: wgpu::TextureFormat::Rgba8Unorm,
            label: Some("Material"), // Metallic, roughness, etc.
        },
    ],
    depth_format: Some(wgpu::TextureFormat::Depth32Float),
    sample_count: 1,
    label: Some("G-Buffer"),
};

let gbuffer = Framebuffer::new_mrt(&graphics.device, gbuffer_config);

// Geometry pass: Write to G-buffer
frame.clear_and_render(
    RenderTarget::Framebuffer(gbuffer),
    Color::BLACK,
    |pass| {
        scene_renderer.render_geometry(pass.descriptor());
    },
);

// Lighting pass: Read from G-buffer, render to surface
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        lighting_renderer.render(
            pass.descriptor(),
            gbuffer.attachment_texture(0), // Albedo
            gbuffer.attachment_texture(1), // Normal
            gbuffer.attachment_texture(2), // Material
            gbuffer.depth_texture().unwrap(),
        );
    },
);
```

**Benefit:** Efficient multiple-light rendering

### Reflection Probes

Capture environment for reflections:

```rust
// Create cube map framebuffer (6 faces)
let probe_config = FramebufferConfig {
    width: 256,
    height: 256,
    format: wgpu::TextureFormat::Rgba16Float,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    sample_count: 1,
    label: Some("Reflection Probe"),
};

// Render each cube face
for face in 0..6 {
    let view_proj = compute_cube_face_matrix(probe_position, face);

    frame.clear_and_render(
        RenderTarget::Framebuffer(probe_fb[face]),
        Color::BLACK,
        |pass| {
            scene_renderer.render_from_position(pass.descriptor(), view_proj);
        },
    );
}

// Use cube map in scene rendering
scene_renderer.set_reflection_probe(probe_cubemap);
```

### Screen-Space Effects

Effects that use screen as input:

```rust
// Render scene normally
frame.clear_and_render(
    RenderTarget::Framebuffer(scene_fb),
    Color::BLACK,
    |pass| {
        scene_renderer.render(pass.descriptor());
    },
);

// Screen-space ambient occlusion (SSAO)
frame.clear_and_render(
    RenderTarget::Framebuffer(ssao_fb),
    Color::WHITE,
    |pass| {
        ssao_renderer.render(
            pass.descriptor(),
            scene_fb.depth_texture().unwrap(),
        );
    },
);

// Composite scene + SSAO
frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        composite.render(
            pass.descriptor(),
            scene_fb.color_texture(),
            ssao_fb.color_texture(),
        );
    },
);
```

## Texture Formats

### Common Color Formats

| Format | Use Case | Precision | Size |
|--------|----------|-----------|------|
| `Rgba8UnormSrgb` | Standard color (LDR) | 8-bit/channel | 4 bytes |
| `Rgba8Unorm` | Linear color (LDR) | 8-bit/channel | 4 bytes |
| `Rgba16Float` | HDR color | 16-bit float | 8 bytes |
| `Rgba32Float` | High precision HDR | 32-bit float | 16 bytes |
| `Bgra8UnormSrgb` | Windows default | 8-bit/channel | 4 bytes |

**Recommendation:**
- LDR final output: `Rgba8UnormSrgb`
- HDR intermediate: `Rgba16Float`
- Normals/data: `Rgba16Float` or `Rgba8Unorm`

### Depth Formats

| Format | Precision | Stencil | Size |
|--------|-----------|---------|------|
| `Depth32Float` | Best | No | 4 bytes |
| `Depth24Plus` | Good | No | 3-4 bytes |
| `Depth24PlusStencil8` | Good | Yes | 4 bytes |

**Recommendation:** Use `Depth32Float` unless you need stencil.

### Special Formats

```rust
// Single-channel (grayscale)
wgpu::TextureFormat::R8Unorm // 8-bit
wgpu::TextureFormat::R16Float // 16-bit float
wgpu::TextureFormat::R32Float // 32-bit float

// Two-channel (RG)
wgpu::TextureFormat::Rg8Unorm
wgpu::TextureFormat::Rg16Float

// Integer formats
wgpu::TextureFormat::Rgba8Uint // Unsigned int
wgpu::TextureFormat::Rgba8Sint // Signed int
```

**Use Cases:**
- `R32Float`: Shadow maps, height maps
- `Rg16Float`: Velocity buffers, flow maps
- `Rgba8Uint`: Object IDs, entity picking

## Framebuffer Management

### Resizing Framebuffers

Handle window resize:

```rust
struct MyApp {
    scene_fb: Framebuffer,
    window_size: (u32, u32),
}

impl App for MyApp {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        events.dispatch(|event| {
            if let Event::WindowResized(size) = event {
                // Recreate framebuffer at new size
                let config = FramebufferConfig {
                    width: size.width,
                    height: size.height,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                         | wgpu::TextureUsages::TEXTURE_BINDING,
                    sample_count: 1,
                    label: Some("Scene Framebuffer"),
                };

                self.scene_fb = Framebuffer::new(&self.graphics.device, config);
                self.window_size = (size.width, size.height);

                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        // ... rendering
    }
}
```

### Framebuffer Pool

Reuse framebuffers to avoid allocations:

```rust
struct FramebufferPool {
    framebuffers: Vec<Framebuffer>,
    available: Vec<usize>,
}

impl FramebufferPool {
    fn acquire(&mut self, width: u32, height: u32, format: TextureFormat) -> usize {
        // Find matching available framebuffer
        if let Some(index) = self.available.pop() {
            return index;
        }

        // Create new framebuffer
        let fb = Framebuffer::new(&device, config);
        self.framebuffers.push(fb);
        self.framebuffers.len() - 1
    }

    fn release(&mut self, index: usize) {
        self.available.push(index);
    }
}
```

**Use Case:** Temporary framebuffers in post-processing chains

## Performance Considerations

### Resolution vs Performance

| Resolution | Pixels | Relative Cost |
|------------|--------|---------------|
| 1920x1080 | 2.1M | 1x |
| 1280x720 | 0.9M | 0.44x |
| 960x540 | 0.5M | 0.25x |
| 640x360 | 0.2M | 0.11x |

**Optimization:** Use lower resolution for expensive effects (bloom, blur)

```rust
// Half-resolution blur (4x faster)
let blur_config = FramebufferConfig {
    width: window_width / 2,
    height: window_height / 2,
    // ... other config
};
```

### Format Selection

```rust
// Expensive: 16 bytes per pixel
wgpu::TextureFormat::Rgba32Float

// Cheap: 4 bytes per pixel (4x less memory bandwidth)
wgpu::TextureFormat::Rgba8Unorm

// Middle ground: 8 bytes per pixel
wgpu::TextureFormat::Rgba16Float
```

**Guideline:** Use the smallest format that doesn't introduce artifacts.

### MSAA Cost

| Sample Count | Memory | Bandwidth | Relative Cost |
|--------------|--------|-----------|---------------|
| 1 (no MSAA) | 1x | 1x | 1x |
| 2x MSAA | 2x | 1.5x | 1.5x |
| 4x MSAA | 4x | 2x | 2x |
| 8x MSAA | 8x | 3x | 3x |

**Recommendation:** Use 4x MSAA for quality, 1x for performance.

## Troubleshooting

### Black Screen After Framebuffer Render

**Common causes:**
1. Forgot to use framebuffer texture in second pass
2. Incorrect texture format (sRGB vs linear mismatch)
3. Viewport not set correctly

**Debug:**
```rust
// Verify texture is valid
println!("Texture size: {:?}", framebuffer.color_texture().size());
```

### Out of Memory

**Cause:** Too many large framebuffers.

**Fix:** Use smaller resolutions or fewer framebuffers
```rust
// Instead of 1920x1080 for every effect:
let blur_fb = Framebuffer::new(&device, FramebufferConfig {
    width: 960,  // Half resolution
    height: 540,
    // ...
});
```

### Framebuffer Appears Stretched

**Cause:** Aspect ratio mismatch between framebuffer and display.

**Fix:** Match aspect ratios or adjust UV coordinates.

## Next Steps

- **Practice:** Try the `render_to_texture` example (when added)
- **Learn More:** [Render Passes](render-passes.md) for multi-pass rendering
- **Advanced:** [Custom Shaders](custom-shaders.md) for post-processing effects
- **Examples:** `image_blitting`, `sprite_sheet` for texture usage

## See Also

- [Render Passes](render-passes.md) - Organizing render commands
- [Custom Shaders](custom-shaders.md) - Writing shader effects
- [Materials and Textures](materials-and-textures.md) - Texture management
- API Reference: [`Framebuffer`](../../api/astrelis-render/struct.Framebuffer.html)
- API Reference: [`RenderTarget`](../../api/astrelis-render/enum.RenderTarget.html)
