//! Extension traits for low-level wgpu access.
//!
//! This module provides traits that allow accessing the underlying wgpu types
//! from the Astrelis wrapper types. Use these when you need raw wgpu access
//! for advanced use cases not covered by the high-level API.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_render::{GraphicsContext, GraphicsContextExt, AsWgpu};
//!
//! let ctx = GraphicsContext::new_owned_sync();
//!
//! // Access raw wgpu device
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

use crate::{
    ComputePass, FrameContext, Framebuffer, GraphicsContext, RenderPass, WindowContext,
};

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
// GraphicsContextExt
// =============================================================================

/// Extended access to GraphicsContext internals.
///
/// This trait provides direct access to the underlying wgpu components
/// for advanced use cases that require raw wgpu operations.
pub trait GraphicsContextExt {
    /// Get a reference to the wgpu device.
    fn device(&self) -> &wgpu::Device;

    /// Get a reference to the wgpu queue.
    fn queue(&self) -> &wgpu::Queue;

    /// Get a reference to the wgpu adapter.
    fn adapter(&self) -> &wgpu::Adapter;

    /// Get a reference to the wgpu instance.
    fn instance(&self) -> &wgpu::Instance;
}

impl GraphicsContextExt for GraphicsContext {
    fn device(&self) -> &wgpu::Device {
        &self.device
    }

    fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }
}

impl GraphicsContextExt for Arc<GraphicsContext> {
    fn device(&self) -> &wgpu::Device {
        &self.device
    }

    fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }
}

// =============================================================================
// AsWgpu Implementations
// =============================================================================

impl AsWgpu for GraphicsContext {
    type WgpuType = wgpu::Device;

    fn as_wgpu(&self) -> &Self::WgpuType {
        &self.device
    }
}

impl AsWgpu for Arc<GraphicsContext> {
    type WgpuType = wgpu::Device;

    fn as_wgpu(&self) -> &Self::WgpuType {
        &self.device
    }
}

impl<'a> AsWgpu for FrameContext {
    type WgpuType = wgpu::CommandEncoder;

    fn as_wgpu(&self) -> &Self::WgpuType {
        self.encoder.as_ref().expect("Encoder already taken")
    }
}

impl<'a> AsWgpuMut for FrameContext {
    fn as_wgpu_mut(&mut self) -> &mut Self::WgpuType {
        self.encoder.as_mut().expect("Encoder already taken")
    }
}

impl<'a> AsWgpu for RenderPass<'a> {
    type WgpuType = wgpu::RenderPass<'static>;

    fn as_wgpu(&self) -> &Self::WgpuType {
        self.descriptor.as_ref().unwrap()
    }
}

impl<'a> AsWgpuMut for RenderPass<'a> {
    fn as_wgpu_mut(&mut self) -> &mut Self::WgpuType {
        self.descriptor.as_mut().unwrap()
    }
}

impl<'a> AsWgpu for ComputePass<'a> {
    type WgpuType = wgpu::ComputePass<'static>;

    fn as_wgpu(&self) -> &Self::WgpuType {
        self.pass.as_ref().unwrap()
    }
}

impl<'a> AsWgpuMut for ComputePass<'a> {
    fn as_wgpu_mut(&mut self) -> &mut Self::WgpuType {
        self.pass.as_mut().unwrap()
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

// =============================================================================
// FrameContextExt
// =============================================================================

/// Extended access to FrameContext internals.
pub trait FrameContextExt {
    /// Get direct access to the command encoder.
    fn encoder_ref(&self) -> Option<&wgpu::CommandEncoder>;

    /// Get mutable access to the command encoder.
    fn encoder_mut(&mut self) -> Option<&mut wgpu::CommandEncoder>;

    /// Get the surface texture view for this frame.
    fn surface_view(&self) -> &wgpu::TextureView;

    /// Get the surface texture for this frame.
    fn surface_texture(&self) -> &wgpu::Texture;
}

impl FrameContextExt for FrameContext {
    fn encoder_ref(&self) -> Option<&wgpu::CommandEncoder> {
        self.encoder.as_ref()
    }

    fn encoder_mut(&mut self) -> Option<&mut wgpu::CommandEncoder> {
        self.encoder.as_mut()
    }

    fn surface_view(&self) -> &wgpu::TextureView {
        self.surface().view()
    }

    fn surface_texture(&self) -> &wgpu::Texture {
        self.surface().texture()
    }
}

// =============================================================================
// RenderPassExt (for raw access)
// =============================================================================

/// Extended access to RenderPass internals.
pub trait RenderPassRawExt<'a> {
    /// Get raw access to the underlying wgpu render pass.
    fn raw_pass(&mut self) -> &mut wgpu::RenderPass<'static>;

    /// Get the graphics context.
    fn graphics_context(&self) -> &GraphicsContext;
}

impl<'a> RenderPassRawExt<'a> for RenderPass<'a> {
    fn raw_pass(&mut self) -> &mut wgpu::RenderPass<'static> {
        self.descriptor.as_mut().unwrap()
    }

    fn graphics_context(&self) -> &GraphicsContext {
        &self.context.context
    }
}

// =============================================================================
// ComputePassExt (for raw access)
// =============================================================================

/// Extended access to ComputePass internals.
pub trait ComputePassRawExt<'a> {
    /// Get raw access to the underlying wgpu compute pass.
    fn raw_pass(&mut self) -> &mut wgpu::ComputePass<'static>;

    /// Get the graphics context.
    fn graphics_context(&self) -> &GraphicsContext;
}

impl<'a> ComputePassRawExt<'a> for ComputePass<'a> {
    fn raw_pass(&mut self) -> &mut wgpu::ComputePass<'static> {
        self.pass.as_mut().unwrap()
    }

    fn graphics_context(&self) -> &GraphicsContext {
        &self.context.context
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
        fn _takes_graphics_context_ext<T: GraphicsContextExt>(_: &T) {}
    }
}
