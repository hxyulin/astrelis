//! Newtype wrappers around wgpu GPU resource types.
//!
//! Each wrapper provides direct ownership of the underlying wgpu resource
//! and a `raw()` escape hatch for advanced use cases. Dropping a wrapper
//! releases the GPU resource (wgpu handles cleanup via internal `Arc`s).

/// A GPU buffer.
pub struct Buffer(pub(crate) wgpu::Buffer);

impl Buffer {
    /// Access the underlying [`wgpu::Buffer`].
    pub fn raw(&self) -> &wgpu::Buffer {
        &self.0
    }

    /// Returns the size of the buffer in bytes.
    pub fn size(&self) -> u64 {
        self.0.size()
    }
}

/// A GPU texture.
pub struct Texture(pub(crate) wgpu::Texture);

impl Texture {
    /// Access the underlying [`wgpu::Texture`].
    pub fn raw(&self) -> &wgpu::Texture {
        &self.0
    }
}

/// A view into a texture.
pub struct TextureView(pub(crate) wgpu::TextureView);

impl TextureView {
    /// Access the underlying [`wgpu::TextureView`].
    pub fn raw(&self) -> &wgpu::TextureView {
        &self.0
    }
}

/// A texture sampler.
pub struct Sampler(pub(crate) wgpu::Sampler);

impl Sampler {
    /// Access the underlying [`wgpu::Sampler`].
    pub fn raw(&self) -> &wgpu::Sampler {
        &self.0
    }
}

/// A compiled shader module.
pub struct ShaderModule(pub(crate) wgpu::ShaderModule);

impl ShaderModule {
    /// Access the underlying [`wgpu::ShaderModule`].
    pub fn raw(&self) -> &wgpu::ShaderModule {
        &self.0
    }
}

/// A bind group layout describing the shape of a bind group.
pub struct BindGroupLayout(pub(crate) wgpu::BindGroupLayout);

impl BindGroupLayout {
    /// Access the underlying [`wgpu::BindGroupLayout`].
    pub fn raw(&self) -> &wgpu::BindGroupLayout {
        &self.0
    }
}

/// A bind group — a set of resources bound together for shader access.
pub struct BindGroup(pub(crate) wgpu::BindGroup);

impl BindGroup {
    /// Access the underlying [`wgpu::BindGroup`].
    pub fn raw(&self) -> &wgpu::BindGroup {
        &self.0
    }
}

/// A pipeline layout describing bind group layouts and push constant ranges.
pub struct PipelineLayout(pub(crate) wgpu::PipelineLayout);

impl PipelineLayout {
    /// Access the underlying [`wgpu::PipelineLayout`].
    pub fn raw(&self) -> &wgpu::PipelineLayout {
        &self.0
    }
}

/// A render pipeline.
pub struct RenderPipeline(pub(crate) wgpu::RenderPipeline);

impl RenderPipeline {
    /// Access the underlying [`wgpu::RenderPipeline`].
    pub fn raw(&self) -> &wgpu::RenderPipeline {
        &self.0
    }
}

/// A compute pipeline.
pub struct ComputePipeline(pub(crate) wgpu::ComputePipeline);

impl ComputePipeline {
    /// Access the underlying [`wgpu::ComputePipeline`].
    pub fn raw(&self) -> &wgpu::ComputePipeline {
        &self.0
    }
}
