//! GPU device — creates and manages GPU resources.

use crate::bind_group::{BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor};
use crate::buffer::{BufferDescriptor, BufferInitDescriptor};
use crate::command::{CommandEncoder, TextureCopyView};
use crate::convert::{bind_group as conv_bg, buffer as conv_buf, pipeline as conv_pl, texture as conv_tex, types as conv};
use crate::error::GpuError;
use crate::pipeline::{
    ComputePipelineDescriptor, PipelineLayoutDescriptor, RenderPipelineDescriptor,
};
use crate::resources::*;
use crate::shader::{ShaderModuleDescriptor, ShaderSource};
use crate::texture::{Extent3d, SamplerDescriptor, TextureDescriptor, TextureViewDescriptor};

fn mipmap_filter_mode(f: crate::types::FilterMode) -> wgpu::MipmapFilterMode {
    match f {
        crate::types::FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
        crate::types::FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
    }
}

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

/// The concrete GPU device. Creates and manages GPU resources.
///
/// All resource creation methods return newtype wrappers that own the
/// underlying wgpu resource. Dropping the wrapper releases the resource.
pub struct GpuDevice {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) adapter_info: AdapterInfo,
    pub(crate) gpu_profiler: std::sync::Mutex<wgpu_profiler::GpuProfiler>,
    /// Last submission index from `queue.submit()`, used to poll for
    /// completion before reading profiler results.
    pub(crate) last_submission: std::sync::Mutex<Option<wgpu::SubmissionIndex>>,
}

