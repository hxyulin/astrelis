//! Unstable contracts implemented by GPU backends.

use std::{any::Any, fmt::Debug, ops::Range, sync::Arc};

use crate::{
    AdapterInfo, BindGroupDescriptor, BindGroupLayoutDescriptor, BufferDescriptor,
    BufferTextureCopy, CommandEncoderDescriptor, ComputePassDescriptor, ComputePipelineDescriptor,
    DeviceCapabilities, DeviceDescriptor, DeviceError, DeviceId, Extent3d, Features, GpuError,
    Limits, MapMode, PipelineLayoutDescriptor, PollMode, QuerySetDescriptor, RenderPassDescriptor,
    RenderPipelineDescriptor, RequestAdapterOptions, SamplerDescriptor, ShaderModuleDescriptor,
    SurfaceCapabilities, SurfaceConfiguration, SurfaceFrameStatus, TextureCopy, TextureDataLayout,
    TextureDescriptor, TextureViewDescriptor,
};

/// Boxed backend future.
pub type BackendFuture<T> = std::pin::Pin<Box<dyn Future<Output = T> + Send>>;

/// Result of creating a backend device and its queue.
pub type DeviceQueueResult = Result<(Arc<dyn Device>, Arc<dyn Queue>), GpuError>;

/// Common native-access support.
pub trait NativeHandle: Any + Debug + Send + Sync {
    /// Returns this value for backend-specific downcasting.
    fn as_any(&self) -> &dyn Any;
}

/// Backend instance operations.
pub trait Instance: NativeHandle {
    /// Creates a presentation surface.
    fn create_surface(&self, target: crate::SurfaceTarget) -> Result<Arc<dyn Surface>, GpuError>;
    /// Selects an adapter.
    fn request_adapter(
        &self,
        options: RequestAdapterOptions,
    ) -> BackendFuture<Result<Arc<dyn Adapter>, GpuError>>;
}

/// Backend adapter operations.
pub trait Adapter: NativeHandle {
    /// Adapter metadata.
    fn info(&self) -> AdapterInfo;
    /// Supported features.
    fn features(&self) -> Features;
    /// Supported limits.
    fn limits(&self) -> Limits;
    /// Creates a logical device and queue.
    fn request_device(
        &self,
        id: DeviceId,
        descriptor: DeviceDescriptor,
    ) -> BackendFuture<DeviceQueueResult>;
}

/// Backend device and resource factory.
pub trait Device: NativeHandle {
    /// Stable device identifier.
    fn id(&self) -> DeviceId;
    /// Enabled capabilities.
    fn capabilities(&self) -> DeviceCapabilities;
    /// Installs an error callback.
    fn set_error_handler(&self, handler: Arc<dyn Fn(DeviceError) + Send + Sync>);
    /// Advances callbacks and mapping.
    fn poll(&self, mode: PollMode) -> Result<(), GpuError>;
    /// Creates a buffer.
    fn create_buffer(&self, descriptor: BufferDescriptor) -> Arc<dyn Buffer>;
    /// Creates a texture.
    fn create_texture(&self, descriptor: TextureDescriptor) -> Arc<dyn Texture>;
    /// Creates a sampler.
    fn create_sampler(&self, descriptor: SamplerDescriptor) -> Arc<dyn Sampler>;
    /// Creates a WGSL shader module.
    fn create_shader_module(&self, descriptor: ShaderModuleDescriptor) -> Arc<dyn ShaderModule>;
    /// Creates a bind-group layout.
    fn create_bind_group_layout(
        &self,
        descriptor: BindGroupLayoutDescriptor,
    ) -> Arc<dyn BindGroupLayout>;
    /// Creates a pipeline layout.
    fn create_pipeline_layout(
        &self,
        descriptor: PipelineLayoutDescriptor,
    ) -> Arc<dyn PipelineLayout>;
    /// Creates a populated bind group.
    fn create_bind_group(&self, descriptor: BindGroupDescriptor) -> Arc<dyn BindGroup>;
    /// Creates a render pipeline.
    fn create_render_pipeline(
        &self,
        descriptor: RenderPipelineDescriptor,
    ) -> Arc<dyn RenderPipeline>;
    /// Creates a query set.
    fn create_query_set(&self, descriptor: QuerySetDescriptor) -> Arc<dyn QuerySet>;
    /// Creates a compute pipeline.
    fn create_compute_pipeline(
        &self,
        descriptor: ComputePipelineDescriptor,
    ) -> Arc<dyn ComputePipeline>;
    /// Creates a command encoder.
    fn create_command_encoder(
        &self,
        descriptor: CommandEncoderDescriptor,
    ) -> Box<dyn CommandEncoder>;
}

/// Backend queue operations.
pub trait Queue: NativeHandle {
    /// Stable device identifier.
    fn device_id(&self) -> DeviceId;
    /// Nanoseconds represented by one timestamp query tick.
    fn timestamp_period(&self) -> f32;
    /// Writes bytes into a buffer.
    fn write_buffer(&self, buffer: &dyn Buffer, offset: u64, data: &[u8]) -> Result<(), GpuError>;
    /// Uploads bytes into a texture.
    fn write_texture(
        &self,
        destination: &TextureCopy,
        data: &[u8],
        layout: TextureDataLayout,
        extent: Extent3d,
    ) -> Result<(), GpuError>;
    /// Submits finished command buffers.
    fn submit(&self, buffers: Vec<Box<dyn CommandBuffer>>)
    -> Result<crate::SubmissionId, GpuError>;
}

