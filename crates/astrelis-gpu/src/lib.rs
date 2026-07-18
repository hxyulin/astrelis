//! Backend-neutral GPU resources, commands, and presentation surfaces.

#![warn(missing_docs)]

mod types;

pub use types::*;

use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

/// Unstable contracts implemented by GPU backends.
pub mod backend;

static NEXT_DEVICE_ID: AtomicU64 = AtomicU64::new(1);

/// Monotonic queue submission identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubmissionId(pub u64);

/// Owned source of native window and display handles.
#[derive(Clone)]
pub struct SurfaceTarget {
    inner: Arc<dyn SurfaceTargetHandle>,
}

trait SurfaceTargetHandle: HasWindowHandle + HasDisplayHandle + fmt::Debug + Send + Sync {}
impl<T> SurfaceTargetHandle for T where
    T: HasWindowHandle + HasDisplayHandle + fmt::Debug + Send + Sync
{
}

impl SurfaceTarget {
    /// Retains a native handle provider for the lifetime of a surface.
    pub fn new<T>(target: T) -> Self
    where
        T: HasWindowHandle + HasDisplayHandle + fmt::Debug + Send + Sync + 'static,
    {
        Self {
            inner: Arc::new(target),
        }
    }
}

impl fmt::Debug for SurfaceTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SurfaceTarget")
            .finish_non_exhaustive()
    }
}

impl HasWindowHandle for SurfaceTarget {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle()
    }
}

impl HasDisplayHandle for SurfaceTarget {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.inner.display_handle()
    }
}

macro_rules! shared_handle {
    ($name:ident, $backend:path) => {
        #[doc = concat!("Backend-neutral `", stringify!($name), "` handle.")]
        #[derive(Clone)]
        pub struct $name {
            pub(crate) inner: Arc<dyn $backend>,
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .finish_non_exhaustive()
            }
        }
    };
}

shared_handle!(Instance, backend::Instance);
shared_handle!(Adapter, backend::Adapter);
shared_handle!(Device, backend::Device);
shared_handle!(Queue, backend::Queue);
shared_handle!(Surface, backend::Surface);
shared_handle!(Buffer, backend::Buffer);
shared_handle!(Texture, backend::Texture);
shared_handle!(TextureView, backend::TextureView);
shared_handle!(Sampler, backend::Sampler);
shared_handle!(ShaderModule, backend::ShaderModule);
shared_handle!(RenderPipeline, backend::RenderPipeline);
shared_handle!(QuerySet, backend::QuerySet);
shared_handle!(BindGroupLayout, backend::BindGroupLayout);
shared_handle!(PipelineLayout, backend::PipelineLayout);
shared_handle!(BindGroup, backend::BindGroup);
shared_handle!(ComputePipeline, backend::ComputePipeline);

