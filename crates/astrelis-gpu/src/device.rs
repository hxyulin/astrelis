//! GPU device trait and related types.

use crate::bind_group::{BindGroupDescriptor, BindGroupLayoutDescriptor};
use crate::buffer::{BufferDescriptor, BufferInitDescriptor};
use crate::command::{BufferCopyView, CommandEncoder, TextureCopyView};
use crate::error::GpuError;
use crate::id::{
    BindGroupId, BindGroupLayoutId, BufferId, ComputePipelineId, PipelineLayoutId,
    RenderPipelineId, SamplerId, ShaderModuleId, TextureId, TextureViewId,
};
use crate::pipeline::{
    ComputePipelineDescriptor, PipelineLayoutDescriptor, RenderPipelineDescriptor,
};
use crate::shader::ShaderModuleDescriptor;
use crate::texture::{Extent3d, SamplerDescriptor, TextureDescriptor, TextureViewDescriptor};

/// Information about the GPU adapter.
#[derive(Clone, Debug)]
pub struct AdapterInfo {
    /// Human-readable adapter name (e.g., "NVIDIA GeForce RTX 4090").
    pub name: String,
    /// The graphics API backend in use.
    pub backend: GpuBackendType,
    /// The type of GPU device.
    pub device_type: DeviceType,
}

/// Which graphics API backend is in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GpuBackendType {
    /// Vulkan (Linux, Windows, Android).
    Vulkan,
    /// Metal (macOS, iOS).
    Metal,
    /// DirectX 12 (Windows).
    Dx12,
    /// OpenGL / OpenGL ES.
    Gl,
    /// WebGPU (browser).
    BrowserWebGpu,
}

/// Type of GPU device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceType {
    /// Dedicated graphics card.
    DiscreteGpu,
    /// GPU integrated into the CPU.
    IntegratedGpu,
    /// Software/virtual GPU.
    VirtualGpu,
    /// CPU-based rendering.
    Cpu,
    /// Unknown or other type.
    Other,
}

/// The main GPU device trait for creating and managing resources.
///
/// All resource creation methods return typed handles ([`BufferId`],
/// [`TextureId`], etc.). The backend owns the actual GPU objects;
/// handles are lightweight IDs.
///
/// Methods take `&self` — backends use interior mutability for thread safety.
pub trait GpuDevice {
    /// The command encoder type for this backend.
    type Encoder: CommandEncoder;

    /// Returns information about the GPU adapter.
    fn adapter_info(&self) -> &AdapterInfo;

    // --- Buffer ---

    /// Creates a GPU buffer.
    fn create_buffer(&self, desc: &BufferDescriptor<'_>) -> Result<BufferId, GpuError>;

    /// Creates a GPU buffer with initial data.
    fn create_buffer_init(&self, desc: &BufferInitDescriptor<'_>) -> Result<BufferId, GpuError>;

    /// Destroys a buffer, releasing its GPU memory.
    fn destroy_buffer(&self, id: BufferId);

    /// Writes data to a buffer at the given byte offset.
    fn write_buffer(&self, buffer: BufferId, offset: u64, data: &[u8]);

    // --- Texture ---

    /// Creates a GPU texture.
    fn create_texture(&self, desc: &TextureDescriptor<'_>) -> Result<TextureId, GpuError>;

    /// Creates a view into a texture.
    fn create_texture_view(
        &self,
        texture: TextureId,
        desc: &TextureViewDescriptor<'_>,
    ) -> Result<TextureViewId, GpuError>;

    /// Creates a texture sampler.
    fn create_sampler(&self, desc: &SamplerDescriptor<'_>) -> Result<SamplerId, GpuError>;

    /// Destroys a texture.
    fn destroy_texture(&self, id: TextureId);

    /// Destroys a texture view.
    fn destroy_texture_view(&self, id: TextureViewId);

    /// Destroys a sampler.
    fn destroy_sampler(&self, id: SamplerId);

    /// Writes data to a texture.
    fn write_texture(
        &self,
        dst: TextureCopyView,
        data: &[u8],
        layout: BufferCopyView,
        size: Extent3d,
    );

    // --- Shader ---

    /// Creates a shader module from source code.
    fn create_shader_module(
        &self,
        desc: &ShaderModuleDescriptor<'_>,
    ) -> Result<ShaderModuleId, GpuError>;

    /// Destroys a shader module.
    fn destroy_shader_module(&self, id: ShaderModuleId);

    // --- Bind group ---

    /// Creates a bind group layout.
    fn create_bind_group_layout(
        &self,
        desc: &BindGroupLayoutDescriptor<'_>,
    ) -> Result<BindGroupLayoutId, GpuError>;

    /// Creates a bind group.
    fn create_bind_group(
        &self,
        desc: &BindGroupDescriptor<'_>,
    ) -> Result<BindGroupId, GpuError>;

    /// Destroys a bind group layout.
    fn destroy_bind_group_layout(&self, id: BindGroupLayoutId);

    /// Destroys a bind group.
    fn destroy_bind_group(&self, id: BindGroupId);

    // --- Pipeline ---

    /// Creates a pipeline layout.
    fn create_pipeline_layout(
        &self,
        desc: &PipelineLayoutDescriptor<'_>,
    ) -> Result<PipelineLayoutId, GpuError>;

    /// Creates a render pipeline.
    fn create_render_pipeline(
        &self,
        desc: &RenderPipelineDescriptor<'_>,
    ) -> Result<RenderPipelineId, GpuError>;

    /// Creates a compute pipeline.
    fn create_compute_pipeline(
        &self,
        desc: &ComputePipelineDescriptor<'_>,
    ) -> Result<ComputePipelineId, GpuError>;

    /// Destroys a pipeline layout.
    fn destroy_pipeline_layout(&self, id: PipelineLayoutId);

    /// Destroys a render pipeline.
    fn destroy_render_pipeline(&self, id: RenderPipelineId);

    /// Destroys a compute pipeline.
    fn destroy_compute_pipeline(&self, id: ComputePipelineId);

    // --- Command ---

    /// Creates a new command encoder for recording GPU commands.
    fn create_command_encoder(&self, label: Option<&str>) -> Self::Encoder;
}
