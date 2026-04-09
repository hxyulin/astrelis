//! wgpu command encoder implementation.

use std::sync::Arc;

use astrelis_gpu::command::{
    BufferCopyView, CommandEncoder, RenderPassDescriptor, TextureCopyView,
};
use astrelis_gpu::id::BufferId;
use astrelis_gpu::texture::Extent3d;

use crate::compute_pass::WgpuComputePass;
use crate::convert::texture as conv_tex;
use crate::device::WgpuDevice;
use crate::render_pass::WgpuRenderPass;

/// wgpu-backed command encoder.
pub struct WgpuCommandEncoder {
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub(crate) device: Arc<WgpuDevice>,
}

impl WgpuCommandEncoder {
    pub(crate) fn new(encoder: wgpu::CommandEncoder, device: Arc<WgpuDevice>) -> Self {
        Self {
            encoder: Some(encoder),
            device,
        }
    }

    /// Consumes the encoder and returns the finished command buffer.
    pub(crate) fn finish(mut self) -> wgpu::CommandBuffer {
        self.encoder
            .take()
            .expect("encoder already consumed")
            .finish()
    }
}

impl CommandEncoder for WgpuCommandEncoder {
    type RenderPass<'pass> = WgpuRenderPass<'pass> where Self: 'pass;
    type ComputePass<'pass> = WgpuComputePass<'pass> where Self: 'pass;

    fn begin_render_pass<'pass>(
        &'pass mut self,
        desc: &RenderPassDescriptor<'_>,
    ) -> Self::RenderPass<'pass> {
        let encoder = self.encoder.as_mut().expect("encoder already consumed");
        WgpuRenderPass::new(encoder, &self.device, desc)
    }

    fn begin_compute_pass<'pass>(
        &'pass mut self,
        label: Option<&str>,
    ) -> Self::ComputePass<'pass> {
        let encoder = self.encoder.as_mut().expect("encoder already consumed");
        WgpuComputePass::new(encoder, &self.device, label)
    }

    fn copy_buffer_to_buffer(
        &mut self,
        src: BufferId,
        src_offset: u64,
        dst: BufferId,
        dst_offset: u64,
        size: u64,
    ) {
        let buffers = self.device.buffers.read_guard();
        let src_buf = buffers
            .get(&src.raw())
            .expect("invalid source buffer handle");
        let dst_buf = buffers
            .get(&dst.raw())
            .expect("invalid destination buffer handle");
        self.encoder
            .as_mut()
            .expect("encoder already consumed")
            .copy_buffer_to_buffer(src_buf, src_offset, dst_buf, dst_offset, size);
    }

    fn copy_buffer_to_texture(
        &mut self,
        src: BufferCopyView,
        dst: TextureCopyView,
        size: Extent3d,
    ) {
        let buffers = self.device.buffers.read_guard();
        let textures = self.device.textures.read_guard();
        let src_buf = buffers
            .get(&src.buffer.raw())
            .expect("invalid source buffer handle");
        let dst_tex = textures
            .get(&dst.texture.raw())
            .expect("invalid destination texture handle");
        self.encoder
            .as_mut()
            .expect("encoder already consumed")
            .copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: src_buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: src.offset,
                        bytes_per_row: src.bytes_per_row,
                        rows_per_image: src.rows_per_image,
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: dst_tex,
                    mip_level: dst.mip_level,
                    origin: wgpu::Origin3d {
                        x: dst.origin.x,
                        y: dst.origin.y,
                        z: dst.origin.z,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                conv_tex::extent3d(size),
            );
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: TextureCopyView,
        dst: BufferCopyView,
        size: Extent3d,
    ) {
        let textures = self.device.textures.read_guard();
        let buffers = self.device.buffers.read_guard();
        let src_tex = textures
            .get(&src.texture.raw())
            .expect("invalid source texture handle");
        let dst_buf = buffers
            .get(&dst.buffer.raw())
            .expect("invalid destination buffer handle");
        self.encoder
            .as_mut()
            .expect("encoder already consumed")
            .copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: src_tex,
                    mip_level: src.mip_level,
                    origin: wgpu::Origin3d {
                        x: src.origin.x,
                        y: src.origin.y,
                        z: src.origin.z,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: dst_buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: dst.offset,
                        bytes_per_row: dst.bytes_per_row,
                        rows_per_image: dst.rows_per_image,
                    },
                },
                conv_tex::extent3d(size),
            );
    }

    fn copy_texture_to_texture(
        &mut self,
        src: TextureCopyView,
        dst: TextureCopyView,
        size: Extent3d,
    ) {
        let textures = self.device.textures.read_guard();
        let src_tex = textures
            .get(&src.texture.raw())
            .expect("invalid source texture handle");
        let dst_tex = textures
            .get(&dst.texture.raw())
            .expect("invalid destination texture handle");
        self.encoder
            .as_mut()
            .expect("encoder already consumed")
            .copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: src_tex,
                    mip_level: src.mip_level,
                    origin: wgpu::Origin3d {
                        x: src.origin.x,
                        y: src.origin.y,
                        z: src.origin.z,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: dst_tex,
                    mip_level: dst.mip_level,
                    origin: wgpu::Origin3d {
                        x: dst.origin.x,
                        y: dst.origin.y,
                        z: dst.origin.z,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                conv_tex::extent3d(size),
            );
    }
}
