# astrelis-gpu-wgpu

[wgpu](https://wgpu.rs/) 29 backend for the Astrelis GPU abstraction.

This crate implements the traits defined in [`astrelis-gpu`](../astrelis-gpu)
using wgpu as the GPU layer. It supports all desktop platforms via Vulkan,
Metal, DX12, and OpenGL, and provides `raw-window-handle` integration for
surface creation from any `astrelis-window` backend.

## Quick Start

```rust,ignore
use astrelis_gpu::backend::{GpuBackend, GpuConfig};
use astrelis_gpu::device::GpuDevice;
use astrelis_gpu::surface::{GpuSurface, SurfaceConfiguration};
use astrelis_gpu::types::PresentMode;
use astrelis_gpu_wgpu::WgpuBackend;

// Create the GPU backend (selects adapter, creates device + queue).
let gpu = WgpuBackend::new(&GpuConfig::default())?;
println!("GPU: {}", gpu.device().adapter_info().name);

// Create a surface from any &dyn Window.
let mut surface = gpu.create_surface(window)?;
surface.configure(&SurfaceConfiguration {
    format: surface.preferred_format(),
    width: 800,
    height: 600,
    present_mode: PresentMode::AutoVsync,
    desired_maximum_frame_latency: 2,
});
```

## Examples

Run any example with:

```sh
cargo run -p astrelis-gpu-wgpu --example <name>
```

| Example | Description |
|---------|-------------|
| `clear_color` | Clears the window to a cycling color — simplest GPU example |
| `triangle` | Renders a colored triangle with vertex buffers and a WGSL shader |

## Public API

Only `WgpuBackend` is publicly exported. All other types are accessed
through the `astrelis_gpu` trait interfaces:

| Trait | wgpu type |
|-------|-----------|
| `GpuBackend` | `WgpuBackend` |
| `GpuDevice` | (internal `WgpuDevice`) |
| `GpuQueue` | (internal `WgpuQueue`) |
| `GpuSurface` | (internal `WgpuSurface`) |
| `CommandEncoder` | (internal `WgpuCommandEncoder`) |
| `RenderPass` | (internal `WgpuRenderPass`) |
| `ComputePass` | (internal `WgpuComputePass`) |

## Configuration

`GpuConfig` controls backend initialization:

| Field | Default | Description |
|-------|---------|-------------|
| `power_preference` | `None` | GPU selection hint (low power, high performance) |
| `validation` | `Some(cfg!(debug_assertions))` | Enable validation/debug layers |
| `device_label` | `None` | Debug label for the device |

## License

MIT