impl GpuDevice {
    pub(crate) fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter_info: AdapterInfo,
        gpu_profiler: wgpu_profiler::GpuProfiler,
    ) -> Self {
        astrelis_profiling::profile_function!();
        Self {
            device,
            queue,
            adapter_info,
            gpu_profiler: std::sync::Mutex::new(gpu_profiler),
            last_submission: std::sync::Mutex::new(None),
        }
    }

    /// Returns information about the GPU adapter.
    pub fn adapter_info(&self) -> &AdapterInfo {
        &self.adapter_info
    }

    /// Returns a reference to the underlying [`wgpu::Device`].
    ///
    /// This is an escape hatch for advanced use cases (e.g., egui integration,
    /// custom compute passes) that need direct access to the raw wgpu device.
    pub fn raw_device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Returns a reference to the underlying [`wgpu::Queue`].
    ///
    /// This is an escape hatch for advanced use cases that need direct access
    /// to the raw wgpu queue.
    pub fn raw_queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Processes finished GPU profiling frames and forwards results
    /// to [`astrelis_profiling::gpu::report_gpu_frame`].
    ///
    /// Call this once per frame (typically before
    /// [`astrelis_profiling::new_frame`]) to resolve GPU timestamp
    /// queries from previous frames. Results arrive 1-3 frames late
    /// due to GPU buffering. The profiler absorbs the scopes into
    /// its global timeline for the in-engine viewer to render.
    pub fn process_gpu_profiling_frames(&self) {
        astrelis_profiling::profile_function!();
        // Poll for completion of the last submission so buffer mapping
        // callbacks fire before we try to read profiler results.
        // We use a generous 100 ms timeout — the previous frame's GPU
        // work should complete well within this. An indefinite wait
        // risks hanging on drivers with broken timestamp support
        // (e.g. Metal on macOS 26 where timestamp queries never
        // resolve). If the timeout expires the profiling data for
        // that frame is lost, but the app stays responsive.
        let poll_type = match self.last_submission.lock().unwrap().take() {
            Some(idx) => wgpu::PollType::Wait {
                submission_index: Some(idx),
                timeout: Some(std::time::Duration::from_millis(100)),
            },
            None => wgpu::PollType::Poll,
        };
        let _ = self.device.poll(poll_type);
        let mut profiler = self.gpu_profiler.lock().unwrap();
        let period = self.queue.get_timestamp_period();
        while let Some(results) = profiler.process_finished_frame(period) {
            let scopes = convert_results(&results);
            if !scopes.is_empty() {
                astrelis_profiling::gpu::report_gpu_frame(
                    astrelis_profiling::gpu::GpuFrame { scopes },
                );
            }
        }
    }

    // --- Buffer ---

    /// Creates a GPU buffer.
    pub fn create_buffer(&self, desc: &BufferDescriptor<'_>) -> Buffer {
        astrelis_profiling::profile_function!();
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: desc.label,
            size: desc.size,
            usage: conv_buf::buffer_usages(desc.usage),
            mapped_at_creation: desc.mapped_at_creation,
        });
        Buffer(buffer)
    }

    /// Creates a GPU buffer with initial data.
    pub fn create_buffer_init(&self, desc: &BufferInitDescriptor<'_>) -> Buffer {
        astrelis_profiling::profile_function!();
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: desc.label,
            size: desc.contents.len() as u64,
            usage: conv_buf::buffer_usages(desc.usage) | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        buffer.slice(..).get_mapped_range_mut().copy_from_slice(desc.contents);
        buffer.unmap();
        Buffer(buffer)
    }

    /// Writes data to a buffer at the given byte offset.
    pub fn write_buffer(&self, buffer: &Buffer, offset: u64, data: &[u8]) {
        self.queue.write_buffer(&buffer.0, offset, data);
    }

    // --- Texture ---

    /// Creates a GPU texture.
    pub fn create_texture(&self, desc: &TextureDescriptor<'_>) -> Texture {
        astrelis_profiling::profile_function!();
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
        Texture(texture)
    }

    /// Creates a view into a texture.
    pub fn create_texture_view(
        &self,
        texture: &Texture,
        desc: &TextureViewDescriptor<'_>,
    ) -> TextureView {
        astrelis_profiling::profile_function!();
        let view = texture.0.create_view(&wgpu::TextureViewDescriptor {
            label: desc.label,
            format: desc.format.map(conv::texture_format),
            dimension: desc.dimension.map(conv::texture_view_dimension),
            base_mip_level: desc.base_mip_level,
            mip_level_count: desc.mip_level_count,
            base_array_layer: desc.base_array_layer,
            array_layer_count: desc.array_layer_count,
            ..Default::default()
        });
        TextureView(view)
    }

    /// Creates a texture sampler.
    pub fn create_sampler(&self, desc: &SamplerDescriptor<'_>) -> Sampler {
        astrelis_profiling::profile_function!();
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
        Sampler(sampler)
    }

    /// Writes data to a texture.
    pub fn write_texture(
        &self,
        dst: TextureCopyView<'_>,
        data: &[u8],
        layout: crate::command::BufferCopyView<'_>,
        size: Extent3d,
    ) {
        astrelis_profiling::profile_function!();
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &dst.texture.0,
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
    }

    // --- Shader ---

    /// Creates a shader module from source code.
    pub fn create_shader_module(
        &self,
        desc: &ShaderModuleDescriptor<'_>,
    ) -> Result<ShaderModule, GpuError> {
        astrelis_profiling::profile_function!();
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
        Ok(ShaderModule(module))
    }

    // --- Bind group ---

    /// Creates a bind group layout.
    pub fn create_bind_group_layout(
        &self,
        desc: &BindGroupLayoutDescriptor<'_>,
    ) -> BindGroupLayout {
        astrelis_profiling::profile_function!();
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
        BindGroupLayout(layout)
    }

    /// Creates a bind group.
    pub fn create_bind_group(
        &self,
        desc: &BindGroupDescriptor<'_>,
    ) -> BindGroup {
        astrelis_profiling::profile_function!();
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
                    wgpu::BindGroupEntry {
                        binding: *binding,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &buffer.0,
                            offset: *offset,
                            size: size.and_then(std::num::NonZeroU64::new),
                        }),
                    }
                }
                BindGroupEntry::TextureView { binding, view } => {
                    wgpu::BindGroupEntry {
                        binding: *binding,
                        resource: wgpu::BindingResource::TextureView(&view.0),
                    }
                }
                BindGroupEntry::Sampler { binding, sampler } => {
                    wgpu::BindGroupEntry {
                        binding: *binding,
                        resource: wgpu::BindingResource::Sampler(&sampler.0),
                    }
                }
            })
            .collect();

        let bind_group = self
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: desc.label,
                layout: &desc.layout.0,
                entries: &wgpu_entries,
            });
        BindGroup(bind_group)
    }

    // --- Pipeline ---

    /// Creates a pipeline layout.
    pub fn create_pipeline_layout(
        &self,
        desc: &PipelineLayoutDescriptor<'_>,
    ) -> PipelineLayout {
        astrelis_profiling::profile_function!();
        let bind_group_layout_opts: Vec<Option<&wgpu::BindGroupLayout>> = desc
            .bind_group_layouts
            .iter()
            .map(|l| Some(&l.0))
            .collect();

        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: desc.label,
                bind_group_layouts: &bind_group_layout_opts,
                ..Default::default()
            });
        PipelineLayout(layout)
    }

    /// Creates a render pipeline.
    pub fn create_render_pipeline(
        &self,
        desc: &RenderPipelineDescriptor<'_>,
    ) -> RenderPipeline {
        astrelis_profiling::profile_function!();

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
            wgpu::FragmentState {
                module: &f.module.0,
                entry_point: Some(f.entry_point),
                targets: &color_targets,
                compilation_options: Default::default(),
            }
        });

        let pipeline =
            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: desc.label,
                    layout: desc.layout.as_ref().map(|l| &l.0),
                    vertex: wgpu::VertexState {
                        module: &desc.vertex.module.0,
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
        RenderPipeline(pipeline)
    }

    /// Creates a compute pipeline.
    pub fn create_compute_pipeline(
        &self,
        desc: &ComputePipelineDescriptor<'_>,
    ) -> ComputePipeline {
        astrelis_profiling::profile_function!();

        let pipeline =
            self.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: desc.label,
                    layout: desc.layout.as_ref().map(|l| &l.0),
                    module: &desc.module.0,
                    entry_point: Some(desc.entry_point),
                    compilation_options: Default::default(),
                    cache: None,
                });
        ComputePipeline(pipeline)
    }

    /// Wraps a raw wgpu encoder operation with GPU profiler timestamps.
    ///
    /// Use this to profile GPU work that goes through raw wgpu APIs
    /// (e.g., third-party renderers like egui or text). The closure
    /// receives the raw encoder; a start timestamp is written before
    /// it runs and an end timestamp after. The scope appears in the
    /// profiler timeline alongside spans from [`CommandEncoder`].
    ///
    /// # Example
    ///
    /// ```ignore
    /// gpu.device().gpu_profile_scope(
    ///     "text_render",
    ///     &mut raw_encoder,
    ///     |encoder| {
    ///         text_renderer.render(gpu, encoder, view, w, h);
    ///     },
    /// );
    /// ```
    pub fn gpu_profile_scope(
        &self,
        label: &str,
        encoder: &mut wgpu::CommandEncoder,
        f: impl FnOnce(&mut wgpu::CommandEncoder),
    ) {
        if !astrelis_profiling::is_enabled() {
            f(encoder);
            return;
        }
        let profiler = self.gpu_profiler.lock().unwrap();
        let query = profiler.begin_query(label, encoder);
        drop(profiler);

        f(encoder);

        let profiler = self.gpu_profiler.lock().unwrap();
        profiler.end_query(encoder, query);
    }

    // --- Command ---

    /// Creates a new command encoder for recording GPU commands.
    pub fn create_command_encoder(&self, label: Option<&str>) -> CommandEncoder<'_> {
        astrelis_profiling::profile_function!();
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label });
        CommandEncoder::new(encoder, self)
    }
}

/// Converts a tree of `wgpu_profiler` timer-query results into
/// backend-agnostic [`astrelis_profiling::gpu::GpuScope`]s.
///
/// Results whose `time` field is `None` (unresolved) are silently
/// dropped — wgpu-profiler may emit these for queries still pending
/// readback. The timestamp period has already been applied by
/// `process_finished_frame`, so `time.start`/`time.end` are in
/// seconds; we convert to nanoseconds here.
///
/// Scopes with `start_ns == 0` or `end_ns < start_ns` are treated
/// as corrupt and dropped.
fn convert_results(
    results: &[wgpu_profiler::GpuTimerQueryResult],
) -> Vec<astrelis_profiling::gpu::GpuScope> {
    use astrelis_profiling::gpu::GpuScope;
    results
        .iter()
        .filter_map(|r| {
            let time = r.time.as_ref()?;
            let start_ns = (time.start * 1_000_000_000.0) as i64;
            let end_ns = (time.end * 1_000_000_000.0) as i64;
            if start_ns <= 0 || end_ns < start_ns {
                return None;
            }
            Some(GpuScope {
                label: r.label.clone(),
                start_ns,
                end_ns,
                nested: convert_results(&r.nested_queries),
            })
        })
        .collect()
}
