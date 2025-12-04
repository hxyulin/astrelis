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
//! - Low-level extensible Renderer for WGPU resource management
//! - Building blocks for higher-level renderers (TextRenderer, SceneRenderer, etc.)

mod blend;
mod color;
mod compute;
mod context;
mod features;
mod frame;
mod framebuffer;
mod indirect;
mod renderer;
mod target;
mod window;

// Re-export all modules
pub use blend::*;
pub use color::*;
pub use compute::*;
pub use context::*;
pub use features::*;
pub use frame::*;
pub use framebuffer::*;
pub use indirect::*;
pub use renderer::*;
pub use target::*;
pub use window::*;

// Re-export wgpu under 'wgpu' module
pub use wgpu;
