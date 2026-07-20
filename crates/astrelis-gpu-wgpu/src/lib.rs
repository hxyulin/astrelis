//! Wgpu implementation of the Astrelis GPU API.

#![warn(missing_docs)]

use std::{
    any::Any,
    ops::Range,
    sync::{Arc, Mutex},
};

use astrelis_gpu::{
    self as gpu, AdapterInfo, AddressMode, BindGroupDescriptor, BindGroupLayoutDescriptor,
    BindingResource, BindingType, BlendComponent, BlendFactor, BlendOperation, BufferBindingType,
    BufferDescriptor, BufferTextureCopy, CommandEncoderDescriptor, CompareFunction,
    CompositeAlphaMode, ComputePassDescriptor, ComputePipelineDescriptor, DeviceCapabilities,
    DeviceDescriptor, DeviceError, DeviceErrorKind, DeviceId, DeviceType, Extent3d, Face, Features,
    FilterMode, FrontFace, GpuError, GraphicsApi, IndexFormat, Limits, LoadOp, LoadOpValue,
    MapMode, PipelineLayoutDescriptor, PollMode, PowerPreference, PresentMode, PrimitiveTopology,
    QuerySetDescriptor, QueryType, RenderPassDescriptor, RenderPipelineDescriptor,
    RequestAdapterOptions, SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor,
    ShaderStages, StencilOperation, StoreOp, SurfaceCapabilities, SurfaceConfiguration,
    SurfaceFrameStatus, TextureCopy, TextureDataLayout, TextureDescriptor, TextureDimension,
    TextureFormat, TextureSampleType, TextureUsages, TextureViewDescriptor, TextureViewDimension,
    VertexFormat, VertexStepMode, backend,
};

#[cfg(feature = "profiling")]
mod profiling;
#[cfg(feature = "profiling")]
pub use profiling::WgpuGpuProfiler;

/// Wgpu instance creation options.
#[derive(Clone, Copy, Debug)]
pub struct InstanceDescriptor {
    /// Native graphics APIs to enable.
    pub backends: wgpu::Backends,
    /// Allow standard wgpu environment variables to override settings.
    pub use_environment: bool,
}

impl Default for InstanceDescriptor {
    fn default() -> Self {
        Self {
            backends: wgpu::Backends::PRIMARY,
            use_environment: true,
        }
    }
}

/// Creates a backend-neutral instance implemented by wgpu.
pub fn create_instance(descriptor: InstanceDescriptor) -> gpu::Instance {
    let mut native = if descriptor.use_environment {
        wgpu::InstanceDescriptor::new_without_display_handle_from_env()
    } else {
        wgpu::InstanceDescriptor::new_without_display_handle()
    };
    native.backends = descriptor.backends;
    gpu::Instance::from_backend(Arc::new(WgpuInstance {
        raw: wgpu::Instance::new(native),
    }))
}

/// Wraps an externally owned wgpu device and queue.
///
/// The caller must provide the graphics API because wgpu devices do not expose
/// their originating adapter after creation.
pub fn wrap_device(
    device: wgpu::Device,
    queue: wgpu::Queue,
    api: GraphicsApi,
) -> (gpu::Device, gpu::Queue) {
    let id = gpu::allocate_device_id();
    let features = convert_features_from_wgpu(device.features());
    let wrapped_device = Arc::new(WgpuDevice {
        id,
        capabilities: DeviceCapabilities {
            features,
            limits: convert_limits_from_wgpu(&device.limits()),
            api,
            reliable_timestamps: api != GraphicsApi::Metal
                && features.contains(Features::TIMESTAMP_QUERY),
        },
        raw: device,
        error_handler: Mutex::new(None),
    });
    install_wgpu_error_handlers(&wrapped_device);
    let wrapped_queue = Arc::new(WgpuQueue { id, raw: queue });
    (
        gpu::Device::from_backend(wrapped_device),
        gpu::Queue::from_backend(wrapped_queue),
    )
}

/// Access to a wrapped wgpu instance.
pub trait WgpuInstanceExt {
    /// Returns the native handle when this instance uses the wgpu backend.
    fn as_wgpu(&self) -> Option<&wgpu::Instance>;
}

impl WgpuInstanceExt for gpu::Instance {
    fn as_wgpu(&self) -> Option<&wgpu::Instance> {
        self.backend()
            .as_any()
            .downcast_ref::<WgpuInstance>()
            .map(|value| &value.raw)
    }
}

/// Access to a wrapped wgpu adapter.
pub trait WgpuAdapterExt {
    /// Returns the native handle when this adapter uses the wgpu backend.
    fn as_wgpu(&self) -> Option<&wgpu::Adapter>;
}

impl WgpuAdapterExt for gpu::Adapter {
    fn as_wgpu(&self) -> Option<&wgpu::Adapter> {
        self.backend()
            .as_any()
            .downcast_ref::<WgpuAdapter>()
            .map(|value| &value.raw)
    }
}

/// Access to a wrapped wgpu device.
pub trait WgpuDeviceExt {
    /// Returns the native handle when this device uses the wgpu backend.
    fn as_wgpu(&self) -> Option<&wgpu::Device>;
}

impl WgpuDeviceExt for gpu::Device {
    fn as_wgpu(&self) -> Option<&wgpu::Device> {
        self.backend()
            .as_any()
            .downcast_ref::<WgpuDevice>()
            .map(|value| &value.raw)
    }
}

/// Access to a wrapped wgpu queue.
pub trait WgpuQueueExt {
    /// Returns the native handle when this queue uses the wgpu backend.
    fn as_wgpu(&self) -> Option<&wgpu::Queue>;
}

impl WgpuQueueExt for gpu::Queue {
    fn as_wgpu(&self) -> Option<&wgpu::Queue> {
        self.backend()
            .as_any()
            .downcast_ref::<WgpuQueue>()
            .map(|value| &value.raw)
    }
}

/// Access to wrapped wgpu resources.
pub trait WgpuResourceExt<T> {
    /// Returns the native handle when this resource uses the wgpu backend.
    fn as_wgpu(&self) -> Option<&T>;
}

impl WgpuResourceExt<wgpu::Buffer> for gpu::Buffer {
    fn as_wgpu(&self) -> Option<&wgpu::Buffer> {
        self.inner_backend()
            .as_any()
            .downcast_ref::<WgpuBuffer>()
            .map(|value| &value.raw)
    }
}

impl WgpuResourceExt<wgpu::Texture> for gpu::Texture {
    fn as_wgpu(&self) -> Option<&wgpu::Texture> {
        self.backend()
            .as_any()
            .downcast_ref::<WgpuTexture>()
            .map(|value| &value.raw)
    }
}

impl WgpuResourceExt<wgpu::TextureView> for gpu::TextureView {
    fn as_wgpu(&self) -> Option<&wgpu::TextureView> {
        self.backend()
            .as_any()
            .downcast_ref::<WgpuTextureView>()
            .map(|value| &value.raw)
    }
}

impl WgpuResourceExt<wgpu::Sampler> for gpu::Sampler {
    fn as_wgpu(&self) -> Option<&wgpu::Sampler> {
        self.backend()
            .as_any()
            .downcast_ref::<WgpuSampler>()
            .map(|value| &value.raw)
    }
}

impl WgpuResourceExt<wgpu::ShaderModule> for gpu::ShaderModule {
    fn as_wgpu(&self) -> Option<&wgpu::ShaderModule> {
        self.backend()
            .as_any()
            .downcast_ref::<WgpuShaderModule>()
            .map(|value| &value.raw)
    }
}

impl WgpuResourceExt<wgpu::RenderPipeline> for gpu::RenderPipeline {
    fn as_wgpu(&self) -> Option<&wgpu::RenderPipeline> {
        self.backend()
            .as_any()
            .downcast_ref::<WgpuRenderPipeline>()
            .map(|value| &value.raw)
    }
}

#[derive(Debug)]
struct WgpuInstance {
    raw: wgpu::Instance,
}

impl backend::NativeHandle for WgpuInstance {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::Instance for WgpuInstance {
    fn create_surface(
        &self,
        target: gpu::SurfaceTarget,
    ) -> Result<Arc<dyn backend::Surface>, GpuError> {
        let surface = self
            .raw
            .create_surface(target)
            .map_err(|error| GpuError::new(error.to_string()))?;
        Ok(Arc::new(WgpuSurface {
            raw: surface,
            device_id: Mutex::new(None),
        }))
    }

