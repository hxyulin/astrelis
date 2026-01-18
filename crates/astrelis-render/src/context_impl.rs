// ! Implementation of RenderContext trait for GraphicsContext.
//!
//! This allows GraphicsContext to be used polymorphically with the
//! RenderContext trait, enabling testing with MockRenderContext.

use crate::context::GraphicsContext;
use astrelis_test_utils::{
    GpuBindGroup, GpuBindGroupLayout, GpuBuffer, GpuComputePipeline, GpuRenderPipeline,
    GpuSampler, GpuShaderModule, GpuTexture, RenderContext,
};
use wgpu::{
    BindGroupDescriptor, BindGroupLayoutDescriptor, BufferDescriptor, ComputePipelineDescriptor,
    RenderPipelineDescriptor, SamplerDescriptor, ShaderModuleDescriptor, TextureDescriptor,
};

impl RenderContext for GraphicsContext {
    fn create_buffer(&self, desc: &BufferDescriptor) -> GpuBuffer {
        let buffer = self.device.create_buffer(desc);
        GpuBuffer::from_wgpu(buffer)
    }

    fn write_buffer(&self, buffer: &GpuBuffer, offset: u64, data: &[u8]) {
        let wgpu_buffer = buffer.as_wgpu();
        self.queue.write_buffer(wgpu_buffer, offset, data);
    }

    fn create_texture(&self, desc: &TextureDescriptor) -> GpuTexture {
        let texture = self.device.create_texture(desc);
        GpuTexture::from_wgpu(texture)
    }

    fn create_shader_module(&self, desc: &ShaderModuleDescriptor) -> GpuShaderModule {
        let module = self.device.create_shader_module(desc.clone());
        GpuShaderModule::from_wgpu(module)
    }

    fn create_render_pipeline(&self, desc: &RenderPipelineDescriptor) -> GpuRenderPipeline {
        let pipeline = self.device.create_render_pipeline(desc);
        GpuRenderPipeline::from_wgpu(pipeline)
    }

    fn create_compute_pipeline(&self, desc: &ComputePipelineDescriptor) -> GpuComputePipeline {
        let pipeline = self.device.create_compute_pipeline(desc);
        GpuComputePipeline::from_wgpu(pipeline)
    }

    fn create_bind_group_layout(&self, desc: &BindGroupLayoutDescriptor) -> GpuBindGroupLayout {
        let layout = self.device.create_bind_group_layout(desc);
        GpuBindGroupLayout::from_wgpu(layout)
    }

    fn create_bind_group(&self, desc: &BindGroupDescriptor) -> GpuBindGroup {
        let bind_group = self.device.create_bind_group(desc);
        GpuBindGroup::from_wgpu(bind_group)
    }

    fn create_sampler(&self, desc: &SamplerDescriptor) -> GpuSampler {
        let sampler = self.device.create_sampler(desc);
        GpuSampler::from_wgpu(sampler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "mock")]
    use astrelis_test_utils::MockRenderContext;

    #[test]
    #[cfg(feature = "mock")]
    fn test_render_context_trait_object() {
        // Test that we can use both GraphicsContext and MockRenderContext
        // polymorphically through the RenderContext trait

        let mock_ctx = MockRenderContext::new();

        fn uses_render_context(ctx: &dyn RenderContext) {
            let buffer = ctx.create_buffer(&BufferDescriptor {
                label: Some("Test Buffer"),
                size: 256,
                usage: wgpu::BufferUsages::UNIFORM,
                mapped_at_creation: false,
            });

            ctx.write_buffer(&buffer, 0, &[0u8; 256]);
        }

        // Should work with mock context
        uses_render_context(&mock_ctx);

        // Verify the mock recorded the calls
        let calls = mock_ctx.calls();
        assert_eq!(calls.len(), 2); // create_buffer + write_buffer
    }
}
