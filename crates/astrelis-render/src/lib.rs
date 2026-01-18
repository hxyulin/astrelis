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

mod blend;
mod blit;
mod color;
mod compute;
mod context;
mod context_impl;
mod features;
mod frame;
mod framebuffer;
mod indirect;
mod renderer;
mod sprite;
mod target;
mod window;

// Re-export all modules
pub use blend::*;
pub use blit::*;
pub use color::*;
pub use compute::*;
pub use context::*;
pub use features::*;
pub use frame::*;
pub use framebuffer::*;
pub use indirect::*;
pub use renderer::*;
pub use sprite::*;
pub use target::*;
pub use window::*;

// Re-export wgpu under 'wgpu' module
pub use wgpu;
