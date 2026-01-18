//! GPU resource wrappers that can be real or mock.
//!
//! These types wrap WGPU resources and allow for both real GPU operations
//! and mock implementations for testing.

use wgpu;

/// Wrapper around GPU buffer that can be real or mock.
///
/// # Design Pattern: Opaque Wrapper
///
/// This type hides whether it contains a real `wgpu::Buffer` or a mock.
/// Users hold owned `GpuBuffer`, which is cheap to clone (Arc inside).
///
/// # Benefits
/// 1. No lifetimes - users own the buffer
/// 2. Can be mock or real without user knowing
/// 3. Clone is cheap (Arc internally for real buffers)
#[derive(Clone, Debug)]
pub struct GpuBuffer {
    inner: GpuBufferInner,
}

#[derive(Clone, Debug)]
enum GpuBufferInner {
    Real(wgpu::Buffer),
    #[cfg(feature = "mock")]
    Mock { id: usize, size: u64 },
}

impl GpuBuffer {
    /// Create from real WGPU buffer
    pub fn from_wgpu(buffer: wgpu::Buffer) -> Self {
        Self {
            inner: GpuBufferInner::Real(buffer),
        }
    }

    /// Create mock buffer (for testing)
    #[cfg(feature = "mock")]
    pub fn mock(id: usize, size: u64) -> Self {
        Self {
            inner: GpuBufferInner::Mock { id, size },
        }
    }

    /// Get the underlying wgpu::Buffer (if real)
    ///
    /// # Panics
    /// Panics if this is a mock buffer (test code should never call this)
    pub fn as_wgpu(&self) -> &wgpu::Buffer {
        match &self.inner {
            GpuBufferInner::Real(buffer) => buffer,
            #[cfg(feature = "mock")]
            GpuBufferInner::Mock { .. } => {
                panic!("Attempted to get wgpu::Buffer from mock buffer - this is a test-only buffer")
            }
        }
    }

    /// Check if this is a mock (useful in tests)
    #[cfg(feature = "mock")]
    pub fn is_mock(&self) -> bool {
        matches!(self.inner, GpuBufferInner::Mock { .. })
    }

    /// Get mock ID (for test assertions)
    #[cfg(feature = "mock")]
    pub fn mock_id(&self) -> Option<usize> {
        match &self.inner {
            GpuBufferInner::Mock { id, .. } => Some(*id),
            _ => None,
        }
    }
}

/// Wrapper around GPU texture that can be real or mock.
#[derive(Clone, Debug)]
pub struct GpuTexture {
    inner: GpuTextureInner,
}

#[derive(Clone, Debug)]
enum GpuTextureInner {
    Real(wgpu::Texture),
    #[cfg(feature = "mock")]
    Mock {
        id: usize,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    },
}

impl GpuTexture {
    /// Create from real WGPU texture
    pub fn from_wgpu(texture: wgpu::Texture) -> Self {
        Self {
            inner: GpuTextureInner::Real(texture),
        }
    }

    /// Create mock texture (for testing)
    #[cfg(feature = "mock")]
    pub fn mock(id: usize, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self {
            inner: GpuTextureInner::Mock {
                id,
                width,
                height,
                format,
            },
        }
    }

    /// Get the underlying wgpu::Texture (if real)
    ///
    /// # Panics
    /// Panics if this is a mock texture
    pub fn as_wgpu(&self) -> &wgpu::Texture {
        match &self.inner {
            GpuTextureInner::Real(texture) => texture,
            #[cfg(feature = "mock")]
            GpuTextureInner::Mock { .. } => {
                panic!("Attempted to get wgpu::Texture from mock texture")
            }
        }
    }

    /// Check if this is a mock
    #[cfg(feature = "mock")]
    pub fn is_mock(&self) -> bool {
        matches!(self.inner, GpuTextureInner::Mock { .. })
    }

    /// Get mock ID (for test assertions)
    #[cfg(feature = "mock")]
    pub fn mock_id(&self) -> Option<usize> {
        match &self.inner {
            GpuTextureInner::Mock { id, .. } => Some(*id),
            _ => None,
        }
    }
}

/// Wrapper around GPU shader module that can be real or mock.
#[derive(Clone, Debug)]
pub struct GpuShaderModule {
    inner: GpuShaderModuleInner,
}

