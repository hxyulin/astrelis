//! Extension traits for low-level wgpu access.
//!
//! This module provides traits that allow accessing the underlying wgpu types
//! from the Astrelis wrapper types. Use these when you need raw wgpu access
//! for advanced use cases not covered by the high-level API.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_render::{GraphicsContext, AsWgpu};
//!
//! let ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
//!
//! // Access raw wgpu device via inherent methods
//! let device: &wgpu::Device = ctx.device();
//! let queue: &wgpu::Queue = ctx.queue();
//!
//! // Create custom wgpu resources
//! let buffer = device.create_buffer(&wgpu::BufferDescriptor {
//!     label: Some("Custom Buffer"),
//!     size: 1024,
//!     usage: wgpu::BufferUsages::UNIFORM,
//!     mapped_at_creation: false,
//! });
//! ```

use std::sync::Arc;

use crate::{ComputePass, Framebuffer, GraphicsContext, RenderPass, WindowContext};

// =============================================================================
// Core Extension Traits
// =============================================================================

/// Access the underlying wgpu type (immutable).
///
/// Implement this trait to expose the underlying wgpu type for advanced access.
pub trait AsWgpu {
    /// The underlying wgpu type.
    type WgpuType;

    /// Get a reference to the underlying wgpu type.
    fn as_wgpu(&self) -> &Self::WgpuType;
}

/// Access the underlying wgpu type (mutable).
///
/// Implement this trait to expose mutable access to the underlying wgpu type.
pub trait AsWgpuMut: AsWgpu {
    /// Get a mutable reference to the underlying wgpu type.
    fn as_wgpu_mut(&mut self) -> &mut Self::WgpuType;
}

/// Consume and return the underlying wgpu type.
///
/// Implement this trait when ownership of the wgpu type can be transferred.
pub trait IntoWgpu {
    /// The underlying wgpu type.
    type WgpuType;

    /// Consume self and return the underlying wgpu type.
    fn into_wgpu(self) -> Self::WgpuType;
}

// =============================================================================
// AsWgpu Implementations
// =============================================================================

impl AsWgpu for GraphicsContext {
    type WgpuType = wgpu::Device;

    fn as_wgpu(&self) -> &Self::WgpuType {
        self.device()
    }
}

impl AsWgpu for Arc<GraphicsContext> {
    type WgpuType = wgpu::Device;

    fn as_wgpu(&self) -> &Self::WgpuType {
        self.device()
    }
}

// Note: Frame no longer implements AsWgpu for CommandEncoder because each RenderPass
// now owns its own encoder. Use `frame.create_encoder()` to get an encoder, or
// access via RenderPass::encoder()/encoder_mut().

impl<'a> AsWgpu for RenderPass<'a> {
    type WgpuType = wgpu::RenderPass<'static>;

    fn as_wgpu(&self) -> &Self::WgpuType {
        self.wgpu_pass_ref()
    }
}

impl<'a> AsWgpuMut for RenderPass<'a> {
    fn as_wgpu_mut(&mut self) -> &mut Self::WgpuType {
        self.wgpu_pass()
    }
}

impl<'a> AsWgpu for ComputePass<'a> {
    type WgpuType = wgpu::ComputePass<'static>;

    fn as_wgpu(&self) -> &Self::WgpuType {
        self.wgpu_pass_ref()
    }
}

impl<'a> AsWgpuMut for ComputePass<'a> {
    fn as_wgpu_mut(&mut self) -> &mut Self::WgpuType {
        self.wgpu_pass()
    }
}

impl AsWgpu for Framebuffer {
    type WgpuType = wgpu::Texture;

    fn as_wgpu(&self) -> &Self::WgpuType {
        self.color_texture()
    }
}

impl AsWgpu for WindowContext {
    type WgpuType = wgpu::Surface<'static>;

    fn as_wgpu(&self) -> &Self::WgpuType {
        &self.surface
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a GPU context, so they're integration tests.
    // Here we just test that the traits compile correctly.

    #[test]
    fn test_trait_object_safety() {
        // Ensure the core traits can be used as trait objects where applicable
        fn _takes_as_wgpu<T: AsWgpu>(_: &T) {}
        fn _takes_as_wgpu_mut<T: AsWgpuMut>(_: &mut T) {}
        fn _takes_into_wgpu<T: IntoWgpu>(_: T) {}
    }
}
