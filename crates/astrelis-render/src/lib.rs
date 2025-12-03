//! Astrelis Render - Modular rendering framework for Astrelis
//!
//! This crate provides:
//! - Graphics context management
//! - Window rendering contexts
//! - Frame and render pass management
//! - Low-level extensible Renderer for WGPU resource management
//! - Building blocks for higher-level renderers (TextRenderer, SceneRenderer, etc.)

mod color;
mod context;
mod frame;
mod renderer;
mod window;

// Re-export all modules
pub use color::*;
pub use context::*;
pub use frame::*;
pub use renderer::*;
pub use window::*;

// Re-export wgpu under 'wgpu' module
pub use wgpu;