impl BindGroup {
    /// Reports whether two handles refer to the same underlying bind group.
    ///
    /// This is a pointer-identity check on the shared backend storage, not a
    /// descriptor comparison: two bind groups built from equal descriptors are
    /// distinct resources and compare unequal. Callers use it to coalesce
    /// consecutive draws that reuse one bound resource.
    pub fn same_resource(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Instance {
    /// Wraps backend instance storage.
    pub fn from_backend(inner: Arc<dyn backend::Instance>) -> Self {
        Self { inner }
    }

    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn backend(&self) -> &dyn backend::Instance {
        self.inner.as_ref()
    }

    /// Creates a presentation surface.
    pub fn create_surface(&self, target: SurfaceTarget) -> Result<Surface, GpuError> {
        self.inner
            .create_surface(target)
            .map(|inner| Surface { inner })
    }

    /// Selects a compatible adapter.
    pub async fn request_adapter(
        &self,
        options: RequestAdapterOptions,
    ) -> Result<Adapter, GpuError> {
        self.inner
            .request_adapter(options)
            .await
            .map(|inner| Adapter { inner })
    }
}

impl Adapter {
    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn backend(&self) -> &dyn backend::Adapter {
        self.inner.as_ref()
    }

    /// Adapter metadata.
    pub fn info(&self) -> AdapterInfo {
        self.inner.info()
    }

    /// Supported features.
    pub fn features(&self) -> Features {
        self.inner.features()
    }

    /// Supported limits.
    pub fn limits(&self) -> Limits {
        self.inner.limits()
    }

    /// Requests a logical device and its queue.
    pub async fn request_device(
        &self,
        descriptor: DeviceDescriptor,
    ) -> Result<(Device, Queue), GpuError> {
        let missing = descriptor.required_features - self.features();
        if !missing.is_empty() {
            return Err(GpuError::new(format!(
                "adapter is missing required features: {missing:?}"
            )));
        }
        let id = DeviceId(NEXT_DEVICE_ID.fetch_add(1, Ordering::Relaxed));
        let (device, queue) = self.inner.request_device(id, descriptor).await?;
        Ok((Device { inner: device }, Queue { inner: queue }))
    }
}

impl Device {
    /// Wraps backend device storage.
    #[doc(hidden)]
    pub fn from_backend(inner: Arc<dyn backend::Device>) -> Self {
        Self { inner }
    }

    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn backend(&self) -> &dyn backend::Device {
        self.inner.as_ref()
    }

    /// Stable device identifier.
    pub fn id(&self) -> DeviceId {
        self.inner.id()
    }

    /// Enabled capabilities.
    pub fn capabilities(&self) -> DeviceCapabilities {
        self.inner.capabilities()
    }

    /// Installs a handler for asynchronous validation and device errors.
    pub fn set_error_handler(&self, handler: impl Fn(DeviceError) + Send + Sync + 'static) {
        self.inner.set_error_handler(Arc::new(handler));
    }

    /// Advances mapping, callbacks, and completed work.
    pub fn poll(&self, mode: PollMode) -> Result<(), GpuError> {
        self.inner.poll(mode)
    }

    /// Creates a buffer.
    pub fn create_buffer(&self, descriptor: BufferDescriptor) -> Buffer {
        Buffer {
            inner: self.inner.create_buffer(descriptor),
        }
    }

    /// Creates and initializes a buffer.
    pub fn create_buffer_init(
        &self,
        queue: &Queue,
        label: Option<String>,
        contents: &[u8],
        usage: BufferUsages,
    ) -> Result<Buffer, GpuError> {
        ensure_device(self.id(), queue.device_id())?;
        let buffer = self.create_buffer(BufferDescriptor {
            label,
            size: contents.len() as u64,
            usage: usage | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buffer, 0, contents)?;
        Ok(buffer)
    }

    /// Creates a texture.
    pub fn create_texture(&self, descriptor: TextureDescriptor) -> Texture {
        Texture {
            inner: self.inner.create_texture(descriptor),
        }
    }

    /// Creates a sampler.
    pub fn create_sampler(&self, descriptor: SamplerDescriptor) -> Sampler {
        Sampler {
            inner: self.inner.create_sampler(descriptor),
        }
    }

    /// Creates a WGSL shader module.
    pub fn create_shader_module(&self, descriptor: ShaderModuleDescriptor) -> ShaderModule {
        ShaderModule {
            inner: self.inner.create_shader_module(descriptor),
        }
    }

    /// Creates a bind-group layout.
    pub fn create_bind_group_layout(
        &self,
        descriptor: BindGroupLayoutDescriptor,
    ) -> BindGroupLayout {
        BindGroupLayout {
            inner: self.inner.create_bind_group_layout(descriptor),
        }
    }

    /// Creates a pipeline layout.
    pub fn create_pipeline_layout(
        &self,
        descriptor: PipelineLayoutDescriptor,
    ) -> Result<PipelineLayout, GpuError> {
        for layout in &descriptor.bind_group_layouts {
            ensure_device(self.id(), layout.device_id())?;
        }
        Ok(PipelineLayout {
            inner: self.inner.create_pipeline_layout(descriptor),
        })
    }

    /// Creates a bind group.
    pub fn create_bind_group(
        &self,
        descriptor: BindGroupDescriptor,
    ) -> Result<BindGroup, GpuError> {
        ensure_device(self.id(), descriptor.layout.device_id())?;
        for entry in &descriptor.entries {
            let resource_device = match &entry.resource {
                BindingResource::Buffer(binding) => binding.buffer.device_id(),
                BindingResource::Sampler(sampler) => sampler.device_id(),
                BindingResource::TextureView(view) => view.device_id(),
            };
            ensure_device(self.id(), resource_device)?;
        }
        Ok(BindGroup {
            inner: self.inner.create_bind_group(descriptor),
        })
    }

    /// Creates a render pipeline.
    pub fn create_render_pipeline(
        &self,
        descriptor: RenderPipelineDescriptor,
    ) -> Result<RenderPipeline, GpuError> {
        ensure_device(self.id(), descriptor.vertex.module.device_id())?;
        if let Some(layout) = &descriptor.layout {
            ensure_device(self.id(), layout.device_id())?;
        }
        if let Some(fragment) = &descriptor.fragment {
            ensure_device(self.id(), fragment.module.device_id())?;
        }
        Ok(RenderPipeline {
            inner: self.inner.create_render_pipeline(descriptor),
        })
    }

    /// Creates a GPU query set.
    pub fn create_query_set(&self, descriptor: QuerySetDescriptor) -> QuerySet {
        QuerySet {
            inner: self.inner.create_query_set(descriptor),
        }
    }

    /// Creates a compute pipeline.
    pub fn create_compute_pipeline(
        &self,
        descriptor: ComputePipelineDescriptor,
    ) -> Result<ComputePipeline, GpuError> {
        ensure_device(self.id(), descriptor.module.device_id())?;
        if let Some(layout) = &descriptor.layout {
            ensure_device(self.id(), layout.device_id())?;
        }
        Ok(ComputePipeline {
            inner: self.inner.create_compute_pipeline(descriptor),
        })
    }

    /// Starts command recording.
    pub fn create_command_encoder(&self, descriptor: CommandEncoderDescriptor) -> CommandEncoder {
        CommandEncoder {
            inner: Some(self.inner.create_command_encoder(descriptor)),
        }
    }
}

impl Queue {
    /// Wraps backend queue storage.
    #[doc(hidden)]
    pub fn from_backend(inner: Arc<dyn backend::Queue>) -> Self {
        Self { inner }
    }

    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn backend(&self) -> &dyn backend::Queue {
        self.inner.as_ref()
    }

    /// Stable identifier of the associated device.
    pub fn device_id(&self) -> DeviceId {
        self.inner.device_id()
    }

    /// Nanoseconds represented by one timestamp query tick.
    pub fn timestamp_period(&self) -> f32 {
        self.inner.timestamp_period()
    }

    /// Writes bytes into a buffer.
    pub fn write_buffer(&self, buffer: &Buffer, offset: u64, data: &[u8]) -> Result<(), GpuError> {
        ensure_device(self.device_id(), buffer.device_id())?;
        self.inner.write_buffer(buffer.inner.as_ref(), offset, data)
    }

    /// Uploads CPU bytes into a texture.
    pub fn write_texture(
        &self,
        destination: &TextureCopy,
        data: &[u8],
        layout: TextureDataLayout,
        extent: Extent3d,
    ) -> Result<(), GpuError> {
        ensure_device(self.device_id(), destination.texture.device_id())?;
        self.inner.write_texture(destination, data, layout, extent)
    }

    /// Submits command buffers, consuming them.
    pub fn submit(
        &self,
        command_buffers: impl IntoIterator<Item = CommandBuffer>,
    ) -> Result<SubmissionId, GpuError> {
        let mut native = Vec::new();
        for mut command_buffer in command_buffers {
            ensure_device(self.device_id(), command_buffer.device_id)?;
            native.push(
                command_buffer
                    .inner
                    .take()
                    .ok_or_else(|| GpuError::new("command buffer was already submitted"))?,
            );
        }
        let id = self.inner.submit(native)?;
        Ok(id)
    }
}

impl Surface {
    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn backend(&self) -> &dyn backend::Surface {
        self.inner.as_ref()
    }

    /// Returns presentation capabilities for an adapter.
    pub fn capabilities(&self, adapter: &Adapter) -> Result<SurfaceCapabilities, GpuError> {
        self.inner.capabilities(adapter.inner.as_ref())
    }

    /// Configures presentation.
    pub fn configure(
        &self,
        device: &Device,
        configuration: SurfaceConfiguration,
    ) -> Result<(), GpuError> {
        if configuration.width == 0 || configuration.height == 0 {
            return Err(GpuError::new(
                "cannot configure a surface with a zero-sized extent",
            ));
        }
        self.inner.configure(device.inner.as_ref(), configuration)
    }

    /// Acquires the next presentation frame.
    pub fn acquire(&self) -> Result<SurfaceFrameStatus, GpuError> {
        self.inner.acquire()
    }
}

/// An acquired presentation frame.
pub struct SurfaceFrame {
    inner: Option<Box<dyn backend::SurfaceFrame>>,
}

impl SurfaceFrame {
    /// Wraps a backend frame.
    pub fn from_backend(inner: Box<dyn backend::SurfaceFrame>) -> Self {
        Self { inner: Some(inner) }
    }

    /// Returns the frame texture.
    pub fn texture(&self) -> Texture {
        Texture {
            inner: self
                .inner
                .as_ref()
                .expect("frame already presented")
                .texture(),
        }
    }

    /// Presents and consumes the frame.
    pub fn present(mut self) -> Result<(), GpuError> {
        self.inner
            .take()
            .ok_or_else(|| GpuError::new("frame already presented"))?
            .present()
    }
}

impl fmt::Debug for SurfaceFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SurfaceFrame")
            .finish_non_exhaustive()
    }
}

impl Buffer {
    /// Stable identifier of the owning device.
    pub fn device_id(&self) -> DeviceId {
        self.inner.device_id()
    }

    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn inner_backend(&self) -> &dyn backend::Buffer {
        self.inner.as_ref()
    }

