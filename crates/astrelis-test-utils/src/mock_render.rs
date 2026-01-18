//! Mock implementation of RenderContext for testing.
//!
//! This module provides a mock GPU context that records operations
//! without actually interacting with the GPU.

use crate::{gpu_types::*, render_context::RenderContext};
use parking_lot::Mutex;
use wgpu::*;

/// Records a GPU operation call for verification in tests.
#[derive(Debug, Clone)]
pub enum RenderCall {
    CreateBuffer {
        size: u64,
        usage: BufferUsages,
    },
    WriteBuffer {
        buffer_id: usize,
        offset: u64,
        size: usize,
    },
    CreateTexture {
        width: u32,
        height: u32,
        format: TextureFormat,
    },
    CreateShaderModule {
        label: Option<String>,
    },
    CreateRenderPipeline {
        label: Option<String>,
    },
    CreateComputePipeline {
        label: Option<String>,
    },
    CreateBindGroupLayout {
        label: Option<String>,
    },
    CreateBindGroup {
        label: Option<String>,
    },
    CreateSampler {
        label: Option<String>,
    },
}

/// Mock buffers stored in the context.
#[derive(Debug, Clone)]
struct MockBuffer {
    id: usize,
    size: u64,
    usage: BufferUsages,
}

/// Mock textures stored in the context.
#[derive(Debug, Clone)]
struct MockTexture {
    id: usize,
    width: u32,
    height: u32,
    format: TextureFormat,
}

/// Mock implementation of RenderContext for testing.
///
/// # Borrow Checking Pattern: Interior Mutability
///
/// Methods take `&self` but need to mutate internal state (record calls).
/// Solution: Use `Mutex<Vec<RenderCall>>` for interior mutability.
///
/// ## Why Mutex instead of RefCell?
/// - `Mutex` is `Send + Sync` (required for RenderContext trait)
/// - `RefCell` is `!Sync`, so can't be used in multi-threaded contexts
/// - `parking_lot::Mutex` has less overhead than `std::sync::Mutex`
///
/// # Example
///
/// ```rust
/// use astrelis_test_utils::{MockRenderContext, RenderContext};
/// use wgpu::*;
///
/// let mock = MockRenderContext::new();
///
/// // Create a buffer (mock)
/// let buffer = mock.create_buffer(&BufferDescriptor {
///     label: None,
///     size: 1024,
///     usage: BufferUsages::VERTEX,
///     mapped_at_creation: false,
/// });
///
/// // Verify it was mocked
/// assert!(buffer.is_mock());
///
/// // Check recorded calls
/// assert_eq!(mock.count_buffer_creates(), 1);
/// ```
pub struct MockRenderContext {
    /// Recorded calls for verification
    calls: Mutex<Vec<RenderCall>>,

    /// Mock buffers (we don't create real GPU buffers)
    buffers: Mutex<Vec<MockBuffer>>,

    /// Mock textures
    textures: Mutex<Vec<MockTexture>>,

    /// Counters for generating IDs
    next_shader_id: Mutex<usize>,
    next_pipeline_id: Mutex<usize>,
    next_bind_group_layout_id: Mutex<usize>,
    next_bind_group_id: Mutex<usize>,
    next_sampler_id: Mutex<usize>,
}

impl MockRenderContext {
    /// Create a new mock render context.
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            buffers: Mutex::new(Vec::new()),
            textures: Mutex::new(Vec::new()),
            next_shader_id: Mutex::new(0),
            next_pipeline_id: Mutex::new(0),
            next_bind_group_layout_id: Mutex::new(0),
            next_bind_group_id: Mutex::new(0),
            next_sampler_id: Mutex::new(0),
        }
    }

    /// Get a copy of all recorded calls (for test assertions).
    pub fn calls(&self) -> Vec<RenderCall> {
        self.calls.lock().clone()
    }

    /// Count calls of a specific type.
    pub fn count_buffer_creates(&self) -> usize {
        self.calls
            .lock()
            .iter()
            .filter(|call| matches!(call, RenderCall::CreateBuffer { .. }))
            .count()
    }

    /// Count buffer write operations.
    pub fn count_buffer_writes(&self) -> usize {
        self.calls
            .lock()
            .iter()
            .filter(|call| matches!(call, RenderCall::WriteBuffer { .. }))
            .count()
    }

    /// Count texture creates.
    pub fn count_texture_creates(&self) -> usize {
        self.calls
            .lock()
            .iter()
            .filter(|call| matches!(call, RenderCall::CreateTexture { .. }))
            .count()
    }

    /// Count shader module creates.
    pub fn count_shader_creates(&self) -> usize {
        self.calls
            .lock()
            .iter()
            .filter(|call| matches!(call, RenderCall::CreateShaderModule { .. }))
            .count()
    }

    /// Count render pipeline creates.
    pub fn count_render_pipeline_creates(&self) -> usize {
        self.calls
            .lock()
            .iter()
            .filter(|call| matches!(call, RenderCall::CreateRenderPipeline { .. }))
            .count()
    }

    /// Clear recorded calls (useful between test steps).
    pub fn clear_calls(&self) {
        self.calls.lock().clear();
    }

    /// Get total number of recorded calls.
    pub fn call_count(&self) -> usize {
        self.calls.lock().len()
    }
}