    fn request_adapter(
        &self,
        options: RequestAdapterOptions,
    ) -> backend::BackendFuture<Result<Arc<dyn backend::Adapter>, GpuError>> {
        let instance = self.raw.clone();
        Box::pin(async move {
            let surface = options
                .compatible_surface
                .as_ref()
                .map(|surface| {
                    surface
                        .backend()
                        .as_any()
                        .downcast_ref::<WgpuSurface>()
                        .ok_or_else(|| GpuError::new("surface belongs to a different GPU backend"))
                })
                .transpose()?;
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: convert_power_preference(options.power_preference),
                    force_fallback_adapter: options.force_fallback_adapter,
                    compatible_surface: surface.map(|surface| &surface.raw),
                })
                .await
                .map_err(|error| GpuError::new(error.to_string()))?;
            Ok(Arc::new(WgpuAdapter { raw: adapter }) as Arc<dyn backend::Adapter>)
        })
    }
}

#[derive(Debug)]
struct WgpuAdapter {
    raw: wgpu::Adapter,
}

impl backend::NativeHandle for WgpuAdapter {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::Adapter for WgpuAdapter {
    fn info(&self) -> AdapterInfo {
        let info = self.raw.get_info();
        AdapterInfo {
            name: info.name,
            driver: info.driver,
            driver_info: info.driver_info,
            vendor: info.vendor,
            device: info.device,
            device_type: convert_device_type(info.device_type),
            api: convert_api(info.backend),
        }
    }

    fn features(&self) -> Features {
        convert_features_from_wgpu(self.raw.features())
    }

    fn limits(&self) -> Limits {
        convert_limits_from_wgpu(&self.raw.limits())
    }

    fn request_device(
        &self,
        id: DeviceId,
        descriptor: DeviceDescriptor,
    ) -> backend::BackendFuture<backend::DeviceQueueResult> {
        let adapter = self.raw.clone();
        Box::pin(async move {
            let available = adapter.features();
            let requested = descriptor.required_features
                | (descriptor.optional_features & convert_features_from_wgpu(available));
            let features = convert_features_to_wgpu(requested);
            let limits = convert_limits_to_wgpu(descriptor.required_limits);
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: descriptor.label.as_deref(),
                    required_features: features,
                    required_limits: limits,
                    ..Default::default()
                })
                .await
                .map_err(|error| GpuError::new(error.to_string()))?;
            let api = convert_api(adapter.get_info().backend);
            let capabilities = DeviceCapabilities {
                features: requested,
                limits: convert_limits_from_wgpu(&device.limits()),
                api,
                // Metal 4 currently advertises timestamp queries but returns zero-valued
                // samples: https://github.com/gfx-rs/wgpu/issues/9414
                reliable_timestamps: api != GraphicsApi::Metal
                    && requested.contains(Features::TIMESTAMP_QUERY),
            };
            let device = Arc::new(WgpuDevice {
                id,
                raw: device,
                capabilities,
                error_handler: Mutex::new(None),
            });
            install_wgpu_error_handlers(&device);
            let queue = Arc::new(WgpuQueue { id, raw: queue });
            Ok((
                device as Arc<dyn backend::Device>,
                queue as Arc<dyn backend::Queue>,
            ))
        })
    }
}

type ErrorHandler = Arc<dyn Fn(DeviceError) + Send + Sync>;

struct WgpuDevice {
    id: DeviceId,
    raw: wgpu::Device,
    capabilities: DeviceCapabilities,
    error_handler: Mutex<Option<ErrorHandler>>,
}

impl std::fmt::Debug for WgpuDevice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WgpuDevice")
            .field("id", &self.id)
            .finish()
    }
}

fn install_wgpu_error_handlers(device: &Arc<WgpuDevice>) {
    let weak = Arc::downgrade(device);
    device.raw.on_uncaptured_error(Arc::new(move |error| {
        if let Some(device) = weak.upgrade() {
            let (kind, message) = match error {
                wgpu::Error::OutOfMemory { source } => {
                    (DeviceErrorKind::OutOfMemory, source.to_string())
                }
                wgpu::Error::Validation { description, .. } => {
                    (DeviceErrorKind::Validation, description)
                }
                wgpu::Error::Internal { description, .. } => {
                    (DeviceErrorKind::Internal, description)
                }
            };
            device.emit_error(kind, message);
        }
    }));

    let weak = Arc::downgrade(device);
    device
        .raw
        .set_device_lost_callback(move |_reason, message| {
            if let Some(device) = weak.upgrade() {
                device.emit_error(DeviceErrorKind::DeviceLost, message);
            }
        });
}

impl WgpuDevice {
    fn emit_error(&self, kind: DeviceErrorKind, message: String) {
        if let Some(handler) = self
            .error_handler
            .lock()
            .expect("GPU error handler poisoned")
            .clone()
        {
            handler(DeviceError {
                device: self.id,
                kind,
                message,
            });
        }
    }
}

impl backend::NativeHandle for WgpuDevice {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::Device for WgpuDevice {
    fn id(&self) -> DeviceId {
        self.id
    }

    fn capabilities(&self) -> DeviceCapabilities {
        self.capabilities
    }

    fn set_error_handler(&self, handler: ErrorHandler) {
        *self
            .error_handler
            .lock()
            .expect("GPU error handler poisoned") = Some(handler);
    }

    fn poll(&self, mode: PollMode) -> Result<(), GpuError> {
        let poll_type = match mode {
            PollMode::Poll => wgpu::PollType::Poll,
            PollMode::Wait => wgpu::PollType::wait_indefinitely(),
        };
        self.raw
            .poll(poll_type)
            .map(|_| ())
            .map_err(|error| GpuError::new(error.to_string()))
    }

    fn create_buffer(&self, descriptor: BufferDescriptor) -> Arc<dyn backend::Buffer> {
        let buffer = self.raw.create_buffer(&wgpu::BufferDescriptor {
            label: descriptor.label.as_deref(),
            size: descriptor.size,
            usage: convert_buffer_usages(descriptor.usage),
            mapped_at_creation: descriptor.mapped_at_creation,
        });
        Arc::new(WgpuBuffer {
            id: self.id,
            raw: buffer,
        })
    }

    fn create_texture(&self, descriptor: TextureDescriptor) -> Arc<dyn backend::Texture> {
        let texture = self.raw.create_texture(&wgpu::TextureDescriptor {
            label: descriptor.label.as_deref(),
            size: convert_extent(descriptor.size),
            mip_level_count: descriptor.mip_level_count,
            sample_count: descriptor.sample_count,
            dimension: convert_texture_dimension(descriptor.dimension),
            format: convert_texture_format(descriptor.format),
            usage: convert_texture_usages(descriptor.usage),
            view_formats: &[],
        });
        Arc::new(WgpuTexture {
            id: self.id,
            raw: texture,
            sample_count: descriptor.sample_count,
            dimension: descriptor.dimension,
            format: descriptor.format,
        })
    }

    fn create_sampler(&self, descriptor: SamplerDescriptor) -> Arc<dyn backend::Sampler> {
        Arc::new(WgpuSampler {
            id: self.id,
            raw: self.raw.create_sampler(&wgpu::SamplerDescriptor {
                label: descriptor.label.as_deref(),
                address_mode_u: convert_address_mode(descriptor.address_mode_u),
                address_mode_v: convert_address_mode(descriptor.address_mode_v),
                address_mode_w: convert_address_mode(descriptor.address_mode_w),
                mag_filter: convert_filter_mode(descriptor.mag_filter),
                min_filter: convert_filter_mode(descriptor.min_filter),
                mipmap_filter: convert_mipmap_filter_mode(descriptor.mipmap_filter),
                lod_min_clamp: descriptor.lod_min_clamp,
                lod_max_clamp: descriptor.lod_max_clamp,
                ..Default::default()
            }),
        })
    }

