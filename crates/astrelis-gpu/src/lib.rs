//! Concrete wgpu-based GPU types for the Astrelis engine.
//!
//! This crate provides direct wgpu-backed GPU abstractions with thin newtype
//! wrappers and no trait indirection.
//!
//! # Architecture
//!
//! - [`Gpu`] — top-level entry point; initializes adapter and device
//! - [`GpuDevice`] — creates GPU resources, returning owned newtype wrappers
//! - [`CommandEncoder`] — records GPU commands into a command buffer
//! - [`RenderPass`] / [`ComputePass`] — record render / compute work within a pass
//! - [`Surface`] / [`SurfaceFrame`] — manages presentation to a window
//!
//! # Resources
//!
//! GPU resources are represented as lightweight newtype wrappers around wgpu
//! types (e.g., [`Buffer`], [`Texture`], [`TextureView`]). Dropping a wrapper
//! releases the GPU resource. Each wrapper provides a `raw()` escape hatch
//! for direct wgpu access.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_gpu::{Gpu, GpuConfig};
//! use astrelis_gpu::command::{ColorAttachment, RenderPassDescriptor};
//! use astrelis_gpu::surface::SurfaceConfiguration;
//! use astrelis_gpu::types::{LoadOp, PresentMode, StoreOp};
//!
//! let gpu = Gpu::new(&GpuConfig::default())?;
//! let mut surface = gpu.create_surface(window)?;
//! surface.configure(&SurfaceConfiguration {
//!     format: surface.preferred_format(),
//!     width: 800, height: 600,
//!     present_mode: PresentMode::AutoVsync,
//!     desired_maximum_frame_latency: 2,
//! });
//!
//! // Render loop: acquire -> record -> submit -> present
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
//! gpu.submit(std::iter::once(encoder));
//! frame.present();
//! ```

#![warn(missing_docs)]

pub mod backend;
pub mod bind_group;
pub mod buffer;
pub mod command;
/// Type conversion utilities between engine types and `wgpu` types.
pub mod convert;
pub mod device;
pub mod error;
pub mod pipeline;
pub(crate) mod profiling;
pub mod resources;
pub mod shader;
pub mod surface;
pub mod texture;
pub mod types;

// Convenience re-exports.
pub use backend::{Gpu, GpuConfig};
pub use command::{CommandEncoder, ComputePass, RenderPass};
pub use device::GpuDevice;
pub use error::GpuError;
pub use profiling::{GpuProfilingCapabilities, GpuProfilingTier};
pub use resources::*;
pub use surface::{Surface, SurfaceFrame};