impl Default for MockRenderContext {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderContext for MockRenderContext {
    fn create_buffer(&self, desc: &BufferDescriptor) -> GpuBuffer {
        let mut buffers = self.buffers.lock();
        let id = buffers.len();

        buffers.push(MockBuffer {
            id,
            size: desc.size,
            usage: desc.usage,
        });

        self.calls.lock().push(RenderCall::CreateBuffer {
            size: desc.size,
            usage: desc.usage,
        });

        GpuBuffer::mock(id, desc.size)
    }

    fn write_buffer(&self, buffer: &GpuBuffer, offset: u64, data: &[u8]) {
        if let Some(buffer_id) = buffer.mock_id() {
            self.calls.lock().push(RenderCall::WriteBuffer {
                buffer_id,
                offset,
                size: data.len(),
            });
        }
    }

    fn create_texture(&self, desc: &TextureDescriptor) -> GpuTexture {
        let mut textures = self.textures.lock();
        let id = textures.len();

        textures.push(MockTexture {
            id,
            width: desc.size.width,
            height: desc.size.height,
            format: desc.format,
        });

        self.calls.lock().push(RenderCall::CreateTexture {
            width: desc.size.width,
            height: desc.size.height,
            format: desc.format,
        });

        GpuTexture::mock(id, desc.size.width, desc.size.height, desc.format)
    }


    fn create_shader_module(&self, desc: &ShaderModuleDescriptor) -> GpuShaderModule {
        let mut id = self.next_shader_id.lock();
        let shader_id = *id;
        *id += 1;

        self.calls.lock().push(RenderCall::CreateShaderModule {
            label: desc.label.map(|s| s.to_string()),
        });

        GpuShaderModule::mock(shader_id)
    }

    fn create_render_pipeline(&self, desc: &RenderPipelineDescriptor) -> GpuRenderPipeline {
        let mut id = self.next_pipeline_id.lock();
        let pipeline_id = *id;
        *id += 1;

        self.calls
            .lock()
            .push(RenderCall::CreateRenderPipeline {
                label: desc.label.map(|s| s.to_string()),
            });

        GpuRenderPipeline::mock(pipeline_id)
    }

    fn create_compute_pipeline(&self, desc: &ComputePipelineDescriptor) -> GpuComputePipeline {
        let mut id = self.next_pipeline_id.lock();
        let pipeline_id = *id;
        *id += 1;

        self.calls
            .lock()
            .push(RenderCall::CreateComputePipeline {
                label: desc.label.map(|s| s.to_string()),
            });

        GpuComputePipeline::mock(pipeline_id)
    }

    fn create_bind_group_layout(&self, desc: &BindGroupLayoutDescriptor) -> GpuBindGroupLayout {
        let mut id = self.next_bind_group_layout_id.lock();
        let layout_id = *id;
        *id += 1;

        self.calls
            .lock()
            .push(RenderCall::CreateBindGroupLayout {
                label: desc.label.map(|s| s.to_string()),
            });

        GpuBindGroupLayout::mock(layout_id)
    }

    fn create_bind_group(&self, desc: &BindGroupDescriptor) -> GpuBindGroup {
        let mut id = self.next_bind_group_id.lock();
        let bind_group_id = *id;
        *id += 1;

        self.calls.lock().push(RenderCall::CreateBindGroup {
            label: desc.label.map(|s| s.to_string()),
        });

        GpuBindGroup::mock(bind_group_id)
    }

    fn create_sampler(&self, desc: &SamplerDescriptor) -> GpuSampler {
        let mut id = self.next_sampler_id.lock();
        let sampler_id = *id;
        *id += 1;

        self.calls.lock().push(RenderCall::CreateSampler {
            label: desc.label.map(|s| s.to_string()),
        });

        GpuSampler::mock(sampler_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_buffer_creation() {
        let mock = MockRenderContext::new();

        let buffer = mock.create_buffer(&BufferDescriptor {
            label: Some("test_buffer"),
            size: 1024,
            usage: BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        assert!(buffer.is_mock());
        assert_eq!(mock.count_buffer_creates(), 1);
    }

    #[test]
    fn test_mock_buffer_write() {
        let mock = MockRenderContext::new();

        let buffer = mock.create_buffer(&BufferDescriptor {
            label: None,
            size: 1024,
            usage: BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let data = vec![0u8; 256];
        mock.write_buffer(&buffer, 0, &data);

        assert_eq!(mock.count_buffer_writes(), 1);
    }

    #[test]
    fn test_mock_texture_creation() {
        let mock = MockRenderContext::new();

        let texture = mock.create_texture(&TextureDescriptor {
            label: Some("test_texture"),
            size: Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        assert!(texture.is_mock());
        assert_eq!(mock.count_texture_creates(), 1);
    }

    #[test]
    fn test_clear_calls() {
        let mock = MockRenderContext::new();

        mock.create_buffer(&BufferDescriptor {
            label: None,
            size: 1024,
            usage: BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        assert_eq!(mock.call_count(), 1);

        mock.clear_calls();
        assert_eq!(mock.call_count(), 0);
    }
}