    fn create_shader_module(
        &self,
        descriptor: ShaderModuleDescriptor,
    ) -> Arc<dyn backend::ShaderModule> {
        Arc::new(WgpuShaderModule {
            id: self.id,
            raw: self.raw.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: descriptor.label.as_deref(),
                source: wgpu::ShaderSource::Wgsl(descriptor.wgsl.into()),
            }),
        })
    }

    fn create_bind_group_layout(
        &self,
        descriptor: BindGroupLayoutDescriptor,
    ) -> Arc<dyn backend::BindGroupLayout> {
        let entries: Vec<_> = descriptor
            .entries
            .iter()
            .map(|entry| wgpu::BindGroupLayoutEntry {
                binding: entry.binding,
                visibility: convert_shader_stages(entry.visibility),
                ty: convert_binding_type(&entry.ty),
                count: None,
            })
            .collect();
        Arc::new(WgpuBindGroupLayout {
            id: self.id,
            raw: self
                .raw
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: descriptor.label.as_deref(),
                    entries: &entries,
                }),
        })
    }

    fn create_pipeline_layout(
        &self,
        descriptor: PipelineLayoutDescriptor,
    ) -> Arc<dyn backend::PipelineLayout> {
        let layouts: Vec<Option<&wgpu::BindGroupLayout>> = descriptor
            .bind_group_layouts
            .iter()
            .map(|layout| {
                Some(
                    &layout
                        .backend()
                        .as_any()
                        .downcast_ref::<WgpuBindGroupLayout>()
                        .expect("layout backend was checked by astrelis-gpu")
                        .raw,
                )
            })
            .collect();
        Arc::new(WgpuPipelineLayout {
            id: self.id,
            raw: self
                .raw
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: descriptor.label.as_deref(),
                    bind_group_layouts: &layouts,
                    immediate_size: 0,
                }),
        })
    }

    fn create_bind_group(&self, descriptor: BindGroupDescriptor) -> Arc<dyn backend::BindGroup> {
        enum NativeResource<'a> {
            Buffer(wgpu::BufferBinding<'a>),
            Sampler(&'a wgpu::Sampler),
            TextureView(&'a wgpu::TextureView),
        }
        let resources: Vec<NativeResource<'_>> = descriptor
            .entries
            .iter()
            .map(|entry| match &entry.resource {
                BindingResource::Buffer(binding) => {
                    let buffer = binding
                        .buffer
                        .inner_backend()
                        .as_any()
                        .downcast_ref::<WgpuBuffer>()
                        .expect("buffer backend was checked by astrelis-gpu");
                    NativeResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer.raw,
                        offset: binding.offset,
                        size: binding.size,
                    })
                }
                BindingResource::Sampler(sampler) => {
                    let sampler = sampler
                        .backend()
                        .as_any()
                        .downcast_ref::<WgpuSampler>()
                        .expect("sampler backend was checked by astrelis-gpu");
                    NativeResource::Sampler(&sampler.raw)
                }
                BindingResource::TextureView(view) => {
                    let view = view
                        .backend()
                        .as_any()
                        .downcast_ref::<WgpuTextureView>()
                        .expect("texture view backend was checked by astrelis-gpu");
                    NativeResource::TextureView(&view.raw)
                }
            })
            .collect();
        let entries: Vec<_> = descriptor
            .entries
            .iter()
            .zip(&resources)
            .map(|(entry, resource)| wgpu::BindGroupEntry {
                binding: entry.binding,
                resource: match resource {
                    NativeResource::Buffer(binding) => {
                        wgpu::BindingResource::Buffer(binding.clone())
                    }
                    NativeResource::Sampler(sampler) => wgpu::BindingResource::Sampler(sampler),
                    NativeResource::TextureView(view) => wgpu::BindingResource::TextureView(view),
                },
            })
            .collect();
        let layout = descriptor
            .layout
            .backend()
            .as_any()
            .downcast_ref::<WgpuBindGroupLayout>()
            .expect("layout backend was checked by astrelis-gpu");
        Arc::new(WgpuBindGroup {
            id: self.id,
            raw: self.raw.create_bind_group(&wgpu::BindGroupDescriptor {
                label: descriptor.label.as_deref(),
                layout: &layout.raw,
                entries: &entries,
            }),
        })
    }

    fn create_render_pipeline(
        &self,
        descriptor: RenderPipelineDescriptor,
    ) -> Arc<dyn backend::RenderPipeline> {
        let vertex_module = descriptor
            .vertex
            .module
            .backend()
            .as_any()
            .downcast_ref::<WgpuShaderModule>()
            .expect("shader module backend was checked by astrelis-gpu");
        let vertex_attributes: Vec<Vec<wgpu::VertexAttribute>> = descriptor
            .vertex
            .buffers
            .iter()
            .map(|layout| {
                layout
                    .attributes
                    .iter()
                    .map(|attribute| wgpu::VertexAttribute {
                        format: convert_vertex_format(attribute.format),
                        offset: attribute.offset,
                        shader_location: attribute.shader_location,
                    })
                    .collect()
            })
            .collect();
        let vertex_buffers: Vec<wgpu::VertexBufferLayout<'_>> = descriptor
            .vertex
            .buffers
            .iter()
            .zip(&vertex_attributes)
            .map(|(layout, attributes)| wgpu::VertexBufferLayout {
                array_stride: layout.array_stride,
                step_mode: convert_vertex_step_mode(layout.step_mode),
                attributes,
            })
            .collect();

        let fragment_module = descriptor.fragment.as_ref().map(|fragment| {
            fragment
                .module
                .backend()
                .as_any()
                .downcast_ref::<WgpuShaderModule>()
                .expect("shader module backend was checked by astrelis-gpu")
        });
        let fragment_targets: Vec<Option<wgpu::ColorTargetState>> = descriptor
            .fragment
            .as_ref()
            .map(|fragment| {
                fragment
                    .targets
                    .iter()
                    .map(|target| {
                        target.as_ref().map(|target| wgpu::ColorTargetState {
                            format: convert_texture_format(target.format),
                            blend: target.blend.map(|blend| wgpu::BlendState {
                                color: convert_blend_component(blend.color),
                                alpha: convert_blend_component(blend.alpha),
                            }),
                            write_mask: wgpu::ColorWrites::from_bits_retain(
                                target.write_mask.bits(),
                            ),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let fragment_state = descriptor
            .fragment
            .as_ref()
            .map(|fragment| wgpu::FragmentState {
                module: &fragment_module.expect("fragment module exists").raw,
                entry_point: Some(&fragment.entry_point),
                compilation_options: Default::default(),
                targets: &fragment_targets,
            });

        let pipeline_layout = descriptor.layout.as_ref().map(|layout| {
            &layout
                .backend()
                .as_any()
                .downcast_ref::<WgpuPipelineLayout>()
                .expect("pipeline layout backend was checked by astrelis-gpu")
                .raw
        });
        let pipeline = self
            .raw
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: descriptor.label.as_deref(),
                layout: pipeline_layout,
                vertex: wgpu::VertexState {
                    module: &vertex_module.raw,
                    entry_point: Some(&descriptor.vertex.entry_point),
                    compilation_options: Default::default(),
                    buffers: &vertex_buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: convert_primitive_topology(descriptor.primitive.topology),
                    front_face: convert_front_face(descriptor.primitive.front_face),
                    cull_mode: descriptor.primitive.cull_mode.map(convert_face),
                    ..Default::default()
                },
                depth_stencil: descriptor
                    .depth_stencil
                    .map(|state| wgpu::DepthStencilState {
                        format: convert_texture_format(state.format),
                        depth_write_enabled: Some(state.depth_write_enabled),
                        depth_compare: Some(convert_compare_function(state.depth_compare)),
                        stencil: wgpu::StencilState {
                            front: wgpu::StencilFaceState {
                                compare: convert_compare_function(state.stencil.front.compare),
                                fail_op: convert_stencil_operation(state.stencil.front.fail_op),
                                depth_fail_op: convert_stencil_operation(
                                    state.stencil.front.depth_fail_op,
                                ),
                                pass_op: convert_stencil_operation(state.stencil.front.pass_op),
                            },
                            back: wgpu::StencilFaceState {
                                compare: convert_compare_function(state.stencil.back.compare),
                                fail_op: convert_stencil_operation(state.stencil.back.fail_op),
                                depth_fail_op: convert_stencil_operation(
                                    state.stencil.back.depth_fail_op,
                                ),
                                pass_op: convert_stencil_operation(state.stencil.back.pass_op),
                            },
                            read_mask: state.stencil.read_mask,
                            write_mask: state.stencil.write_mask,
                        },
                        bias: wgpu::DepthBiasState {
                            constant: state.bias_constant,
                            slope_scale: state.bias_slope_scale,
                            clamp: state.bias_clamp,
                        },
                    }),
                multisample: wgpu::MultisampleState {
                    count: descriptor.multisample.count,
                    mask: descriptor.multisample.mask,
                    alpha_to_coverage_enabled: descriptor.multisample.alpha_to_coverage_enabled,
                },
                fragment: fragment_state,
                multiview_mask: None,
                cache: None,
            });
        Arc::new(WgpuRenderPipeline {
            id: self.id,
            raw: pipeline,
        })
    }

    fn create_query_set(&self, descriptor: QuerySetDescriptor) -> Arc<dyn backend::QuerySet> {
        Arc::new(WgpuQuerySet {
            id: self.id,
            raw: self.raw.create_query_set(&wgpu::QuerySetDescriptor {
                label: descriptor.label.as_deref(),
                ty: match descriptor.query_type {
                    QueryType::Timestamp => wgpu::QueryType::Timestamp,
                },
                count: descriptor.count,
            }),
        })
    }

    fn create_compute_pipeline(
        &self,
        descriptor: ComputePipelineDescriptor,
    ) -> Arc<dyn backend::ComputePipeline> {
        let module = descriptor
            .module
            .backend()
            .as_any()
            .downcast_ref::<WgpuShaderModule>()
            .expect("shader backend was checked by astrelis-gpu");
        let layout = descriptor.layout.as_ref().map(|layout| {
            &layout
                .backend()
                .as_any()
                .downcast_ref::<WgpuPipelineLayout>()
                .expect("pipeline layout backend was checked by astrelis-gpu")
                .raw
        });
        Arc::new(WgpuComputePipeline {
            id: self.id,
            raw: self
                .raw
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: descriptor.label.as_deref(),
                    layout,
                    module: &module.raw,
                    entry_point: Some(&descriptor.entry_point),
                    compilation_options: Default::default(),
                    cache: None,
                }),
        })
    }

    fn create_command_encoder(
        &self,
        descriptor: CommandEncoderDescriptor,
    ) -> Box<dyn backend::CommandEncoder> {
        Box::new(WgpuCommandEncoder {
            id: self.id,
            raw: Some(
                self.raw
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: descriptor.label.as_deref(),
                    }),
            ),
        })
    }
}

