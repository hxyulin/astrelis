//! Astrelis Render - Modular rendering framework for Astrelis
//!
//! This crate provides:
//! - Graphics context management
//! - Window rendering contexts
//! - Frame and render pass management
//! - Compute pass management
//! - Framebuffer abstraction for offscreen rendering
//! - Render target abstraction (Surface/Framebuffer)
//! - Blend mode presets for common scenarios
//! - GPU feature detection and management
//! - Indirect draw buffer support for GPU-driven rendering
//! - Texture blitting for fullscreen quad rendering
//! - Sprite sheet support for animations
//! - Low-level extensible Renderer for WGPU resource management
//! - Building blocks for higher-level renderers (TextRenderer, SceneRenderer, etc.)
//!
//! ## Error Handling
//!
//! This crate uses consistent error handling patterns:
//!
//! ### Result Types
//! - **Creation methods** return `Result<T, GraphicsError>` for GPU initialization
//!   - Example: `GraphicsContext::new_owned_sync()` returns `Result<Arc<Self>, GraphicsError>`
//! - **Fallible operations** return `Result<T, E>` with specific error types
//!   - Example: `Readback::read()` returns `Result<Vec<u8>, ReadbackError>`
//! - **Use `.expect()` for examples/tests** where error handling isn't critical
//!   - Example: `let ctx = GraphicsContext::new_owned_sync().expect("GPU required")`
//!
//! ### Option Types
//! - **Optional resources** return `Option<&T>` for possibly-missing values
//!   - Example: `WindowManager::get_window(id)` returns `Option<&RenderableWindow>`
//! - **Hit testing** returns `Option<T>` for no-hit scenarios
//!   - Example: `hit_test(point)` returns `Option<WidgetId>`
//!
//! ### Panicking vs Fallible
//! - **Avoid panic-suffixed methods** - Use `.expect()` at call sites instead
//!   - ❌ Bad: `resource_or_panic()` method
//!   - ✅ Good: `resource().expect("Resource required")` at call site
//! - **Provide both variants** for common operations
//!   - `resource()` - Panics if unavailable (use when required)
//!   - `try_resource()` - Returns `Option` (use when optional)

mod atlas;
pub mod batched;
mod blend;
mod blit;
mod buffer_pool;
mod camera;
pub mod capability;
mod color;
mod compute;
mod context;
mod depth;
mod extension;
mod features;
mod frame;
mod framebuffer;
pub mod gpu_profiling;
mod indirect;
mod line_renderer;
mod material;
mod mesh;
mod point_renderer;
mod quad_renderer;
mod query;
mod readback;
mod render_graph;
mod renderer;
mod sampler_cache;
mod sprite;
mod target;
pub mod transform;
mod types;
mod window;
mod window_manager;

// Re-export all modules
pub use atlas::*;
pub use blend::*;
pub use blit::*;
pub use buffer_pool::*;
pub use camera::*;
pub use capability::{GpuRequirements, RenderCapability};
pub use color::*;
pub use compute::*;
pub use context::*;
pub use depth::*;
pub use extension::*;
pub use features::*;
pub use frame::*;
pub use framebuffer::*;
pub use indirect::*;
pub use line_renderer::*;
pub use material::*;
pub use mesh::*;
pub use point_renderer::*;
pub use quad_renderer::*;
pub use query::*;
pub use readback::*;
pub use render_graph::*;
pub use renderer::*;
pub use sampler_cache::*;
pub use sprite::*;
pub use target::*;
pub use transform::{DataRangeParams, DataTransform};
pub use types::*;
pub use window::*;
pub use window_manager::*;

// Re-export wgpu under 'wgpu' module
pub use wgpu;
