//! [wgpu](https://wgpu.rs/) 29 GPU backend for the Astrelis engine.
//!
//! This crate implements [`astrelis_gpu::GpuBackend`] using wgpu, providing
//! cross-platform GPU access via Vulkan, Metal, DX12, and OpenGL.
//!
//! # Quick Start
//!
//! ```ignore
//! use astrelis_gpu::{GpuBackend, GpuConfig};
//! use astrelis_gpu_wgpu::WgpuBackend;
//!
//! let gpu = WgpuBackend::new(&GpuConfig::default())?;
//! println!("GPU: {}", gpu.device().adapter_info().name);
//!
//! let surface = gpu.create_surface(window)?;
//! ```
//!
//! Only [`WgpuBackend`] is publicly exported. All other types are accessed
//! through the [`astrelis_gpu`] trait interfaces.

#![warn(missing_docs)]

mod backend;
mod compute_pass;
mod convert;
mod device;
mod encoder;
mod queue;
mod render_pass;
mod resources;
mod surface;

pub use backend::WgpuBackend;