#[derive(Debug)]
struct WgpuQueue {
    id: DeviceId,
    raw: wgpu::Queue,
}

impl backend::NativeHandle for WgpuQueue {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::Queue for WgpuQueue {
    fn device_id(&self) -> DeviceId {
        self.id
    }

    fn timestamp_period(&self) -> f32 {
        self.raw.get_timestamp_period()
    }

    fn write_buffer(
        &self,
        buffer: &dyn backend::Buffer,
        offset: u64,
        data: &[u8],
    ) -> Result<(), GpuError> {
        let buffer = downcast_ref::<WgpuBuffer>(buffer)?;
        self.raw.write_buffer(&buffer.raw, offset, data);
        Ok(())
    }

    fn write_texture(
        &self,
        destination: &TextureCopy,
        data: &[u8],
        layout: TextureDataLayout,
        extent: Extent3d,
    ) -> Result<(), GpuError> {
        let texture = destination
            .texture
            .backend()
            .as_any()
            .downcast_ref::<WgpuTexture>()
            .ok_or_else(|| GpuError::new("texture belongs to another backend"))?;
        self.raw.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.raw,
                mip_level: destination.mip_level,
                origin: wgpu::Origin3d {
                    x: destination.origin.x,
                    y: destination.origin.y,
                    z: destination.origin.z,
                },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: layout.offset,
                bytes_per_row: layout.bytes_per_row,
                rows_per_image: layout.rows_per_image,
            },
            convert_extent(extent),
        );
        Ok(())
    }

    fn submit(
        &self,
        buffers: Vec<Box<dyn backend::CommandBuffer>>,
    ) -> Result<gpu::SubmissionId, GpuError> {
        let mut native = Vec::with_capacity(buffers.len());
        for buffer in buffers {
            let buffer = buffer
                .into_any()
                .downcast::<WgpuCommandBuffer>()
                .map_err(|_| GpuError::new("command buffer belongs to another backend"))?;
            native.push(buffer.raw);
        }
        let index = self.raw.submit(native);
        // wgpu's submission index is intentionally opaque, so Astrelis uses a
        // queue-local diagnostic identifier.
        let _ = index;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Ok(gpu::SubmissionId(
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ))
    }
}

#[derive(Debug)]
struct WgpuSurface {
    raw: wgpu::Surface<'static>,
    device_id: Mutex<Option<DeviceId>>,
}

impl backend::NativeHandle for WgpuSurface {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::Surface for WgpuSurface {
    fn capabilities(
        &self,
        adapter: &dyn backend::Adapter,
    ) -> Result<SurfaceCapabilities, GpuError> {
        let adapter = downcast_ref::<WgpuAdapter>(adapter)?;
        let capabilities = self.raw.get_capabilities(&adapter.raw);
        Ok(SurfaceCapabilities {
            formats: capabilities
                .formats
                .into_iter()
                .filter_map(convert_texture_format_from_wgpu)
                .collect(),
            present_modes: capabilities
                .present_modes
                .into_iter()
                .filter_map(convert_present_mode_from_wgpu)
                .collect(),
            alpha_modes: capabilities
                .alpha_modes
                .into_iter()
                .filter_map(convert_alpha_mode_from_wgpu)
                .collect(),
        })
    }

    fn configure(
        &self,
        device: &dyn backend::Device,
        configuration: SurfaceConfiguration,
    ) -> Result<(), GpuError> {
        let device = downcast_ref::<WgpuDevice>(device)?;
        *self
            .device_id
            .lock()
            .expect("surface device identity poisoned") = Some(device.id);
        self.raw.configure(
            &device.raw,
            &wgpu::SurfaceConfiguration {
                usage: convert_texture_usages(configuration.usage),
                format: convert_texture_format(configuration.format),
                width: configuration.width,
                height: configuration.height,
                present_mode: convert_present_mode(configuration.present_mode),
                alpha_mode: convert_alpha_mode(configuration.alpha_mode),
                view_formats: configuration
                    .view_formats
                    .into_iter()
                    .map(convert_texture_format)
                    .collect(),
                desired_maximum_frame_latency: configuration.desired_maximum_frame_latency,
            },
        );
        Ok(())
    }

    fn acquire(&self) -> Result<SurfaceFrameStatus, GpuError> {
        use wgpu::CurrentSurfaceTexture as Current;
        let device_id = self
            .device_id
            .lock()
            .expect("surface device identity poisoned")
            .ok_or_else(|| GpuError::new("surface must be configured before acquisition"))?;
        let status = match self.raw.get_current_texture() {
            Current::Success(frame) => SurfaceFrameStatus::Ready(gpu::SurfaceFrame::from_backend(
                Box::new(WgpuSurfaceFrame::new(frame, device_id)),
            )),
            Current::Suboptimal(frame) => SurfaceFrameStatus::Suboptimal(
                gpu::SurfaceFrame::from_backend(Box::new(WgpuSurfaceFrame::new(frame, device_id))),
            ),
            Current::Timeout => SurfaceFrameStatus::Timeout,
            Current::Occluded => SurfaceFrameStatus::Occluded,
            Current::Outdated => SurfaceFrameStatus::Outdated,
            Current::Lost => SurfaceFrameStatus::Lost,
            Current::Validation => {
                return Err(GpuError::new(
                    "surface acquisition failed backend validation",
                ));
            }
        };
        Ok(status)
    }
}

#[derive(Debug)]
struct WgpuSurfaceFrame {
    frame: wgpu::SurfaceTexture,
    texture: Arc<WgpuTexture>,
}

impl WgpuSurfaceFrame {
    fn new(frame: wgpu::SurfaceTexture, device_id: DeviceId) -> Self {
        let texture = Arc::new(WgpuTexture {
            id: device_id,
            raw: frame.texture.clone(),
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: convert_texture_format_from_wgpu(frame.texture.format())
                .expect("surface format is supported"),
        });
        Self { frame, texture }
    }
}

impl backend::NativeHandle for WgpuSurfaceFrame {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::SurfaceFrame for WgpuSurfaceFrame {
    fn texture(&self) -> Arc<dyn backend::Texture> {
        self.texture.clone()
    }

    fn present(self: Box<Self>) -> Result<(), GpuError> {
        self.frame.present();
        Ok(())
    }
}

#[derive(Debug)]
struct WgpuBuffer {
    id: DeviceId,
    raw: wgpu::Buffer,
}

impl backend::NativeHandle for WgpuBuffer {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::Buffer for WgpuBuffer {
    fn device_id(&self) -> DeviceId {
        self.id
    }

