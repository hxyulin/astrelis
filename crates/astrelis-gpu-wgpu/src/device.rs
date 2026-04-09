//! wgpu device implementation.

use std::collections::HashMap;
use std::sync::Arc;

use astrelis_gpu::bind_group::{BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor};
use astrelis_gpu::buffer::{BufferDescriptor, BufferInitDescriptor};
use astrelis_gpu::command::{BufferCopyView, TextureCopyView};
use astrelis_gpu::device::{AdapterInfo, GpuDevice};
use astrelis_gpu::error::GpuError;
use astrelis_gpu::id::*;
use astrelis_gpu::pipeline::{
    ComputePipelineDescriptor, PipelineLayoutDescriptor, RenderPipelineDescriptor,
};
use astrelis_gpu::shader::{ShaderModuleDescriptor, ShaderSource};
use astrelis_gpu::texture::{Extent3d, SamplerDescriptor, TextureDescriptor, TextureViewDescriptor};

use crate::convert::{bind_group as conv_bg, buffer as conv_buf, pipeline as conv_pl, texture as conv_tex, types as conv};
use crate::encoder::WgpuCommandEncoder;
use crate::resources::ResourceMap;

fn mipmap_filter_mode(f: astrelis_gpu::types::FilterMode) -> wgpu::MipmapFilterMode {
    match f {
        astrelis_gpu::types::FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
        astrelis_gpu::types::FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
    }
}

/// wgpu-backed GPU device. Holds the underlying `wgpu::Device` and `wgpu::Queue`,
/// plus resource maps for all GPU objects.
pub struct WgpuDevice {
    /// Weak self-reference so `create_command_encoder` can hand the `Arc` to encoders.
    self_ref: std::sync::OnceLock<std::sync::Weak<Self>>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) adapter_info: AdapterInfo,
    pub(crate) buffers: ResourceMap<BufferMarker, wgpu::Buffer>,
    pub(crate) textures: ResourceMap<TextureMarker, wgpu::Texture>,
    pub(crate) texture_views: ResourceMap<TextureViewMarker, wgpu::TextureView>,
    pub(crate) samplers: ResourceMap<SamplerMarker, wgpu::Sampler>,
    pub(crate) shader_modules: ResourceMap<ShaderModuleMarker, wgpu::ShaderModule>,
    pub(crate) bind_group_layouts: ResourceMap<BindGroupLayoutMarker, wgpu::BindGroupLayout>,
    pub(crate) bind_groups: ResourceMap<BindGroupMarker, wgpu::BindGroup>,
    pub(crate) pipeline_layouts: ResourceMap<PipelineLayoutMarker, wgpu::PipelineLayout>,
    pub(crate) render_pipelines: ResourceMap<RenderPipelineMarker, wgpu::RenderPipeline>,
    pub(crate) compute_pipelines: ResourceMap<ComputePipelineMarker, wgpu::ComputePipeline>,
}

impl WgpuDevice {
    pub(crate) fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter_info: AdapterInfo,
    ) -> Arc<Self> {
        let arc = Arc::new(Self {
            self_ref: std::sync::OnceLock::new(),
            device,
            queue,
            adapter_info,
            buffers: ResourceMap::new(),
            textures: ResourceMap::new(),
            texture_views: ResourceMap::new(),
            samplers: ResourceMap::new(),
            shader_modules: ResourceMap::new(),
            bind_group_layouts: ResourceMap::new(),
            bind_groups: ResourceMap::new(),
            pipeline_layouts: ResourceMap::new(),
            render_pipelines: ResourceMap::new(),
            compute_pipelines: ResourceMap::new(),
        });
        let _ = arc.self_ref.set(Arc::downgrade(&arc));
        arc
    }

    /// Returns an `Arc` to this device, upgrading the internal weak reference.
    fn arc_self(&self) -> Arc<Self> {
        self.self_ref
            .get()
            .expect("self_ref not initialized")
            .upgrade()
            .expect("WgpuDevice dropped while still in use")
    }

    /// Returns a reference to the underlying [`wgpu::Device`].
    ///
    /// This is an escape hatch for advanced use cases (e.g., egui integration,
    /// custom compute passes) that need direct access to the raw wgpu device.
    pub fn wgpu_device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Returns a reference to the underlying [`wgpu::Queue`].
    ///
    /// This is an escape hatch for advanced use cases that need direct access
    /// to the raw wgpu queue.
    pub fn wgpu_queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Returns a read guard providing access to raw [`wgpu::TextureView`]s by ID.
    ///
    /// This is an escape hatch for advanced use cases (e.g., egui integration)
    /// that need direct access to wgpu texture views managed by this device.
    pub fn texture_views(&self) -> TextureViewReadGuard<'_> {
        TextureViewReadGuard {
            guard: self.texture_views.read_guard(),
        }
    }
}

