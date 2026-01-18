//! Trait abstracting GPU operations for testing.
//!
//! The `RenderContext` trait provides an abstraction over GPU operations,
//! allowing for both real GPU usage and mock implementations for testing.

use crate::gpu_types::*;
use wgpu::{
    BindGroupDescriptor, BindGroupLayoutDescriptor, BufferDescriptor, ComputePipelineDescriptor,
    RenderPipelineDescriptor, SamplerDescriptor, ShaderModuleDescriptor, TextureDescriptor,
};

/// Trait abstracting GPU resource creation and operations.
///
/// # Lifetime Considerations
///
/// This trait does NOT use lifetimes because:
/// 1. All returned types are owned (not borrowed from Device)
/// 2. GPU resources use reference counting internally
/// 3. Resources live until dropped
///
/// This makes the trait object-safe and easy to mock.
///
/// # Borrow Checking Pattern
///
/// Methods take `&self` (shared reference) and return owned wrapper types.
/// This allows:
/// - Multiple components to share the same context (via Arc)
/// - Mock implementations to use interior mutability (Mutex)
/// - No lifetime parameters propagating through the codebase
///
/// # Example
///
/// ```rust,no_run
/// use astrelis_test_utils::RenderContext;
/// use wgpu::{BufferDescriptor, BufferUsages};
///
/// fn render_scene(ctx: &dyn RenderContext) {
///     let desc = BufferDescriptor {
///         label: None,
///         size: 16,
///         usage: BufferUsages::VERTEX,
///         mapped_at_creation: false,
///     };
///     let data = vec![0u8; 16];
///     let buffer = ctx.create_buffer(&desc);
///     ctx.write_buffer(&buffer, 0, &data);
///     // buffer is owned, no lifetime issues
/// }
/// ```
pub trait RenderContext: Send + Sync {
    // Buffer operations

    /// Create a GPU buffer.
    ///
    /// Returns an owned `GpuBuffer` which can be either real or mock.
    fn create_buffer(&self, desc: &BufferDescriptor) -> GpuBuffer;

    /// Write data to a buffer.
    ///
    /// For real buffers, this maps to `queue.write_buffer()`.
    /// For mock buffers, this records the operation for test verification.
    fn write_buffer(&self, buffer: &GpuBuffer, offset: u64, data: &[u8]);

    // Texture operations

    /// Create a GPU texture.
    fn create_texture(&self, desc: &TextureDescriptor) -> GpuTexture;

    // Note: write_texture method will be added when needed during migration
    // It requires wgpu types that aren't in the public API surface we're using

    // Shader operations

    /// Create a shader module from source code.
    fn create_shader_module(&self, desc: &ShaderModuleDescriptor) -> GpuShaderModule;

    // Pipeline operations

    /// Create a render pipeline.
    fn create_render_pipeline(&self, desc: &RenderPipelineDescriptor) -> GpuRenderPipeline;

    /// Create a compute pipeline.
    fn create_compute_pipeline(&self, desc: &ComputePipelineDescriptor) -> GpuComputePipeline;

    // Bind group operations

    /// Create a bind group layout.
    fn create_bind_group_layout(&self, desc: &BindGroupLayoutDescriptor) -> GpuBindGroupLayout;

    /// Create a bind group.
    fn create_bind_group(&self, desc: &BindGroupDescriptor) -> GpuBindGroup;

    // Sampler operations

    /// Create a texture sampler.
    fn create_sampler(&self, desc: &SamplerDescriptor) -> GpuSampler;
}

/// Helper trait for converting WGPU descriptors that reference GPU resources.
///
/// This is needed because WGPU descriptors contain references to concrete types
/// like `&wgpu::Buffer`, but our wrappers contain `GpuBuffer`.
///
/// We'll implement conversion helpers as needed.
pub trait DescriptorHelper {
    /// Convert a descriptor that references wrapped GPU types to one that
    /// references WGPU types (for real GPU operations).
    type WgpuDescriptor<'a>
    where
        Self: 'a;

    fn to_wgpu(&self) -> Self::WgpuDescriptor<'_>;
}

// Note: We'll implement DescriptorHelper for specific descriptors as needed
// during the migration. For now, most descriptors don't reference GPU resources
// directly and can be used as-is.
