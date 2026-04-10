//! Backend-agnostic GPU abstraction traits and types for the Astrelis engine.
//!
//! This crate defines platform-independent GPU abstractions. Concrete
//! implementations (e.g., `astrelis-gpu-wgpu`) live in separate crates.
//!
//! # Architecture
//!
//! - [`GpuBackend`] — top-level entry point; initializes adapter and device
//! - [`GpuDevice`] — creates and destroys GPU resources, returns typed handles
//! - [`GpuQueue`] — submits recorded commands for execution
//! - [`GpuSurface`] / [`SurfaceTexture`] — manages presentation to a window
//! - [`CommandEncoder`] — records GPU commands into a command buffer
//! - [`RenderPass`] / [`ComputePass`] — record render / compute work within a pass
//!
//! # Resource Handles
//!
//! GPU resources are identified by lightweight typed handles (e.g.,
//! [`BufferId`](id::BufferId), [`TextureId`](id::TextureId)). The backend
//! owns the actual GPU objects; handles are `Copy + Send + Sync` IDs built
//! on [`astrelis_core::id::Id<T>`].
//!
//! # Example
//!
//! ```ignore
//! use astrelis_gpu::backend::{GpuBackend, GpuConfig};
//! use astrelis_gpu::command::{ColorAttachment, CommandEncoder, RenderPassDescriptor};
//! use astrelis_gpu::device::GpuDevice;
//! use astrelis_gpu::queue::GpuQueue;
//! use astrelis_gpu::surface::{GpuSurface, SurfaceConfiguration, SurfaceTexture};
//! use astrelis_gpu::types::{LoadOp, PresentMode, StoreOp};
//! use astrelis_gpu_wgpu::WgpuBackend;
//!
//! let gpu = WgpuBackend::new(&GpuConfig::default())?;
//! let mut surface = gpu.create_surface(window)?;
//! surface.configure(&SurfaceConfiguration {
//!     format: surface.preferred_format(),
//!     width: 800, height: 600,
//!     present_mode: PresentMode::AutoVsync,
//!     desired_maximum_frame_latency: 2,
//! });
//!
//! // Render loop: acquire → record → submit → present
//! let frame = surface.acquire()?;
//! let mut encoder = gpu.device().create_command_encoder(Some("frame"));
//! {
//!     let _pass = encoder.begin_render_pass(&RenderPassDescriptor {
//!         label: Some("main"),
//!         color_attachments: &[ColorAttachment {
//!             view: frame.view(),
//!             resolve_target: None,
//!             load_op: LoadOp::Clear(Color::BLACK),
//!             store_op: StoreOp::Store,
//!         }],
//!         depth_stencil_attachment: None,
//!     });
//! }
//! gpu.queue().submit(std::iter::once(encoder));
//! frame.present();
//! ```

#![warn(missing_docs)]

pub mod backend;
pub mod bind_group;
pub mod buffer;
pub mod command;
pub mod device;
pub mod error;
pub mod id;
pub mod pipeline;
pub mod profiling;
pub mod queue;
pub mod shader;
pub mod surface;
pub mod texture;
pub mod types;

// Convenience re-exports.
pub use backend::{GpuBackend, GpuConfig};
pub use command::{CommandEncoder, ComputePass, RenderPass};
pub use device::GpuDevice;
pub use error::GpuError;
pub use queue::GpuQueue;
pub use profiling::{GpuProfilingCapabilities, GpuProfilingTier};
pub use surface::{GpuSurface, SurfaceTexture};