    fn size(&self) -> u64 {
        self.raw.size()
    }

    fn map_async(
        &self,
        mode: MapMode,
        range: Range<u64>,
    ) -> backend::BackendFuture<Result<(), GpuError>> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        self.raw
            .slice(range)
            .map_async(convert_map_mode(mode), move |result| {
                let _ = sender.send(result);
            });
        Box::pin(async move {
            receiver
                .recv()
                .map_err(|_| GpuError::new("mapping callback was dropped"))?
                .map_err(|error| GpuError::new(error.to_string()))
        })
    }

    fn read_mapped(&self, range: Range<u64>) -> Result<Vec<u8>, GpuError> {
        Ok(self.raw.slice(range).get_mapped_range().to_vec())
    }

    fn write_mapped(&self, offset: u64, data: &[u8]) -> Result<(), GpuError> {
        let end = offset
            .checked_add(data.len() as u64)
            .ok_or_else(|| GpuError::new("mapped write range overflow"))?;
        self.raw
            .slice(offset..end)
            .get_mapped_range_mut()
            .copy_from_slice(data);
        Ok(())
    }

    fn unmap(&self) {
        self.raw.unmap();
    }
}

#[derive(Debug)]
struct WgpuTexture {
    id: DeviceId,
    raw: wgpu::Texture,
    sample_count: u32,
    dimension: TextureDimension,
    format: TextureFormat,
}

impl backend::NativeHandle for WgpuTexture {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::Texture for WgpuTexture {
    fn device_id(&self) -> DeviceId {
        self.id
    }

    fn create_view(&self, descriptor: TextureViewDescriptor) -> Arc<dyn backend::TextureView> {
        Arc::new(WgpuTextureView {
            id: self.id,
            raw: self.raw.create_view(&wgpu::TextureViewDescriptor {
                label: descriptor.label.as_deref(),
                format: descriptor.format.map(convert_texture_format),
                base_mip_level: descriptor.base_mip_level,
                mip_level_count: descriptor.mip_level_count,
                base_array_layer: descriptor.base_array_layer,
                array_layer_count: descriptor.array_layer_count,
                ..Default::default()
            }),
            sample_count: self.sample_count,
            dimension: self.dimension,
            format: descriptor.format.unwrap_or(self.format),
        })
    }
}

#[derive(Debug)]
struct WgpuTextureView {
    id: DeviceId,
    raw: wgpu::TextureView,
    sample_count: u32,
    dimension: TextureDimension,
    format: TextureFormat,
}

impl backend::NativeHandle for WgpuTextureView {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::TextureView for WgpuTextureView {
    fn device_id(&self) -> DeviceId {
        self.id
    }

    fn sample_count(&self) -> u32 {
        self.sample_count
    }

    fn dimension(&self) -> TextureDimension {
        self.dimension
    }

    fn format(&self) -> TextureFormat {
        self.format
    }
}

#[derive(Debug)]
struct WgpuSampler {
    id: DeviceId,
    raw: wgpu::Sampler,
}

impl backend::NativeHandle for WgpuSampler {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::Sampler for WgpuSampler {
    fn device_id(&self) -> DeviceId {
        self.id
    }
}

#[derive(Debug)]
struct WgpuShaderModule {
    id: DeviceId,
    raw: wgpu::ShaderModule,
}

impl backend::NativeHandle for WgpuShaderModule {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::ShaderModule for WgpuShaderModule {
    fn device_id(&self) -> DeviceId {
        self.id
    }
}

#[derive(Debug)]
struct WgpuBindGroupLayout {
    id: DeviceId,
    raw: wgpu::BindGroupLayout,
}

impl backend::NativeHandle for WgpuBindGroupLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::BindGroupLayout for WgpuBindGroupLayout {
    fn device_id(&self) -> DeviceId {
        self.id
    }
}

#[derive(Debug)]
struct WgpuPipelineLayout {
    id: DeviceId,
    raw: wgpu::PipelineLayout,
}

impl backend::NativeHandle for WgpuPipelineLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::PipelineLayout for WgpuPipelineLayout {
    fn device_id(&self) -> DeviceId {
        self.id
    }
}

#[derive(Debug)]
struct WgpuBindGroup {
    id: DeviceId,
    raw: wgpu::BindGroup,
}

impl backend::NativeHandle for WgpuBindGroup {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::BindGroup for WgpuBindGroup {
    fn device_id(&self) -> DeviceId {
        self.id
    }
}

#[derive(Debug)]
struct WgpuRenderPipeline {
    id: DeviceId,
    raw: wgpu::RenderPipeline,
}

impl backend::NativeHandle for WgpuRenderPipeline {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::RenderPipeline for WgpuRenderPipeline {
    fn device_id(&self) -> DeviceId {
        self.id
    }
}

#[derive(Debug)]
struct WgpuComputePipeline {
    id: DeviceId,
    raw: wgpu::ComputePipeline,
}

impl backend::NativeHandle for WgpuComputePipeline {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::ComputePipeline for WgpuComputePipeline {
    fn device_id(&self) -> DeviceId {
        self.id
    }
}

#[derive(Debug)]
struct WgpuQuerySet {
    id: DeviceId,
    raw: wgpu::QuerySet,
}

impl backend::NativeHandle for WgpuQuerySet {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl backend::QuerySet for WgpuQuerySet {
    fn device_id(&self) -> DeviceId {
        self.id
    }
}

#[derive(Debug)]
struct WgpuCommandEncoder {
    id: DeviceId,
    raw: Option<wgpu::CommandEncoder>,
}

impl backend::CommandEncoder for WgpuCommandEncoder {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn device_id(&self) -> DeviceId {
        self.id
    }

    fn begin_render_pass<'a>(
        &'a mut self,
        descriptor: RenderPassDescriptor,
    ) -> Result<Box<dyn backend::RenderPass + 'a>, GpuError> {
        let mut native_attachments = Vec::with_capacity(descriptor.color_attachments.len());
        for attachment in &descriptor.color_attachments {
            native_attachments.push(
                attachment
                    .as_ref()
                    .map(|attachment| {
                        let view = attachment
                            .view
                            .backend()
                            .as_any()
                            .downcast_ref::<WgpuTextureView>()
                            .ok_or_else(|| {
                                GpuError::new("texture view belongs to another backend")
                            })?;
                        let resolve = attachment
                            .resolve_target
                            .as_ref()
                            .map(|target| {
                                target
                                    .backend()
                                    .as_any()
                                    .downcast_ref::<WgpuTextureView>()
                                    .map(|target| &target.raw)
                                    .ok_or_else(|| {
                                        GpuError::new(
                                            "resolve texture view belongs to another backend",
                                        )
                                    })
                            })
                            .transpose()?;
                        Ok(wgpu::RenderPassColorAttachment {
                            view: &view.raw,
                            resolve_target: resolve,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: match attachment.load {
                                    LoadOp::Load => wgpu::LoadOp::Load,
                                    LoadOp::Clear(color) => wgpu::LoadOp::Clear(wgpu::Color {
                                        r: color.r,
                                        g: color.g,
                                        b: color.b,
                                        a: color.a,
                                    }),
                                },
                                store: match attachment.store {
                                    StoreOp::Store => wgpu::StoreOp::Store,
                                    StoreOp::Discard => wgpu::StoreOp::Discard,
                                },
                            },
                        })
                    })
                    .transpose()?,
            );
        }
        let encoder = self
            .raw
            .as_mut()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?;
        let timestamp_query = descriptor
            .timestamp_writes
            .as_ref()
            .map(|writes| {
                writes
                    .query_set
                    .backend()
                    .as_any()
                    .downcast_ref::<WgpuQuerySet>()
                    .ok_or_else(|| GpuError::new("query set belongs to another backend"))
            })
            .transpose()?;
        let timestamp_writes =
            descriptor
                .timestamp_writes
                .as_ref()
                .map(|writes| wgpu::RenderPassTimestampWrites {
                    query_set: &timestamp_query.expect("query set exists").raw,
                    beginning_of_pass_write_index: writes.beginning_of_pass_write_index,
                    end_of_pass_write_index: writes.end_of_pass_write_index,
                });
        let depth_stencil_view = descriptor
            .depth_stencil_attachment
            .as_ref()
            .map(|attachment| {
                attachment
                    .view
                    .backend()
                    .as_any()
                    .downcast_ref::<WgpuTextureView>()
                    .ok_or_else(|| GpuError::new("depth/stencil view belongs to another backend"))
            })
            .transpose()?;
        let depth_stencil_attachment =
            descriptor
                .depth_stencil_attachment
                .as_ref()
                .map(|attachment| wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_stencil_view.expect("depth/stencil view exists").raw,
                    depth_ops: attachment.depth_ops.map(|ops| wgpu::Operations {
                        load: match ops.load {
                            LoadOpValue::Load => wgpu::LoadOp::Load,
                            LoadOpValue::Clear(value) => wgpu::LoadOp::Clear(value),
                        },
                        store: convert_store_op(ops.store),
                    }),
                    stencil_ops: attachment.stencil_ops.map(|ops| wgpu::Operations {
                        load: match ops.load {
                            LoadOpValue::Load => wgpu::LoadOp::Load,
                            LoadOpValue::Clear(value) => wgpu::LoadOp::Clear(value),
                        },
                        store: convert_store_op(ops.store),
                    }),
                });
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: descriptor.label.as_deref(),
            color_attachments: &native_attachments,
            depth_stencil_attachment,
            timestamp_writes,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        Ok(Box::new(WgpuRenderPass {
            raw: pass.forget_lifetime(),
        }))
    }