/// Backend surface operations.
pub trait Surface: NativeHandle {
    /// Queries capabilities for an adapter.
    fn capabilities(&self, adapter: &dyn Adapter) -> Result<SurfaceCapabilities, GpuError>;
    /// Configures presentation.
    fn configure(
        &self,
        device: &dyn Device,
        configuration: SurfaceConfiguration,
    ) -> Result<(), GpuError>;
    /// Acquires the next frame.
    fn acquire(&self) -> Result<SurfaceFrameStatus, GpuError>;
}

/// Backend surface frame.
pub trait SurfaceFrame: NativeHandle {
    /// Frame texture.
    fn texture(&self) -> Arc<dyn Texture>;
    /// Presents the frame.
    fn present(self: Box<Self>) -> Result<(), GpuError>;
}

/// Backend buffer.
pub trait Buffer: NativeHandle {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
    /// Size in bytes.
    fn size(&self) -> u64;
    /// Starts an asynchronous mapping operation.
    fn map_async(&self, mode: MapMode, range: Range<u64>) -> BackendFuture<Result<(), GpuError>>;
    /// Copies mapped bytes into CPU-owned memory.
    fn read_mapped(&self, range: Range<u64>) -> Result<Vec<u8>, GpuError>;
    /// Copies bytes into a mapped buffer.
    fn write_mapped(&self, offset: u64, data: &[u8]) -> Result<(), GpuError>;
    /// Unmaps the buffer.
    fn unmap(&self);
}

/// Backend texture.
pub trait Texture: NativeHandle {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
    /// Creates a view.
    fn create_view(&self, descriptor: TextureViewDescriptor) -> Arc<dyn TextureView>;
}

/// Backend texture view.
pub trait TextureView: NativeHandle {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
}

/// Backend sampler.
pub trait Sampler: NativeHandle {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
}

/// Backend shader module.
pub trait ShaderModule: NativeHandle {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
}

/// Backend render pipeline.
pub trait RenderPipeline: NativeHandle {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
}

/// Backend bind-group layout.
pub trait BindGroupLayout: NativeHandle {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
}

/// Backend pipeline layout.
pub trait PipelineLayout: NativeHandle {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
}

/// Backend bind group.
pub trait BindGroup: NativeHandle {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
}

/// Backend compute pipeline.
pub trait ComputePipeline: NativeHandle {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
}

/// Backend query set.
pub trait QuerySet: NativeHandle {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
}

/// Backend command encoder.
pub trait CommandEncoder: Debug + Send {
    /// Returns this value for backend-specific mutable downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Owning device.
    fn device_id(&self) -> DeviceId;
    /// Begins a render pass.
    fn begin_render_pass<'a>(
        &'a mut self,
        descriptor: RenderPassDescriptor,
    ) -> Result<Box<dyn RenderPass + 'a>, GpuError>;
    /// Begins a compute pass.
    fn begin_compute_pass<'a>(
        &'a mut self,
        descriptor: ComputePassDescriptor,
    ) -> Result<Box<dyn ComputePass + 'a>, GpuError>;
    /// Copies one buffer into another.
    fn copy_buffer_to_buffer(
        &mut self,
        source: &dyn Buffer,
        source_offset: u64,
        destination: &dyn Buffer,
        destination_offset: u64,
        size: u64,
    ) -> Result<(), GpuError>;
    /// Copies texture texels into a buffer.
    fn copy_texture_to_buffer(
        &mut self,
        source: &TextureCopy,
        destination: &BufferTextureCopy,
        extent: Extent3d,
    ) -> Result<(), GpuError>;
    /// Resolves query values into a buffer.
    fn resolve_query_set(
        &mut self,
        query_set: &dyn QuerySet,
        queries: Range<u32>,
        destination: &dyn Buffer,
        destination_offset: u64,
    ) -> Result<(), GpuError>;
    /// Adds a debug group.
    fn push_debug_group(&mut self, label: &str);
    /// Removes a debug group.
    fn pop_debug_group(&mut self);
    /// Finishes recording.
    fn finish(self: Box<Self>) -> Result<Box<dyn CommandBuffer>, GpuError>;
}

/// Backend render pass commands.
pub trait RenderPass: Debug {
    /// Sets the render pipeline.
    fn set_pipeline(&mut self, pipeline: &dyn RenderPipeline) -> Result<(), GpuError>;
    /// Sets a vertex buffer slot.
    fn set_vertex_buffer(
        &mut self,
        slot: u32,
        buffer: &dyn Buffer,
        range: Range<u64>,
    ) -> Result<(), GpuError>;
    /// Sets a bind group.
    fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &dyn BindGroup,
        dynamic_offsets: &[u32],
    ) -> Result<(), GpuError>;
    /// Draws vertices and instances.
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>);
}

/// Backend compute pass commands.
pub trait ComputePass: Debug {
    /// Sets the compute pipeline.
    fn set_pipeline(&mut self, pipeline: &dyn ComputePipeline) -> Result<(), GpuError>;
    /// Sets a bind group.
    fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &dyn BindGroup,
        dynamic_offsets: &[u32],
    ) -> Result<(), GpuError>;
    /// Dispatches workgroups.
    fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32);
}

/// Backend command buffer.
pub trait CommandBuffer: Debug + Send {
    /// Owning device.
    fn device_id(&self) -> DeviceId;
    /// Converts this owned value for backend-specific downcasting.
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send>;
}