#[derive(Clone, Debug)]
enum GpuShaderModuleInner {
    Real(wgpu::ShaderModule),
    #[cfg(feature = "mock")]
    Mock { id: usize },
}

impl GpuShaderModule {
    /// Create from real WGPU shader module
    pub fn from_wgpu(module: wgpu::ShaderModule) -> Self {
        Self {
            inner: GpuShaderModuleInner::Real(module),
        }
    }

    /// Create mock shader module (for testing)
    #[cfg(feature = "mock")]
    pub fn mock(id: usize) -> Self {
        Self {
            inner: GpuShaderModuleInner::Mock { id },
        }
    }

    /// Get the underlying wgpu::ShaderModule (if real)
    pub fn as_wgpu(&self) -> &wgpu::ShaderModule {
        match &self.inner {
            GpuShaderModuleInner::Real(module) => module,
            #[cfg(feature = "mock")]
            GpuShaderModuleInner::Mock { .. } => {
                panic!("Attempted to get wgpu::ShaderModule from mock")
            }
        }
    }

    /// Check if this is a mock
    #[cfg(feature = "mock")]
    pub fn is_mock(&self) -> bool {
        matches!(self.inner, GpuShaderModuleInner::Mock { .. })
    }
}

/// Wrapper around GPU render pipeline that can be real or mock.
#[derive(Clone, Debug)]
pub struct GpuRenderPipeline {
    inner: GpuRenderPipelineInner,
}

#[derive(Clone, Debug)]
enum GpuRenderPipelineInner {
    Real(wgpu::RenderPipeline),
    #[cfg(feature = "mock")]
    Mock { id: usize },
}

impl GpuRenderPipeline {
    /// Create from real WGPU render pipeline
    pub fn from_wgpu(pipeline: wgpu::RenderPipeline) -> Self {
        Self {
            inner: GpuRenderPipelineInner::Real(pipeline),
        }
    }

    /// Create mock render pipeline (for testing)
    #[cfg(feature = "mock")]
    pub fn mock(id: usize) -> Self {
        Self {
            inner: GpuRenderPipelineInner::Mock { id },
        }
    }

    /// Get the underlying wgpu::RenderPipeline (if real)
    pub fn as_wgpu(&self) -> &wgpu::RenderPipeline {
        match &self.inner {
            GpuRenderPipelineInner::Real(pipeline) => pipeline,
            #[cfg(feature = "mock")]
            GpuRenderPipelineInner::Mock { .. } => {
                panic!("Attempted to get wgpu::RenderPipeline from mock")
            }
        }
    }

    /// Check if this is a mock
    #[cfg(feature = "mock")]
    pub fn is_mock(&self) -> bool {
        matches!(self.inner, GpuRenderPipelineInner::Mock { .. })
    }
}

/// Wrapper around GPU compute pipeline that can be real or mock.
#[derive(Clone, Debug)]
pub struct GpuComputePipeline {
    inner: GpuComputePipelineInner,
}

#[derive(Clone, Debug)]
enum GpuComputePipelineInner {
    Real(wgpu::ComputePipeline),
    #[cfg(feature = "mock")]
    Mock { id: usize },
}

impl GpuComputePipeline {
    /// Create from real WGPU compute pipeline
    pub fn from_wgpu(pipeline: wgpu::ComputePipeline) -> Self {
        Self {
            inner: GpuComputePipelineInner::Real(pipeline),
        }
    }

    /// Create mock compute pipeline (for testing)
    #[cfg(feature = "mock")]
    pub fn mock(id: usize) -> Self {
        Self {
            inner: GpuComputePipelineInner::Mock { id },
        }
    }

    /// Get the underlying wgpu::ComputePipeline (if real)
    pub fn as_wgpu(&self) -> &wgpu::ComputePipeline {
        match &self.inner {
            GpuComputePipelineInner::Real(pipeline) => pipeline,
            #[cfg(feature = "mock")]
            GpuComputePipelineInner::Mock { .. } => {
                panic!("Attempted to get wgpu::ComputePipeline from mock")
            }
        }
    }

    /// Check if this is a mock
    #[cfg(feature = "mock")]
    pub fn is_mock(&self) -> bool {
        matches!(self.inner, GpuComputePipelineInner::Mock { .. })
    }
}