    fn begin_compute_pass<'a>(
        &'a mut self,
        descriptor: ComputePassDescriptor,
    ) -> Result<Box<dyn backend::ComputePass + 'a>, GpuError> {
        let pass = self
            .raw
            .as_mut()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?
            .begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: descriptor.label.as_deref(),
                timestamp_writes: None,
            });
        Ok(Box::new(WgpuComputePass {
            raw: pass.forget_lifetime(),
        }))
    }

    fn copy_buffer_to_buffer(
        &mut self,
        source: &dyn backend::Buffer,
        source_offset: u64,
        destination: &dyn backend::Buffer,
        destination_offset: u64,
        size: u64,
    ) -> Result<(), GpuError> {
        let source = downcast_ref::<WgpuBuffer>(source)?;
        let destination = downcast_ref::<WgpuBuffer>(destination)?;
        self.raw
            .as_mut()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?
            .copy_buffer_to_buffer(
                &source.raw,
                source_offset,
                &destination.raw,
                destination_offset,
                size,
            );
        Ok(())
    }

    fn copy_texture_to_buffer(
        &mut self,
        source: &TextureCopy,
        destination: &BufferTextureCopy,
        extent: Extent3d,
    ) -> Result<(), GpuError> {
        let source_texture = source
            .texture
            .backend()
            .as_any()
            .downcast_ref::<WgpuTexture>()
            .ok_or_else(|| GpuError::new("texture belongs to another backend"))?;
        let destination_buffer = destination
            .buffer
            .inner_backend()
            .as_any()
            .downcast_ref::<WgpuBuffer>()
            .ok_or_else(|| GpuError::new("buffer belongs to another backend"))?;
        self.raw
            .as_mut()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?
            .copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &source_texture.raw,
                    mip_level: source.mip_level,
                    origin: wgpu::Origin3d {
                        x: source.origin.x,
                        y: source.origin.y,
                        z: source.origin.z,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &destination_buffer.raw,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: destination.offset,
                        bytes_per_row: destination.bytes_per_row,
                        rows_per_image: destination.rows_per_image,
                    },
                },
                convert_extent(extent),
            );
        Ok(())
    }

    fn resolve_query_set(
        &mut self,
        query_set: &dyn backend::QuerySet,
        queries: Range<u32>,
        destination: &dyn backend::Buffer,
        destination_offset: u64,
    ) -> Result<(), GpuError> {
        let query_set = downcast_ref::<WgpuQuerySet>(query_set)?;
        let destination = downcast_ref::<WgpuBuffer>(destination)?;
        self.raw
            .as_mut()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?
            .resolve_query_set(
                &query_set.raw,
                queries,
                &destination.raw,
                destination_offset,
            );
        Ok(())
    }

    fn push_debug_group(&mut self, label: &str) {
        if let Some(encoder) = &mut self.raw {
            encoder.push_debug_group(label);
        }
    }

    fn pop_debug_group(&mut self) {
        if let Some(encoder) = &mut self.raw {
            encoder.pop_debug_group();
        }
    }

    fn finish(mut self: Box<Self>) -> Result<Box<dyn backend::CommandBuffer>, GpuError> {
        let raw = self
            .raw
            .take()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?
            .finish();
        Ok(Box::new(WgpuCommandBuffer { id: self.id, raw }))
    }
}

#[derive(Debug)]
struct WgpuRenderPass {
    raw: wgpu::RenderPass<'static>,
}

impl backend::RenderPass for WgpuRenderPass {
    fn set_pipeline(&mut self, pipeline: &dyn backend::RenderPipeline) -> Result<(), GpuError> {
        let pipeline = downcast_ref::<WgpuRenderPipeline>(pipeline)?;
        self.raw.set_pipeline(&pipeline.raw);
        Ok(())
    }

    fn set_vertex_buffer(
        &mut self,
        slot: u32,
        buffer: &dyn backend::Buffer,
        range: Range<u64>,
    ) -> Result<(), GpuError> {
        let buffer = downcast_ref::<WgpuBuffer>(buffer)?;
        self.raw.set_vertex_buffer(slot, buffer.raw.slice(range));
        Ok(())
    }

    fn set_index_buffer(
        &mut self,
        buffer: &dyn backend::Buffer,
        range: Range<u64>,
        format: IndexFormat,
    ) -> Result<(), GpuError> {
        let buffer = downcast_ref::<WgpuBuffer>(buffer)?;
        self.raw.set_index_buffer(
            buffer.raw.slice(range),
            match format {
                IndexFormat::Uint16 => wgpu::IndexFormat::Uint16,
                IndexFormat::Uint32 => wgpu::IndexFormat::Uint32,
            },
        );
        Ok(())
    }

    fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &dyn backend::BindGroup,
        dynamic_offsets: &[u32],
    ) -> Result<(), GpuError> {
        let bind_group = downcast_ref::<WgpuBindGroup>(bind_group)?;
        self.raw
            .set_bind_group(index, &bind_group.raw, dynamic_offsets);
        Ok(())
    }

    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        self.raw.draw(vertices, instances);
    }

    fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        self.raw.draw_indexed(indices, base_vertex, instances);
    }

    fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.raw.set_scissor_rect(x, y, width, height);
    }

    fn set_viewport(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) {
        self.raw
            .set_viewport(x, y, width, height, min_depth, max_depth);
    }

    fn set_stencil_reference(&mut self, reference: u32) {
        self.raw.set_stencil_reference(reference);
    }
}

#[derive(Debug)]
struct WgpuComputePass {
    raw: wgpu::ComputePass<'static>,
}

impl backend::ComputePass for WgpuComputePass {
    fn set_pipeline(&mut self, pipeline: &dyn backend::ComputePipeline) -> Result<(), GpuError> {
        let pipeline = downcast_ref::<WgpuComputePipeline>(pipeline)?;
        self.raw.set_pipeline(&pipeline.raw);
        Ok(())
    }

    fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &dyn backend::BindGroup,
        dynamic_offsets: &[u32],
    ) -> Result<(), GpuError> {
        let bind_group = downcast_ref::<WgpuBindGroup>(bind_group)?;
        self.raw
            .set_bind_group(index, &bind_group.raw, dynamic_offsets);
        Ok(())
    }

    fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32) {
        self.raw.dispatch_workgroups(x, y, z);
    }
}

#[derive(Debug)]
struct WgpuCommandBuffer {
    id: DeviceId,
    raw: wgpu::CommandBuffer,
}

impl backend::CommandBuffer for WgpuCommandBuffer {
    fn device_id(&self) -> DeviceId {
        self.id
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any + Send> {
        self
    }
}

fn downcast_ref<T: 'static>(value: &dyn backend::NativeHandle) -> Result<&T, GpuError> {
    value
        .as_any()
        .downcast_ref()
        .ok_or_else(|| GpuError::new("GPU handle belongs to another backend"))
}

fn convert_power_preference(value: PowerPreference) -> wgpu::PowerPreference {
    match value {
        PowerPreference::None => wgpu::PowerPreference::None,
        PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
        PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
    }
}

