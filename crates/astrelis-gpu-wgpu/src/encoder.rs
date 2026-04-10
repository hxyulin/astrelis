//! wgpu command encoder implementation.

use std::sync::Arc;

use astrelis_gpu::command::{
    BufferCopyView, CommandEncoder, RenderPassDescriptor, TextureCopyView,
};
use astrelis_gpu::id::BufferId;
use astrelis_gpu::profiling::GpuProfilingTier;
use astrelis_gpu::texture::Extent3d;

use crate::compute_pass::WgpuComputePass;
use crate::convert::texture as conv_tex;
use crate::device::WgpuDevice;
use crate::profiling::query_pool::TimestampPair;
use crate::render_pass::WgpuRenderPass;

/// wgpu-backed command encoder.
pub struct WgpuCommandEncoder {
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub(crate) device: Arc<WgpuDevice>,
    /// The most recent scope whose end timestamp hasn't been written yet.
    /// When a new scope begins or the encoder finishes, we write the end
    /// timestamp for this scope first.
    open_scope: Option<TimestampPair>,
}

impl WgpuCommandEncoder {
    pub(crate) fn new(encoder: wgpu::CommandEncoder, device: Arc<WgpuDevice>) -> Self {
        Self {
            encoder: Some(encoder),
            device,
            open_scope: None,
        }
    }

    /// Writes the end timestamp for the currently open scope, if any.
    ///
    /// Must be called when the previous pass has been dropped (so the encoder
    /// is no longer borrowed by a pass).
    fn close_open_scope(&mut self) {
        if let Some(pair) = self.open_scope.take() {
            let profiler = self.device.gpu_profiler.lock().unwrap();
            if profiler.tier() >= GpuProfilingTier::Encoder
                && let Some(query_set) = profiler.active_query_set()
            {
                let encoder = self.encoder.as_mut().expect("encoder already consumed");
                encoder.write_timestamp(query_set, pair.end_index);
            }
        }
    }

    /// Consumes the encoder and returns the finished command buffer.
    ///
    /// Writes the end timestamp for the last open scope, then resolves
    /// all queries before finishing.
    pub(crate) fn finish(mut self) -> wgpu::CommandBuffer {
        astrelis_profiling::profile_function!();
        // Close the last open scope.
        self.close_open_scope();

        let mut encoder = self.encoder.take().expect("encoder already consumed");

        // Resolve all queries in the active pool.
        {
            let mut profiler = self.device.gpu_profiler.lock().unwrap();
            profiler.resolve_frame(&mut encoder);
        }

        encoder.finish()
    }
}

impl CommandEncoder for WgpuCommandEncoder {
    type RenderPass<'pass> = WgpuRenderPass<'pass> where Self: 'pass;
    type ComputePass<'pass> = WgpuComputePass<'pass> where Self: 'pass;

    fn begin_render_pass<'pass>(
        &'pass mut self,
        desc: &RenderPassDescriptor<'_>,
    ) -> Self::RenderPass<'pass> {
        astrelis_profiling::profile_function!();

        // Close the previous scope's end timestamp before starting a new one.
        // This is safe because the previous pass must have been dropped for
        // the caller to borrow the encoder again.
        self.close_open_scope();

        let encoder = self.encoder.as_mut().expect("encoder already consumed");
        let label = desc.label.unwrap_or("render_pass");

        // For Tier 2+: write start timestamp on the encoder before creating the pass.
        let mut profiler = self.device.gpu_profiler.lock().unwrap();
        let tier = profiler.tier();
        if tier >= GpuProfilingTier::Encoder {
            if let Some(pair) = profiler.begin_scope(label) {
                if let Some(query_set) = profiler.active_query_set() {
                    encoder.write_timestamp(query_set, pair.start_index);
                }
                self.open_scope = Some(pair);
            }
        } else if tier == GpuProfilingTier::Basic {
            // For Tier 1: use pass descriptor timestamp_writes (handled in render_pass.rs).
            profiler.begin_scope(label);
        }
        drop(profiler);

        WgpuRenderPass::new(encoder, &self.device, desc)
    }

    fn begin_compute_pass<'pass>(
        &'pass mut self,
        label: Option<&str>,
    ) -> Self::ComputePass<'pass> {
        astrelis_profiling::profile_function!();

        // Close the previous scope's end timestamp.
        self.close_open_scope();

        let encoder = self.encoder.as_mut().expect("encoder already consumed");
        let scope_label = label.unwrap_or("compute_pass");

        // For Tier 2+: write start timestamp on the encoder before creating the pass.
        let mut profiler = self.device.gpu_profiler.lock().unwrap();
        let tier = profiler.tier();
        if tier >= GpuProfilingTier::Encoder {
            if let Some(pair) = profiler.begin_scope(scope_label) {
                if let Some(query_set) = profiler.active_query_set() {
                    encoder.write_timestamp(query_set, pair.start_index);
                }
                self.open_scope = Some(pair);
            }
        } else if tier == GpuProfilingTier::Basic {
            profiler.begin_scope(scope_label);
        }
        drop(profiler);

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
        astrelis_profiling::profile_function!();
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
        astrelis_profiling::profile_function!();
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
        astrelis_profiling::profile_function!();
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
        astrelis_profiling::profile_function!();
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