/// Wrapper around GPU bind group layout that can be real or mock.
#[derive(Clone, Debug)]
pub struct GpuBindGroupLayout {
    inner: GpuBindGroupLayoutInner,
}

#[derive(Clone, Debug)]
enum GpuBindGroupLayoutInner {
    Real(wgpu::BindGroupLayout),
    #[cfg(feature = "mock")]
    Mock { id: usize },
}

impl GpuBindGroupLayout {
    /// Create from real WGPU bind group layout
    pub fn from_wgpu(layout: wgpu::BindGroupLayout) -> Self {
        Self {
            inner: GpuBindGroupLayoutInner::Real(layout),
        }
    }

    /// Create mock bind group layout (for testing)
    #[cfg(feature = "mock")]
    pub fn mock(id: usize) -> Self {
        Self {
            inner: GpuBindGroupLayoutInner::Mock { id },
        }
    }

    /// Get the underlying wgpu::BindGroupLayout (if real)
    pub fn as_wgpu(&self) -> &wgpu::BindGroupLayout {
        match &self.inner {
            GpuBindGroupLayoutInner::Real(layout) => layout,
            #[cfg(feature = "mock")]
            GpuBindGroupLayoutInner::Mock { .. } => {
                panic!("Attempted to get wgpu::BindGroupLayout from mock")
            }
        }
    }

    /// Check if this is a mock
    #[cfg(feature = "mock")]
    pub fn is_mock(&self) -> bool {
        matches!(self.inner, GpuBindGroupLayoutInner::Mock { .. })
    }
}

/// Wrapper around GPU bind group that can be real or mock.
#[derive(Clone, Debug)]
pub struct GpuBindGroup {
    inner: GpuBindGroupInner,
}

#[derive(Clone, Debug)]
enum GpuBindGroupInner {
    Real(wgpu::BindGroup),
    #[cfg(feature = "mock")]
    Mock { id: usize },
}

impl GpuBindGroup {
    /// Create from real WGPU bind group
    pub fn from_wgpu(bind_group: wgpu::BindGroup) -> Self {
        Self {
            inner: GpuBindGroupInner::Real(bind_group),
        }
    }

    /// Create mock bind group (for testing)
    #[cfg(feature = "mock")]
    pub fn mock(id: usize) -> Self {
        Self {
            inner: GpuBindGroupInner::Mock { id },
        }
    }

    /// Get the underlying wgpu::BindGroup (if real)
    pub fn as_wgpu(&self) -> &wgpu::BindGroup {
        match &self.inner {
            GpuBindGroupInner::Real(bind_group) => bind_group,
            #[cfg(feature = "mock")]
            GpuBindGroupInner::Mock { .. } => {
                panic!("Attempted to get wgpu::BindGroup from mock")
            }
        }
    }

    /// Check if this is a mock
    #[cfg(feature = "mock")]
    pub fn is_mock(&self) -> bool {
        matches!(self.inner, GpuBindGroupInner::Mock { .. })
    }
}

/// Wrapper around GPU sampler that can be real or mock.
#[derive(Clone, Debug)]
pub struct GpuSampler {
    inner: GpuSamplerInner,
}

#[derive(Clone, Debug)]
enum GpuSamplerInner {
    Real(wgpu::Sampler),
    #[cfg(feature = "mock")]
    Mock { id: usize },
}

impl GpuSampler {
    /// Create from real WGPU sampler
    pub fn from_wgpu(sampler: wgpu::Sampler) -> Self {
        Self {
            inner: GpuSamplerInner::Real(sampler),
        }
    }

    /// Create mock sampler (for testing)
    #[cfg(feature = "mock")]
    pub fn mock(id: usize) -> Self {
        Self {
            inner: GpuSamplerInner::Mock { id },
        }
    }

    /// Get the underlying wgpu::Sampler (if real)
    pub fn as_wgpu(&self) -> &wgpu::Sampler {
        match &self.inner {
            GpuSamplerInner::Real(sampler) => sampler,
            #[cfg(feature = "mock")]
            GpuSamplerInner::Mock { .. } => {
                panic!("Attempted to get wgpu::Sampler from mock")
            }
        }
    }

    /// Check if this is a mock
    #[cfg(feature = "mock")]
    pub fn is_mock(&self) -> bool {
        matches!(self.inner, GpuSamplerInner::Mock { .. })
    }
}