fn convert_device_type(value: wgpu::DeviceType) -> DeviceType {
    match value {
        wgpu::DeviceType::IntegratedGpu => DeviceType::Integrated,
        wgpu::DeviceType::DiscreteGpu => DeviceType::Discrete,
        wgpu::DeviceType::VirtualGpu => DeviceType::Virtual,
        wgpu::DeviceType::Cpu => DeviceType::Cpu,
        _ => DeviceType::Other,
    }
}

fn convert_api(value: wgpu::Backend) -> GraphicsApi {
    match value {
        wgpu::Backend::Vulkan => GraphicsApi::Vulkan,
        wgpu::Backend::Metal => GraphicsApi::Metal,
        wgpu::Backend::Dx12 => GraphicsApi::Dx12,
        wgpu::Backend::Gl => GraphicsApi::Gl,
        wgpu::Backend::BrowserWebGpu => GraphicsApi::WebGpu,
        _ => GraphicsApi::Other,
    }
}

fn convert_features_from_wgpu(value: wgpu::Features) -> Features {
    let mut result = Features::empty();
    let mappings = [
        (wgpu::Features::TIMESTAMP_QUERY, Features::TIMESTAMP_QUERY),
        (
            wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS,
            Features::TIMESTAMP_QUERY_INSIDE_ENCODERS,
        ),
        (
            wgpu::Features::TEXTURE_COMPRESSION_BC,
            Features::TEXTURE_COMPRESSION_BC,
        ),
        (
            wgpu::Features::TEXTURE_COMPRESSION_ETC2,
            Features::TEXTURE_COMPRESSION_ETC2,
        ),
        (
            wgpu::Features::TEXTURE_COMPRESSION_ASTC,
            Features::TEXTURE_COMPRESSION_ASTC,
        ),
        (
            wgpu::Features::MULTI_DRAW_INDIRECT_COUNT,
            Features::MULTI_DRAW_INDIRECT_COUNT,
        ),
        (
            wgpu::Features::POLYGON_MODE_LINE,
            Features::POLYGON_MODE_LINE,
        ),
    ];
    for (native, neutral) in mappings {
        if value.contains(native) {
            result |= neutral;
        }
    }
    result
}

fn convert_features_to_wgpu(value: Features) -> wgpu::Features {
    let mut result = wgpu::Features::empty();
    let mappings = [
        (Features::TIMESTAMP_QUERY, wgpu::Features::TIMESTAMP_QUERY),
        (
            Features::TIMESTAMP_QUERY_INSIDE_ENCODERS,
            wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS,
        ),
        (
            Features::TEXTURE_COMPRESSION_BC,
            wgpu::Features::TEXTURE_COMPRESSION_BC,
        ),
        (
            Features::TEXTURE_COMPRESSION_ETC2,
            wgpu::Features::TEXTURE_COMPRESSION_ETC2,
        ),
        (
            Features::TEXTURE_COMPRESSION_ASTC,
            wgpu::Features::TEXTURE_COMPRESSION_ASTC,
        ),
        (
            Features::MULTI_DRAW_INDIRECT_COUNT,
            wgpu::Features::MULTI_DRAW_INDIRECT_COUNT,
        ),
        (
            Features::POLYGON_MODE_LINE,
            wgpu::Features::POLYGON_MODE_LINE,
        ),
    ];
    for (neutral, native) in mappings {
        if value.contains(neutral) {
            result |= native;
        }
    }
    result
}

fn convert_limits_from_wgpu(value: &wgpu::Limits) -> Limits {
    Limits {
        max_texture_dimension_2d: value.max_texture_dimension_2d,
        max_bind_groups: value.max_bind_groups,
        max_vertex_buffers: value.max_vertex_buffers,
        max_buffer_size: value.max_buffer_size,
        min_uniform_buffer_offset_alignment: value.min_uniform_buffer_offset_alignment,
        min_storage_buffer_offset_alignment: value.min_storage_buffer_offset_alignment,
    }
}

fn convert_limits_to_wgpu(value: Limits) -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_2d: value.max_texture_dimension_2d,
        max_bind_groups: value.max_bind_groups,
        max_vertex_buffers: value.max_vertex_buffers,
        max_buffer_size: value.max_buffer_size,
        min_uniform_buffer_offset_alignment: value.min_uniform_buffer_offset_alignment,
        min_storage_buffer_offset_alignment: value.min_storage_buffer_offset_alignment,
        ..wgpu::Limits::defaults()
    }
}

fn convert_buffer_usages(value: gpu::BufferUsages) -> wgpu::BufferUsages {
    wgpu::BufferUsages::from_bits_retain(value.bits())
}

fn convert_texture_usages(value: TextureUsages) -> wgpu::TextureUsages {
    wgpu::TextureUsages::from_bits_retain(value.bits())
}

fn convert_extent(value: Extent3d) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: value.width,
        height: value.height,
        depth_or_array_layers: value.depth_or_array_layers,
    }
}

fn convert_texture_dimension(value: TextureDimension) -> wgpu::TextureDimension {
    match value {
        TextureDimension::D1 => wgpu::TextureDimension::D1,
        TextureDimension::D2 => wgpu::TextureDimension::D2,
        TextureDimension::D3 => wgpu::TextureDimension::D3,
    }
}

fn convert_texture_format(value: TextureFormat) -> wgpu::TextureFormat {
    match value {
        TextureFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
        TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
        TextureFormat::R32Float => wgpu::TextureFormat::R32Float,
        TextureFormat::R32Uint => wgpu::TextureFormat::R32Uint,
        TextureFormat::Depth16Unorm => wgpu::TextureFormat::Depth16Unorm,
        TextureFormat::Depth24PlusStencil8 => wgpu::TextureFormat::Depth24PlusStencil8,
        TextureFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
        _ => panic!("texture format is not supported by the wgpu backend"),
    }
}

fn convert_texture_format_from_wgpu(value: wgpu::TextureFormat) -> Option<TextureFormat> {
    Some(match value {
        wgpu::TextureFormat::R8Unorm => TextureFormat::R8Unorm,
        wgpu::TextureFormat::Rgba8Unorm => TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb => TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8Unorm => TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb => TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba16Float => TextureFormat::Rgba16Float,
        wgpu::TextureFormat::R32Float => TextureFormat::R32Float,
        wgpu::TextureFormat::R32Uint => TextureFormat::R32Uint,
        wgpu::TextureFormat::Depth16Unorm => TextureFormat::Depth16Unorm,
        wgpu::TextureFormat::Depth24PlusStencil8 => TextureFormat::Depth24PlusStencil8,
        wgpu::TextureFormat::Depth32Float => TextureFormat::Depth32Float,
        _ => return None,
    })
}

fn convert_address_mode(value: AddressMode) -> wgpu::AddressMode {
    match value {
        AddressMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        AddressMode::Repeat => wgpu::AddressMode::Repeat,
        AddressMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
    }
}

fn convert_filter_mode(value: FilterMode) -> wgpu::FilterMode {
    match value {
        FilterMode::Nearest => wgpu::FilterMode::Nearest,
        FilterMode::Linear => wgpu::FilterMode::Linear,
    }
}

fn convert_mipmap_filter_mode(value: FilterMode) -> wgpu::MipmapFilterMode {
    match value {
        FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
        FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
    }
}

fn convert_map_mode(value: MapMode) -> wgpu::MapMode {
    match value {
        MapMode::Read => wgpu::MapMode::Read,
        MapMode::Write => wgpu::MapMode::Write,
    }
}

fn convert_shader_stages(value: ShaderStages) -> wgpu::ShaderStages {
    let mut stages = wgpu::ShaderStages::empty();
    if value.contains(ShaderStages::VERTEX) {
        stages |= wgpu::ShaderStages::VERTEX;
    }
    if value.contains(ShaderStages::FRAGMENT) {
        stages |= wgpu::ShaderStages::FRAGMENT;
    }
    if value.contains(ShaderStages::COMPUTE) {
        stages |= wgpu::ShaderStages::COMPUTE;
    }
    stages
}