    /// Buffer size in bytes.
    pub fn size(&self) -> u64 {
        self.inner.size()
    }

    /// Starts a mapping operation. The device must be polled for progress.
    pub fn map_async(
        &self,
        mode: MapMode,
        range: std::ops::Range<u64>,
    ) -> backend::BackendFuture<Result<(), GpuError>> {
        self.inner.map_async(mode, range)
    }

    /// Copies bytes from a mapped range.
    pub fn read_mapped(&self, range: std::ops::Range<u64>) -> Result<Vec<u8>, GpuError> {
        self.inner.read_mapped(range)
    }

    /// Copies bytes into a mapped range.
    pub fn write_mapped(&self, offset: u64, data: &[u8]) -> Result<(), GpuError> {
        self.inner.write_mapped(offset, data)
    }

    /// Unmaps this buffer.
    pub fn unmap(&self) {
        self.inner.unmap();
    }
}

impl Sampler {
    /// Stable identifier of the owning device.
    pub fn device_id(&self) -> DeviceId {
        self.inner.device_id()
    }

    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn backend(&self) -> &dyn backend::Sampler {
        self.inner.as_ref()
    }
}

impl Texture {
    /// Stable identifier of the owning device.
    pub fn device_id(&self) -> DeviceId {
        self.inner.device_id()
    }

    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn backend(&self) -> &dyn backend::Texture {
        self.inner.as_ref()
    }

