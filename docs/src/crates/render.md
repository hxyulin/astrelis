# astrelis-render

The `astrelis-render` crate provides a modular, low-level rendering framework built on top of WGPU. It handles graphics context management, window surface integration, and resource creation.

## Features

- **GraphicsContext**: Manages WGPU instance, adapter, device, and queue.
- **WindowContext**: Manages window surface and swapchain.
- **Renderer**: Extensible trait and base implementation for resource management.
- **Frame**: RAII-based frame lifecycle management.
- **WGPU**: Re-exports `wgpu` types for convenience.

## Usage

```rust
use astrelis_render::{GraphicsContext, WindowContext, Renderer};

// Initialize context
let context = GraphicsContext::new_sync();

// Create renderer
let renderer = Renderer::new(context);

// Create resources
let buffer = renderer.create_vertex_buffer(Some("Vertices"), &vertices);
```

## Modules

### `context`

- `GraphicsContext`: The heart of the rendering system. Created once and leaked as `&'static`.

### `window`

- `WindowContext`: Manages the presentation surface for a window. Handles resizing and format selection.

### `renderer`

- `Renderer`: Base struct for creating GPU resources (buffers, textures, bind groups, pipelines).
- `RenderPassBuilder`: Helper for creating render passes.

### `frame`

- `Frame`: Represents an active frame. Manages the command encoder and surface texture.

### `color`

- `Color`: RGBA color struct with conversion to/from WGPU types and hex strings.