fn convert_binding_type(value: &BindingType) -> wgpu::BindingType {
    match value {
        BindingType::Buffer {
            ty,
            has_dynamic_offset,
            min_binding_size,
        } => wgpu::BindingType::Buffer {
            ty: match ty {
                BufferBindingType::Uniform => wgpu::BufferBindingType::Uniform,
                BufferBindingType::ReadOnlyStorage => {
                    wgpu::BufferBindingType::Storage { read_only: true }
                }
                BufferBindingType::Storage => wgpu::BufferBindingType::Storage { read_only: false },
            },
            has_dynamic_offset: *has_dynamic_offset,
            min_binding_size: *min_binding_size,
        },
        BindingType::Sampler(ty) => wgpu::BindingType::Sampler(match ty {
            SamplerBindingType::Filtering => wgpu::SamplerBindingType::Filtering,
            SamplerBindingType::NonFiltering => wgpu::SamplerBindingType::NonFiltering,
            SamplerBindingType::Comparison => wgpu::SamplerBindingType::Comparison,
        }),
        BindingType::Texture {
            sample_type,
            view_dimension,
            multisampled,
        } => wgpu::BindingType::Texture {
            sample_type: match sample_type {
                TextureSampleType::Float => wgpu::TextureSampleType::Float { filterable: true },
                TextureSampleType::UnfilterableFloat => {
                    wgpu::TextureSampleType::Float { filterable: false }
                }
                TextureSampleType::Sint => wgpu::TextureSampleType::Sint,
                TextureSampleType::Uint => wgpu::TextureSampleType::Uint,
                TextureSampleType::Depth => wgpu::TextureSampleType::Depth,
            },
            view_dimension: match view_dimension {
                TextureViewDimension::D1 => wgpu::TextureViewDimension::D1,
                TextureViewDimension::D2 => wgpu::TextureViewDimension::D2,
                TextureViewDimension::D2Array => wgpu::TextureViewDimension::D2Array,
                TextureViewDimension::Cube => wgpu::TextureViewDimension::Cube,
                TextureViewDimension::CubeArray => wgpu::TextureViewDimension::CubeArray,
                TextureViewDimension::D3 => wgpu::TextureViewDimension::D3,
            },
            multisampled: *multisampled,
        },
    }
}

fn convert_vertex_format(value: VertexFormat) -> wgpu::VertexFormat {
    match value {
        VertexFormat::Float32x2 => wgpu::VertexFormat::Float32x2,
        VertexFormat::Float32x3 => wgpu::VertexFormat::Float32x3,
        VertexFormat::Float32x4 => wgpu::VertexFormat::Float32x4,
        VertexFormat::Unorm8x4 => wgpu::VertexFormat::Unorm8x4,
        _ => panic!("vertex format is not supported by the wgpu backend"),
    }
}

fn convert_vertex_step_mode(value: VertexStepMode) -> wgpu::VertexStepMode {
    match value {
        VertexStepMode::Vertex => wgpu::VertexStepMode::Vertex,
        VertexStepMode::Instance => wgpu::VertexStepMode::Instance,
    }
}

fn convert_primitive_topology(value: PrimitiveTopology) -> wgpu::PrimitiveTopology {
    match value {
        PrimitiveTopology::PointList => wgpu::PrimitiveTopology::PointList,
        PrimitiveTopology::LineList => wgpu::PrimitiveTopology::LineList,
        PrimitiveTopology::LineStrip => wgpu::PrimitiveTopology::LineStrip,
        PrimitiveTopology::TriangleList => wgpu::PrimitiveTopology::TriangleList,
        PrimitiveTopology::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
    }
}

fn convert_front_face(value: FrontFace) -> wgpu::FrontFace {
    match value {
        FrontFace::Ccw => wgpu::FrontFace::Ccw,
        FrontFace::Cw => wgpu::FrontFace::Cw,
    }
}

fn convert_face(value: Face) -> wgpu::Face {
    match value {
        Face::Front => wgpu::Face::Front,
        Face::Back => wgpu::Face::Back,
    }
}

fn convert_blend_component(value: BlendComponent) -> wgpu::BlendComponent {
    wgpu::BlendComponent {
        src_factor: match value.src_factor {
            BlendFactor::Zero => wgpu::BlendFactor::Zero,
            BlendFactor::One => wgpu::BlendFactor::One,
            BlendFactor::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
            BlendFactor::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
            BlendFactor::DstAlpha => wgpu::BlendFactor::DstAlpha,
            BlendFactor::OneMinusDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
        },
        dst_factor: match value.dst_factor {
            BlendFactor::Zero => wgpu::BlendFactor::Zero,
            BlendFactor::One => wgpu::BlendFactor::One,
            BlendFactor::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
            BlendFactor::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
            BlendFactor::DstAlpha => wgpu::BlendFactor::DstAlpha,
            BlendFactor::OneMinusDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
        },
        operation: match value.operation {
            BlendOperation::Add => wgpu::BlendOperation::Add,
            BlendOperation::Subtract => wgpu::BlendOperation::Subtract,
            BlendOperation::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
            BlendOperation::Min => wgpu::BlendOperation::Min,
            BlendOperation::Max => wgpu::BlendOperation::Max,
        },
    }
}

fn convert_compare_function(value: CompareFunction) -> wgpu::CompareFunction {
    match value {
        CompareFunction::Never => wgpu::CompareFunction::Never,
        CompareFunction::Less => wgpu::CompareFunction::Less,
        CompareFunction::Equal => wgpu::CompareFunction::Equal,
        CompareFunction::LessEqual => wgpu::CompareFunction::LessEqual,
        CompareFunction::Greater => wgpu::CompareFunction::Greater,
        CompareFunction::NotEqual => wgpu::CompareFunction::NotEqual,
        CompareFunction::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
        CompareFunction::Always => wgpu::CompareFunction::Always,
    }
}

fn convert_stencil_operation(value: StencilOperation) -> wgpu::StencilOperation {
    match value {
        StencilOperation::Keep => wgpu::StencilOperation::Keep,
        StencilOperation::Zero => wgpu::StencilOperation::Zero,
        StencilOperation::Replace => wgpu::StencilOperation::Replace,
        StencilOperation::IncrementClamp => wgpu::StencilOperation::IncrementClamp,
        StencilOperation::DecrementClamp => wgpu::StencilOperation::DecrementClamp,
        StencilOperation::Invert => wgpu::StencilOperation::Invert,
        StencilOperation::IncrementWrap => wgpu::StencilOperation::IncrementWrap,
        StencilOperation::DecrementWrap => wgpu::StencilOperation::DecrementWrap,
    }
}

fn convert_store_op(value: StoreOp) -> wgpu::StoreOp {
    match value {
        StoreOp::Store => wgpu::StoreOp::Store,
        StoreOp::Discard => wgpu::StoreOp::Discard,
    }
}

fn convert_present_mode(value: PresentMode) -> wgpu::PresentMode {
    match value {
        PresentMode::Fifo => wgpu::PresentMode::Fifo,
        PresentMode::Mailbox => wgpu::PresentMode::Mailbox,
        PresentMode::Immediate => wgpu::PresentMode::Immediate,
    }
}

fn convert_present_mode_from_wgpu(value: wgpu::PresentMode) -> Option<PresentMode> {
    Some(match value {
        wgpu::PresentMode::Fifo => PresentMode::Fifo,
        wgpu::PresentMode::Mailbox => PresentMode::Mailbox,
        wgpu::PresentMode::Immediate => PresentMode::Immediate,
        _ => return None,
    })
}

fn convert_alpha_mode(value: CompositeAlphaMode) -> wgpu::CompositeAlphaMode {
    match value {
        CompositeAlphaMode::Auto => wgpu::CompositeAlphaMode::Auto,
        CompositeAlphaMode::Opaque => wgpu::CompositeAlphaMode::Opaque,
        CompositeAlphaMode::PreMultiplied => wgpu::CompositeAlphaMode::PreMultiplied,
        CompositeAlphaMode::PostMultiplied => wgpu::CompositeAlphaMode::PostMultiplied,
    }
}

fn convert_alpha_mode_from_wgpu(value: wgpu::CompositeAlphaMode) -> Option<CompositeAlphaMode> {
    Some(match value {
        wgpu::CompositeAlphaMode::Auto => CompositeAlphaMode::Auto,
        wgpu::CompositeAlphaMode::Opaque => CompositeAlphaMode::Opaque,
        wgpu::CompositeAlphaMode::PreMultiplied => CompositeAlphaMode::PreMultiplied,
        wgpu::CompositeAlphaMode::PostMultiplied => CompositeAlphaMode::PostMultiplied,
        _ => return None,
    })
}