    /// Creates a view.
    pub fn create_view(&self, descriptor: TextureViewDescriptor) -> TextureView {
        TextureView {
            inner: self.inner.create_view(descriptor),
        }
    }
}

impl TextureView {
    /// Stable identifier of the owning device.
    pub fn device_id(&self) -> DeviceId {
        self.inner.device_id()
    }

    /// Texture sample count represented by this view.
    pub fn sample_count(&self) -> u32 {
        self.inner.sample_count()
    }

    /// Texture dimension represented by this view.
    pub fn dimension(&self) -> TextureDimension {
        self.inner.dimension()
    }

    /// Pixel format represented by this view.
    pub fn format(&self) -> TextureFormat {
        self.inner.format()
    }

    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn backend(&self) -> &dyn backend::TextureView {
        self.inner.as_ref()
    }
}

impl ShaderModule {
    /// Stable identifier of the owning device.
    pub fn device_id(&self) -> DeviceId {
        self.inner.device_id()
    }

    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn backend(&self) -> &dyn backend::ShaderModule {
        self.inner.as_ref()
    }
}

impl RenderPipeline {
    /// Stable identifier of the owning device.
    pub fn device_id(&self) -> DeviceId {
        self.inner.device_id()
    }

    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn backend(&self) -> &dyn backend::RenderPipeline {
        self.inner.as_ref()
    }
}

impl QuerySet {
    /// Stable identifier of the owning device.
    pub fn device_id(&self) -> DeviceId {
        self.inner.device_id()
    }

    /// Borrows unstable backend storage.
    #[doc(hidden)]
    pub fn backend(&self) -> &dyn backend::QuerySet {
        self.inner.as_ref()
    }
}

macro_rules! device_owned_handle {
    ($name:ident, $backend:path) => {
        impl $name {
            /// Stable identifier of the owning device.
            pub fn device_id(&self) -> DeviceId {
                self.inner.device_id()
            }

            /// Borrows unstable backend storage.
            #[doc(hidden)]
            pub fn backend(&self) -> &dyn $backend {
                self.inner.as_ref()
            }
        }
    };
}

device_owned_handle!(BindGroupLayout, backend::BindGroupLayout);
device_owned_handle!(PipelineLayout, backend::PipelineLayout);
device_owned_handle!(BindGroup, backend::BindGroup);
device_owned_handle!(ComputePipeline, backend::ComputePipeline);

/// Mutable command recorder.
pub struct CommandEncoder {
    inner: Option<Box<dyn backend::CommandEncoder>>,
}

impl CommandEncoder {
    /// Borrows unstable mutable backend storage.
    #[doc(hidden)]
    pub fn backend_mut(&mut self) -> Result<&mut (dyn backend::CommandEncoder + '_), GpuError> {
        match self.inner.as_mut() {
            Some(inner) => Ok(inner.as_mut()),
            None => Err(GpuError::new("encoder was already finished")),
        }
    }

