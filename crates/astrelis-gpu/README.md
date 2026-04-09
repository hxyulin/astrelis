# astrelis-gpu

Backend-agnostic GPU abstraction traits and types for the Astrelis engine.

This crate defines the platform-independent GPU abstraction (Layer 2).
It has **zero GPU dependencies** — only `astrelis-core`, `astrelis-window`,
and `raw-window-handle`. Concrete backends like
[`astrelis-gpu-wgpu`](../astrelis-gpu-wgpu) implement the traits.

## Architecture

```
GpuBackend::new(config)          — initialize adapter + device
     │
     ├─ backend.device()         — GpuDevice (resource creation)
     │       ├─ create_buffer()       → BufferId
     │       ├─ create_texture()      → TextureId
     │       ├─ create_render_pipeline() → RenderPipelineId
     │       └─ create_command_encoder() → Encoder
     │
     ├─ backend.queue()          — GpuQueue (submission)
     │       └─ submit(encoders)
     │
     └─ backend.create_surface(window) — GpuSurface (presentation)
             ├─ configure(config)
             └─ acquire() → SurfaceTexture
                     ├─ view() → TextureViewId
                     └─ present()
```

## Key Traits

| Trait | Purpose |
|-------|---------|
| `GpuBackend` | Entry point — adapter selection, device creation, surface creation |
| `GpuDevice` | Resource creation/destruction, buffer writes, command encoder creation |
| `GpuQueue` | Command submission |
| `GpuSurface` | Swap chain management, frame acquisition |
| `CommandEncoder` | GPU command recording |
| `RenderPass` | Render command recording within a pass |
| `ComputePass` | Compute command recording within a pass |

## Resource Handles

GPU resources are identified by lightweight typed handles built on
`astrelis_core::id::Id<T>`. Handles are `Copy + Send + Sync` — the
backend owns the actual GPU objects internally.

| Handle | Resource |
|--------|----------|
| `BufferId` | GPU buffer (vertex, index, uniform, storage) |
| `TextureId` | GPU texture |
| `TextureViewId` | View into a texture |
| `SamplerId` | Texture sampler |
| `ShaderModuleId` | Compiled shader |
| `BindGroupLayoutId` | Bind group layout |
| `BindGroupId` | Bound resource set |
| `PipelineLayoutId` | Pipeline layout |
| `RenderPipelineId` | Render pipeline |
| `ComputePipelineId` | Compute pipeline |

## Usage

```rust,ignore
use astrelis_gpu::backend::{GpuBackend, GpuConfig};
use astrelis_gpu::command::{ColorAttachment, CommandEncoder, RenderPassDescriptor};
use astrelis_gpu::device::GpuDevice;
use astrelis_gpu::queue::GpuQueue;
use astrelis_gpu::surface::{GpuSurface, SurfaceConfiguration, SurfaceTexture};
use astrelis_gpu::types::{LoadOp, PresentMode, StoreOp};
use astrelis_gpu_wgpu::WgpuBackend;

// Initialize GPU
let gpu = WgpuBackend::new(&GpuConfig::default())?;

// Create and configure a surface from a window
let mut surface = gpu.create_surface(window)?;
surface.configure(&SurfaceConfiguration {
    format: surface.preferred_format(),
    width: 800,
    height: 600,
    present_mode: PresentMode::AutoVsync,
    desired_maximum_frame_latency: 2,
});

// Each frame: acquire → record → submit → present
let frame = surface.acquire()?;
let mut encoder = gpu.device().create_command_encoder(Some("frame"));
{
    let _pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("main"),
        color_attachments: &[ColorAttachment {
            view: frame.view(),
            resolve_target: None,
            load_op: LoadOp::Clear(Color::BLACK),
            store_op: StoreOp::Store,
        }],
        depth_stencil_attachment: None,
    });
}
gpu.queue().submit(std::iter::once(encoder));
frame.present();
```

## Descriptor Types

All resource creation uses engine-owned descriptor structs (not backend
re-exports), keeping this crate free of platform dependencies:

| Descriptor | Creates |
|------------|---------|
| `BufferDescriptor` / `BufferInitDescriptor` | GPU buffers |
| `TextureDescriptor` | Textures |
| `TextureViewDescriptor` | Texture views |
| `SamplerDescriptor` | Samplers |
| `ShaderModuleDescriptor` | Shader modules |
| `BindGroupLayoutDescriptor` / `BindGroupDescriptor` | Bind groups |
| `PipelineLayoutDescriptor` | Pipeline layouts |
| `RenderPipelineDescriptor` | Render pipelines |
| `ComputePipelineDescriptor` | Compute pipelines |

## License

MIT
