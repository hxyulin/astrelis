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

mod atlas;
mod blend;
mod blit;
mod buffer_pool;
mod camera;
mod color;
mod compute;
mod context;
mod context_impl;
mod extension;
mod features;
mod frame;
mod framebuffer;
mod indirect;
mod material;
mod mesh;
mod query;
mod readback;
mod render_graph;
mod renderer;
mod sprite;
mod target;
mod types;
mod window;

// Re-export all modules
pub use atlas::*;
pub use blend::*;
pub use extension::*;
pub use blit::*;
pub use buffer_pool::*;
pub use camera::*;
pub use color::*;
pub use compute::*;
pub use context::*;
pub use features::*;
pub use frame::*;
pub use framebuffer::*;
pub use indirect::*;
pub use material::*;
pub use mesh::*;
pub use query::*;
pub use readback::*;
pub use render_graph::*;
pub use renderer::*;
pub use sprite::*;
pub use target::*;
pub use types::*;
pub use window::*;

// Re-export wgpu under 'wgpu' module
pub use wgpu;