    /// Begins a borrowed render pass.
    pub fn begin_render_pass(
        &mut self,
        descriptor: RenderPassDescriptor,
    ) -> Result<RenderPass<'_>, GpuError> {
        let id = self
            .inner
            .as_ref()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?
            .device_id();
        for attachment in descriptor.color_attachments.iter().flatten() {
            ensure_device(id, attachment.view.device_id())?;
            if let Some(resolve) = &attachment.resolve_target {
                ensure_device(id, resolve.device_id())?;
            }
        }
        if let Some(attachment) = &descriptor.depth_stencil_attachment {
            ensure_device(id, attachment.view.device_id())?;
        }
        if let Some(writes) = &descriptor.timestamp_writes {
            ensure_device(id, writes.query_set.device_id())?;
        }
        self.inner
            .as_mut()
            .expect("checked above")
            .begin_render_pass(descriptor)
            .map(|inner| RenderPass { id, inner })
    }

    /// Begins a borrowed compute pass.
    pub fn begin_compute_pass(
        &mut self,
        descriptor: ComputePassDescriptor,
    ) -> Result<ComputePass<'_>, GpuError> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?;
        let id = inner.device_id();
        inner
            .begin_compute_pass(descriptor)
            .map(|inner| ComputePass { id, inner })
    }

    /// Records an attachment-only render pass.
    pub fn render_pass(&mut self, descriptor: RenderPassDescriptor) -> Result<(), GpuError> {
        drop(self.begin_render_pass(descriptor)?);
        Ok(())
    }

    /// Copies bytes between buffers.
    pub fn copy_buffer_to_buffer(
        &mut self,
        source: &Buffer,
        source_offset: u64,
        destination: &Buffer,
        destination_offset: u64,
        size: u64,
    ) -> Result<(), GpuError> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?;
        ensure_device(inner.device_id(), source.device_id())?;
        ensure_device(inner.device_id(), destination.device_id())?;
        inner.copy_buffer_to_buffer(
            source.inner.as_ref(),
            source_offset,
            destination.inner.as_ref(),
            destination_offset,
            size,
        )
    }

    /// Copies texture texels into a buffer.
    pub fn copy_texture_to_buffer(
        &mut self,
        source: &TextureCopy,
        destination: &BufferTextureCopy,
        extent: Extent3d,
    ) -> Result<(), GpuError> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?;
        ensure_device(inner.device_id(), source.texture.device_id())?;
        ensure_device(inner.device_id(), destination.buffer.device_id())?;
        inner.copy_texture_to_buffer(source, destination, extent)
    }

    /// Resolves GPU query values into a buffer.
    pub fn resolve_query_set(
        &mut self,
        query_set: &QuerySet,
        queries: std::ops::Range<u32>,
        destination: &Buffer,
        destination_offset: u64,
    ) -> Result<(), GpuError> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?;
        ensure_device(inner.device_id(), query_set.device_id())?;
        ensure_device(inner.device_id(), destination.device_id())?;
        inner.resolve_query_set(
            query_set.backend(),
            queries,
            destination.inner_backend(),
            destination_offset,
        )
    }

    /// Pushes a backend debug group.
    pub fn push_debug_group(&mut self, label: &str) {
        if let Some(inner) = &mut self.inner {
            inner.push_debug_group(label);
        }
    }

    /// Pops a backend debug group.
    pub fn pop_debug_group(&mut self) {
        if let Some(inner) = &mut self.inner {
            inner.pop_debug_group();
        }
    }

    /// Finishes recording.
    pub fn finish(mut self) -> Result<CommandBuffer, GpuError> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| GpuError::new("encoder was already finished"))?;
        let device_id = inner.device_id();
        Ok(CommandBuffer {
            inner: Some(inner.finish()?),
            device_id,
        })
    }
}