/// A read guard providing access to raw [`wgpu::TextureView`]s by their
/// [`TextureViewId`].
///
/// Obtained via [`WgpuDevice::texture_views()`].
pub struct TextureViewReadGuard<'a> {
    guard: std::sync::RwLockReadGuard<'a, HashMap<u64, wgpu::TextureView>>,
}

impl TextureViewReadGuard<'_> {
    /// Looks up a texture view by its ID.
    pub fn get(&self, id: TextureViewId) -> Option<&wgpu::TextureView> {
        self.guard.get(&id.raw())
    }
}

impl GpuDevice for WgpuDevice {
    type Encoder = WgpuCommandEncoder;

    fn adapter_info(&self) -> &AdapterInfo {
        &self.adapter_info
    }

    fn create_buffer(&self, desc: &BufferDescriptor<'_>) -> Result<BufferId, GpuError> {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: desc.label,
            size: desc.size,
            usage: conv_buf::buffer_usages(desc.usage),
            mapped_at_creation: desc.mapped_at_creation,
        });
        Ok(self.buffers.insert(buffer))
    }

    fn create_buffer_init(&self, desc: &BufferInitDescriptor<'_>) -> Result<BufferId, GpuError> {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: desc.label,
            size: desc.contents.len() as u64,
            usage: conv_buf::buffer_usages(desc.usage) | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        buffer.slice(..).get_mapped_range_mut().copy_from_slice(desc.contents);
        buffer.unmap();
        Ok(self.buffers.insert(buffer))
    }

    fn destroy_buffer(&self, id: BufferId) {
        if let Some(buffer) = self.buffers.remove(id) {
            buffer.destroy();
        }
    }

    fn write_buffer(&self, buffer: BufferId, offset: u64, data: &[u8]) {
        self.buffers.get(buffer, |b| {
            self.queue.write_buffer(b, offset, data);
        });
    }

    fn create_texture(&self, desc: &TextureDescriptor<'_>) -> Result<TextureId, GpuError> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: desc.label,
            size: conv_tex::extent3d(desc.size),
            mip_level_count: desc.mip_level_count,
            sample_count: desc.sample_count,
            dimension: conv::texture_dimension(desc.dimension),
            format: conv::texture_format(desc.format),
            usage: conv_tex::texture_usages(desc.usage),
            view_formats: &[],
        });
        Ok(self.textures.insert(texture))
    }

    fn create_texture_view(
        &self,
        texture: TextureId,
        desc: &TextureViewDescriptor<'_>,
    ) -> Result<TextureViewId, GpuError> {
        let view_id = self
            .textures
            .get(texture, |tex| {
                let view = tex.create_view(&wgpu::TextureViewDescriptor {
                    label: desc.label,
                    format: desc.format.map(conv::texture_format),
                    dimension: desc.dimension.map(conv::texture_view_dimension),
                    base_mip_level: desc.base_mip_level,
                    mip_level_count: desc.mip_level_count,
                    base_array_layer: desc.base_array_layer,
                    array_layer_count: desc.array_layer_count,
                    ..Default::default()
                });
                self.texture_views.insert(view)
            })
            .ok_or_else(|| GpuError::InvalidHandle("invalid texture handle".into()))?;
        Ok(view_id)
    }

    fn create_sampler(&self, desc: &SamplerDescriptor<'_>) -> Result<SamplerId, GpuError> {
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: desc.label,
            address_mode_u: conv::address_mode(desc.address_mode_u),
            address_mode_v: conv::address_mode(desc.address_mode_v),
            address_mode_w: conv::address_mode(desc.address_mode_w),
            mag_filter: conv::filter_mode(desc.mag_filter),
            min_filter: conv::filter_mode(desc.min_filter),
            mipmap_filter: mipmap_filter_mode(desc.mipmap_filter),
            lod_min_clamp: desc.lod_min_clamp,
            lod_max_clamp: desc.lod_max_clamp,
            compare: desc.compare.map(conv::compare_function),
            anisotropy_clamp: desc.anisotropy_clamp,
            ..Default::default()
        });
        Ok(self.samplers.insert(sampler))
    }

    fn destroy_texture(&self, id: TextureId) {
        if let Some(texture) = self.textures.remove(id) {
            texture.destroy();
        }
    }

    fn destroy_texture_view(&self, id: TextureViewId) {
        self.texture_views.remove(id);
    }

    fn destroy_sampler(&self, id: SamplerId) {
        self.samplers.remove(id);
    }

    fn write_texture(
        &self,
        dst: TextureCopyView,
        data: &[u8],
        layout: BufferCopyView,
        size: Extent3d,
    ) {
        self.textures.get(dst.texture, |tex| {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: tex,
                    mip_level: dst.mip_level,
                    origin: wgpu::Origin3d {
                        x: dst.origin.x,
                        y: dst.origin.y,
                        z: dst.origin.z,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: layout.offset,
                    bytes_per_row: layout.bytes_per_row,
                    rows_per_image: layout.rows_per_image,
                },
                conv_tex::extent3d(size),
            );
        });
    }

    fn create_shader_module(
        &self,
        desc: &ShaderModuleDescriptor<'_>,
    ) -> Result<ShaderModuleId, GpuError> {
        let source = match desc.source {
            ShaderSource::Wgsl(src) => wgpu::ShaderSource::Wgsl(src.into()),
            ShaderSource::SpirV(_) => {
                return Err(GpuError::ShaderError(
                    "SPIR-V shaders are not supported by the wgpu backend".into(),
                ));
            }
        };
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: desc.label,
                source,
            });
        Ok(self.shader_modules.insert(module))
    }

    fn destroy_shader_module(&self, id: ShaderModuleId) {
        self.shader_modules.remove(id);
    }

    fn create_bind_group_layout(
        &self,
        desc: &BindGroupLayoutDescriptor<'_>,
    ) -> Result<BindGroupLayoutId, GpuError> {
        let entries: Vec<wgpu::BindGroupLayoutEntry> = desc
            .entries
            .iter()
            .map(|e| wgpu::BindGroupLayoutEntry {
                binding: e.binding,
                visibility: conv_bg::shader_stages(e.visibility),
                ty: conv_bg::binding_type(&e.ty),
                count: e.count.and_then(std::num::NonZeroU32::new),
            })
            .collect();
        let layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: desc.label,
                entries: &entries,
            });
        Ok(self.bind_group_layouts.insert(layout))
    }

    fn create_bind_group(
        &self,
        desc: &BindGroupDescriptor<'_>,
    ) -> Result<BindGroupId, GpuError> {
        let buffers_guard = self.buffers.read_guard();
        let views_guard = self.texture_views.read_guard();
        let samplers_guard = self.samplers.read_guard();

        let wgpu_entries: Vec<wgpu::BindGroupEntry<'_>> = desc
            .entries
            .iter()
            .map(|entry| match entry {
                BindGroupEntry::Buffer {
                    binding,
                    buffer,
                    offset,
                    size,
                } => {
                    let buf = buffers_guard
                        .get(&buffer.raw())
                        .expect("invalid buffer handle in bind group entry");
                    wgpu::BindGroupEntry {
                        binding: *binding,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: buf,
                            offset: *offset,
                            size: size.and_then(std::num::NonZeroU64::new),
                        }),
                    }
                }
                BindGroupEntry::TextureView { binding, view } => {
                    let tv = views_guard
                        .get(&view.raw())
                        .expect("invalid texture view handle in bind group entry");
                    wgpu::BindGroupEntry {
                        binding: *binding,
                        resource: wgpu::BindingResource::TextureView(tv),
                    }
                }
                BindGroupEntry::Sampler { binding, sampler } => {
                    let s = samplers_guard
                        .get(&sampler.raw())
                        .expect("invalid sampler handle in bind group entry");
                    wgpu::BindGroupEntry {
                        binding: *binding,
                        resource: wgpu::BindingResource::Sampler(s),
                    }
                }
            })
            .collect();

        let layout_guard = self.bind_group_layouts.read_guard();
        let layout = layout_guard
            .get(&desc.layout.raw())
            .expect("invalid bind group layout handle");

        let bind_group = self
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: desc.label,
                layout,
                entries: &wgpu_entries,
            });
        Ok(self.bind_groups.insert(bind_group))
    }

    fn destroy_bind_group_layout(&self, id: BindGroupLayoutId) {
        self.bind_group_layouts.remove(id);
    }

    fn destroy_bind_group(&self, id: BindGroupId) {
        self.bind_groups.remove(id);
    }

    fn create_pipeline_layout(
        &self,
        desc: &PipelineLayoutDescriptor<'_>,
    ) -> Result<PipelineLayoutId, GpuError> {
        let layouts_guard = self.bind_group_layouts.read_guard();
        let bind_group_layouts: Vec<&wgpu::BindGroupLayout> = desc
            .bind_group_layouts
            .iter()
            .map(|id| {
                layouts_guard
                    .get(&id.raw())
                    .expect("invalid bind group layout handle")
            })
            .collect();

        let bind_group_layout_opts: Vec<Option<&wgpu::BindGroupLayout>> =
            bind_group_layouts.into_iter().map(Some).collect();

        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: desc.label,
                bind_group_layouts: &bind_group_layout_opts,
                ..Default::default()
            });
        Ok(self.pipeline_layouts.insert(layout))
    }

    fn create_render_pipeline(
        &self,
        desc: &RenderPipelineDescriptor<'_>,
    ) -> Result<RenderPipelineId, GpuError> {
        let layouts_guard = self.pipeline_layouts.read_guard();
        let shaders_guard = self.shader_modules.read_guard();

        let layout = desc
            .layout
            .map(|id| {
                layouts_guard
                    .get(&id.raw())
                    .expect("invalid pipeline layout handle")
            });

        let vertex_module = shaders_guard
            .get(&desc.vertex.module.raw())
            .expect("invalid vertex shader module handle");

        let vertex_attrs: Vec<Vec<wgpu::VertexAttribute>> = desc
            .vertex
            .buffers
            .iter()
            .map(|buf| buf.attributes.iter().map(conv_pl::vertex_attribute).collect())
            .collect();

        let vertex_buffer_layouts: Vec<wgpu::VertexBufferLayout<'_>> = desc
            .vertex
            .buffers
            .iter()
            .zip(vertex_attrs.iter())
            .map(|(buf, attrs)| wgpu::VertexBufferLayout {
                array_stride: buf.array_stride,
                step_mode: conv::vertex_step_mode(buf.step_mode),
                attributes: attrs,
            })
            .collect();

        let color_targets: Vec<Option<wgpu::ColorTargetState>> = desc
            .fragment
            .as_ref()
            .map(|f| f.targets.iter().map(|t| Some(conv_pl::color_target_state(t))).collect())
            .unwrap_or_default();

        let fragment_state = desc.fragment.as_ref().map(|f| {
            let module = shaders_guard
                .get(&f.module.raw())
                .expect("invalid fragment shader module handle");
            wgpu::FragmentState {
                module,
                entry_point: Some(f.entry_point),
                targets: &color_targets,
                compilation_options: Default::default(),
            }
        });

        let pipeline =
            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: desc.label,
                    layout,
                    vertex: wgpu::VertexState {
                        module: vertex_module,
                        entry_point: Some(desc.vertex.entry_point),
                        buffers: &vertex_buffer_layouts,
                        compilation_options: Default::default(),
                    },
                    primitive: conv_pl::primitive_state(&desc.primitive),
                    depth_stencil: desc.depth_stencil.as_ref().map(conv_pl::depth_stencil_state),
                    multisample: conv_pl::multisample_state(&desc.multisample),
                    fragment: fragment_state,
                    multiview_mask: None,
                    cache: None,
                });
        Ok(self.render_pipelines.insert(pipeline))
    }

    fn create_compute_pipeline(
        &self,
        desc: &ComputePipelineDescriptor<'_>,
    ) -> Result<ComputePipelineId, GpuError> {
        let layouts_guard = self.pipeline_layouts.read_guard();
        let shaders_guard = self.shader_modules.read_guard();

        let layout = desc
            .layout
            .map(|id| {
                layouts_guard
                    .get(&id.raw())
                    .expect("invalid pipeline layout handle")
            });

        let module = shaders_guard
            .get(&desc.module.raw())
            .expect("invalid compute shader module handle");

        let pipeline =
            self.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: desc.label,
                    layout,
                    module,
                    entry_point: Some(desc.entry_point),
                    compilation_options: Default::default(),
                    cache: None,
                });
        Ok(self.compute_pipelines.insert(pipeline))
    }

    fn destroy_pipeline_layout(&self, id: PipelineLayoutId) {
        self.pipeline_layouts.remove(id);
    }

    fn destroy_render_pipeline(&self, id: RenderPipelineId) {
        self.render_pipelines.remove(id);
    }

    fn destroy_compute_pipeline(&self, id: ComputePipelineId) {
        self.compute_pipelines.remove(id);
    }

    fn create_command_encoder(&self, label: Option<&str>) -> Self::Encoder {
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label });
        WgpuCommandEncoder::new(encoder, self.arc_self())
    }
}