/// Commands recorded within a render pass.
pub struct RenderPass<'a> {
    id: DeviceId,
    inner: Box<dyn backend::RenderPass + 'a>,
}

impl RenderPass<'_> {
    /// Selects the active render pipeline.
    pub fn set_pipeline(&mut self, pipeline: &RenderPipeline) -> Result<(), GpuError> {
        ensure_device(self.id, pipeline.device_id())?;
        self.inner.set_pipeline(pipeline.backend())
    }

    /// Binds a vertex buffer range.
    pub fn set_vertex_buffer(
        &mut self,
        slot: u32,
        buffer: &Buffer,
        range: std::ops::Range<u64>,
    ) -> Result<(), GpuError> {
        ensure_device(self.id, buffer.device_id())?;
        self.inner
            .set_vertex_buffer(slot, buffer.inner_backend(), range)
    }

    /// Binds an index buffer range.
    pub fn set_index_buffer(
        &mut self,
        buffer: &Buffer,
        range: std::ops::Range<u64>,
        format: IndexFormat,
    ) -> Result<(), GpuError> {
        ensure_device(self.id, buffer.device_id())?;
        self.inner
            .set_index_buffer(buffer.inner_backend(), range, format)
    }

    /// Binds resources at a bind-group index.
    pub fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &BindGroup,
        dynamic_offsets: &[u32],
    ) -> Result<(), GpuError> {
        ensure_device(self.id, bind_group.device_id())?;
        self.inner
            .set_bind_group(index, bind_group.backend(), dynamic_offsets)
    }

    /// Draws vertices and instances.
    pub fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        self.inner.draw(vertices, instances);
    }

    /// Draws indexed vertices and instances.
    pub fn draw_indexed(
        &mut self,
        indices: std::ops::Range<u32>,
        base_vertex: i32,
        instances: std::ops::Range<u32>,
    ) {
        self.inner.draw_indexed(indices, base_vertex, instances);
    }

    /// Sets the rasterization scissor rectangle.
    pub fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.inner.set_scissor_rect(x, y, width, height);
    }

    /// Sets the rasterization viewport and depth range.
    pub fn set_viewport(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) {
        self.inner
            .set_viewport(x, y, width, height, min_depth, max_depth);
    }

    /// Sets the dynamic stencil reference.
    pub fn set_stencil_reference(&mut self, reference: u32) {
        self.inner.set_stencil_reference(reference);
    }
}

/// Commands recorded within a compute pass.
pub struct ComputePass<'a> {
    id: DeviceId,
    inner: Box<dyn backend::ComputePass + 'a>,
}

impl ComputePass<'_> {
    /// Selects the active compute pipeline.
    pub fn set_pipeline(&mut self, pipeline: &ComputePipeline) -> Result<(), GpuError> {
        ensure_device(self.id, pipeline.device_id())?;
        self.inner.set_pipeline(pipeline.backend())
    }

    /// Binds resources at a bind-group index.
    pub fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &BindGroup,
        dynamic_offsets: &[u32],
    ) -> Result<(), GpuError> {
        ensure_device(self.id, bind_group.device_id())?;
        self.inner
            .set_bind_group(index, bind_group.backend(), dynamic_offsets)
    }

    /// Dispatches compute workgroups.
    pub fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32) {
        self.inner.dispatch_workgroups(x, y, z);
    }
}

impl fmt::Debug for ComputePass<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComputePass")
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for RenderPass<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("RenderPass").finish_non_exhaustive()
    }
}

impl fmt::Debug for CommandEncoder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandEncoder")
            .finish_non_exhaustive()
    }
}

/// Finished, single-submission command buffer.
pub struct CommandBuffer {
    inner: Option<Box<dyn backend::CommandBuffer>>,
    device_id: DeviceId,
}

impl fmt::Debug for CommandBuffer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandBuffer")
            .field("device_id", &self.device_id)
            .finish()
    }
}

fn ensure_device(expected: DeviceId, actual: DeviceId) -> Result<(), GpuError> {
    if expected == actual {
        Ok(())
    } else {
        Err(GpuError::new(format!(
            "resource belongs to {actual:?}, expected {expected:?}"
        )))
    }
}

/// Allocates an identity for a backend-supplied device.
#[doc(hidden)]
pub fn allocate_device_id() -> DeviceId {
    DeviceId(NEXT_DEVICE_ID.fetch_add(1, Ordering::Relaxed))
}
