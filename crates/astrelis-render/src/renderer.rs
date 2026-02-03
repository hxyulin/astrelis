use astrelis_core::profiling::profile_function;

use crate::context::GraphicsContext;
use crate::types::{GpuTexture, TypedBuffer, UniformBuffer};
use std::sync::Arc;

/// Low-level extensible renderer that simplifies WGPU resource management.
///
/// This provides a foundation for higher-level renderers like TextRenderer, SceneRenderer, etc.
/// It manages common rendering state and provides utilities for resource creation.
pub struct Renderer {
    context: Arc<GraphicsContext>,
}

impl Renderer {
    /// Create a new renderer with the given graphics context.
    pub fn new(context: Arc<GraphicsContext>) -> Self {
        Self { context }
    }

    /// Get the graphics context.
    pub fn context(&self) -> &GraphicsContext {
        &self.context
    }

    /// Get the device.
    pub fn device(&self) -> &wgpu::Device {
        self.context.device()
    }

    /// Get the queue.
    pub fn queue(&self) -> &wgpu::Queue {
        self.context.queue()
    }

    /// Create a shader module from WGSL source.
    pub fn create_shader(&self, label: Option<&str>, source: &str) -> wgpu::ShaderModule {
        profile_function!();
        self.context
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            })
    }

    /// Create a vertex buffer with data.
    pub fn create_vertex_buffer<T: bytemuck::Pod>(
        &self,
        label: Option<&str>,
        data: &[T],
    ) -> wgpu::Buffer {
        profile_function!();
        let buffer = self.context.device().create_buffer(&wgpu::BufferDescriptor {
            label,
            size: std::mem::size_of_val(data) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.context
            .queue()
            .write_buffer(&buffer, 0, bytemuck::cast_slice(data));

        buffer
    }

    /// Create an index buffer with data.
    pub fn create_index_buffer<T: bytemuck::Pod>(
        &self,
        label: Option<&str>,
        data: &[T],
    ) -> wgpu::Buffer {
        profile_function!();
        let buffer = self.context.device().create_buffer(&wgpu::BufferDescriptor {
            label,
            size: std::mem::size_of_val(data) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.context
            .queue()
            .write_buffer(&buffer, 0, bytemuck::cast_slice(data));

        buffer
    }

    /// Create a uniform buffer with data.
    pub fn create_uniform_buffer<T: bytemuck::Pod>(
        &self,
        label: Option<&str>,
        data: &T,
    ) -> wgpu::Buffer {
        profile_function!();
        let buffer = self.context.device().create_buffer(&wgpu::BufferDescriptor {
            label,
            size: std::mem::size_of::<T>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.context.queue().write_buffer(
            &buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(data)),
        );

        buffer
    }

    /// Update a uniform buffer with new data.
    pub fn update_uniform_buffer<T: bytemuck::Pod>(&self, buffer: &wgpu::Buffer, data: &T) {
        self.context.queue().write_buffer(
            buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(data)),
        );
    }

    /// Create an empty storage buffer.
    ///
    /// # Arguments
    ///
    /// * `label` - Optional debug label
    /// * `size` - Size in bytes
    /// * `read_only` - If true, creates a read-only storage buffer (STORAGE),
    ///   otherwise creates a read-write storage buffer (STORAGE | COPY_DST)
    pub fn create_storage_buffer(
        &self,
        label: Option<&str>,
        size: u64,
        read_only: bool,
    ) -> wgpu::Buffer {
        profile_function!();
        let usage = if read_only {
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST
        } else {
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
        };

        self.context.device().create_buffer(&wgpu::BufferDescriptor {
            label,
            size,
            usage,
            mapped_at_creation: false,
        })
    }

    /// Create a storage buffer initialized with data.
    ///
    /// # Arguments
    ///
    /// * `label` - Optional debug label
    /// * `data` - Initial data to write to the buffer
    /// * `read_only` - If true, creates a read-only storage buffer,
    ///   otherwise creates a read-write storage buffer
    pub fn create_storage_buffer_init<T: bytemuck::Pod>(
        &self,
        label: Option<&str>,
        data: &[T],
        read_only: bool,
    ) -> wgpu::Buffer {
        let usage = if read_only {
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST
        } else {
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
        };

        let buffer = self.context.device().create_buffer(&wgpu::BufferDescriptor {
            label,
            size: std::mem::size_of_val(data) as u64,
            usage,
            mapped_at_creation: false,
        });

        self.context
            .queue()
            .write_buffer(&buffer, 0, bytemuck::cast_slice(data));

        buffer
    }

    /// Update a storage buffer with new data at the specified offset.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to update
    /// * `offset` - Byte offset into the buffer
    /// * `data` - Data to write
    pub fn update_storage_buffer<T: bytemuck::Pod>(
        &self,
        buffer: &wgpu::Buffer,
        offset: u64,
        data: &[T],
    ) {
        self.context
            .queue()
            .write_buffer(buffer, offset, bytemuck::cast_slice(data));
    }

    /// Create a texture with descriptor.
    pub fn create_texture(&self, descriptor: &wgpu::TextureDescriptor) -> wgpu::Texture {
        self.context.device().create_texture(descriptor)
    }

    /// Create a 2D texture with data.
    pub fn create_texture_2d(
        &self,
        label: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        data: &[u8],
    ) -> wgpu::Texture {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = self
            .context
            .device()
            .create_texture(&wgpu::TextureDescriptor {
                label,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: usage | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

        let bytes_per_pixel = format.block_copy_size(None).unwrap();

        self.context.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * bytes_per_pixel),
                rows_per_image: Some(height),
            },
            size,
        );

        texture
    }

    /// Create a sampler with descriptor.
    pub fn create_sampler(&self, descriptor: &wgpu::SamplerDescriptor) -> wgpu::Sampler {
        self.context.device().create_sampler(descriptor)
    }

    /// Create a simple linear sampler.
    pub fn create_linear_sampler(&self, label: Option<&str>) -> wgpu::Sampler {
        self.context
            .device()
            .create_sampler(&wgpu::SamplerDescriptor {
                label,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            })
    }

    /// Create a simple nearest sampler.
    pub fn create_nearest_sampler(&self, label: Option<&str>) -> wgpu::Sampler {
        self.context
            .device()
            .create_sampler(&wgpu::SamplerDescriptor {
                label,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            })
    }

    /// Create a bind group layout.
    pub fn create_bind_group_layout(
        &self,
        label: Option<&str>,
        entries: &[wgpu::BindGroupLayoutEntry],
    ) -> wgpu::BindGroupLayout {
        profile_function!();
        self.context
            .device()
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label, entries })
    }

    /// Create a bind group.
    pub fn create_bind_group(
        &self,
        label: Option<&str>,
        layout: &wgpu::BindGroupLayout,
        entries: &[wgpu::BindGroupEntry],
    ) -> wgpu::BindGroup {
        profile_function!();
        self.context
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label,
                layout,
                entries,
            })
    }

    /// Create a pipeline layout.
    pub fn create_pipeline_layout(
        &self,
        label: Option<&str>,
        bind_group_layouts: &[&wgpu::BindGroupLayout],
        push_constant_ranges: &[wgpu::PushConstantRange],
    ) -> wgpu::PipelineLayout {
        profile_function!();
        self.context
            .device()
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label,
                bind_group_layouts,
                push_constant_ranges,
            })
    }

    /// Create a render pipeline.
    pub fn create_render_pipeline(
        &self,
        descriptor: &wgpu::RenderPipelineDescriptor,
    ) -> wgpu::RenderPipeline {
        profile_function!();
        self.context.device().create_render_pipeline(descriptor)
    }

    /// Create a compute pipeline.
    pub fn create_compute_pipeline(
        &self,
        descriptor: &wgpu::ComputePipelineDescriptor,
    ) -> wgpu::ComputePipeline {
        profile_function!();
        self.context.device().create_compute_pipeline(descriptor)
    }

    /// Create a command encoder.
    pub fn create_command_encoder(&self, label: Option<&str>) -> wgpu::CommandEncoder {
        self.context
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label })
    }

    /// Submit command buffers to the queue.
    pub fn submit<I>(&self, command_buffers: I)
    where
        I: IntoIterator<Item = wgpu::CommandBuffer>,
    {
        self.context.queue().submit(command_buffers);
    }

    // =========================================================================
    // Typed Buffer Methods
    // =========================================================================

    /// Create a typed vertex buffer with data.
    ///
    /// Returns a `TypedBuffer<T>` that tracks element count and provides type-safe operations.
    pub fn create_typed_vertex_buffer<T: bytemuck::Pod>(
        &self,
        label: Option<&str>,
        data: &[T],
    ) -> TypedBuffer<T> {
        TypedBuffer::new(
            self.context.device(),
            label,
            data,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        )
    }

    /// Create a typed index buffer with data.
    ///
    /// Returns a `TypedBuffer<T>` that tracks element count and provides type-safe operations.
    pub fn create_typed_index_buffer<T: bytemuck::Pod>(
        &self,
        label: Option<&str>,
        data: &[T],
    ) -> TypedBuffer<T> {
        TypedBuffer::new(
            self.context.device(),
            label,
            data,
            wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        )
    }

    /// Create a typed uniform buffer with data.
    ///
    /// Returns a `UniformBuffer<T>` that provides type-safe uniform operations.
    pub fn create_typed_uniform<T: bytemuck::Pod>(
        &self,
        label: Option<&str>,
        data: &T,
    ) -> UniformBuffer<T> {
        UniformBuffer::new_uniform(self.context.device(), label, data)
    }

    /// Create a GPU texture with cached view and metadata.
    ///
    /// Returns a `GpuTexture` that provides convenient access to the texture, view, and metadata.
    pub fn create_gpu_texture_2d(
        &self,
        label: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> GpuTexture {
        GpuTexture::new_2d(self.context.device(), label, width, height, format, usage)
    }

    /// Create a GPU texture from raw data.
    ///
    /// Returns a `GpuTexture` with data uploaded to the GPU.
    pub fn create_gpu_texture_from_data(
        &self,
        label: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        data: &[u8],
    ) -> GpuTexture {
        profile_function!();
        GpuTexture::from_data(
            self.context.device(),
            self.context.queue(),
            label,
            width,
            height,
            format,
            data,
        )
    }
}

/// Builder for creating a render pipeline with sensible defaults.
///
/// # Example
///
/// ```ignore
/// let pipeline = RenderPipelineBuilder::new(&renderer)
///     .label("My Pipeline")
///     .shader(&shader)
///     .layout(&layout)
///     .vertex_buffer(vertex_layout)
///     .color_target(wgpu::ColorTargetState {
///         format: surface_format,
///         blend: Some(wgpu::BlendState::REPLACE),
///         write_mask: wgpu::ColorWrites::ALL,
///     })
///     .build();
/// ```
pub struct RenderPipelineBuilder<'a> {
    renderer: &'a Renderer,
    label: Option<&'a str>,
    shader: Option<&'a wgpu::ShaderModule>,
    vertex_entry: &'a str,
    fragment_entry: &'a str,
    layout: Option<&'a wgpu::PipelineLayout>,
    vertex_buffers: Vec<wgpu::VertexBufferLayout<'a>>,
    color_targets: Vec<Option<wgpu::ColorTargetState>>,
    primitive: wgpu::PrimitiveState,
    depth_stencil: Option<wgpu::DepthStencilState>,
    multisample: wgpu::MultisampleState,
}

impl<'a> RenderPipelineBuilder<'a> {
    /// Create a new builder with default primitive, depth, and multisample state.
    pub fn new(renderer: &'a Renderer) -> Self {
        Self {
            renderer,
            label: None,
            shader: None,
            vertex_entry: "vs_main",
            fragment_entry: "fs_main",
            layout: None,
            vertex_buffers: Vec::new(),
            color_targets: Vec::new(),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        }
    }

    /// Set a debug label for the pipeline.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Set the shader module (required).
    pub fn shader(mut self, shader: &'a wgpu::ShaderModule) -> Self {
        self.shader = Some(shader);
        self
    }

    /// Set the vertex shader entry point. Defaults to `"vs_main"`.
    pub fn vertex_entry(mut self, entry: &'a str) -> Self {
        self.vertex_entry = entry;
        self
    }

    /// Set the fragment shader entry point. Defaults to `"fs_main"`.
    pub fn fragment_entry(mut self, entry: &'a str) -> Self {
        self.fragment_entry = entry;
        self
    }

    /// Set the pipeline layout (required).
    pub fn layout(mut self, layout: &'a wgpu::PipelineLayout) -> Self {
        self.layout = Some(layout);
        self
    }

    /// Add a vertex buffer layout. Can be called multiple times for multiple slots.
    pub fn vertex_buffer(mut self, layout: wgpu::VertexBufferLayout<'a>) -> Self {
        self.vertex_buffers.push(layout);
        self
    }

    /// Add a color target state. Can be called multiple times for MRT.
    pub fn color_target(mut self, target: wgpu::ColorTargetState) -> Self {
        self.color_targets.push(Some(target));
        self
    }

    /// Override the primitive state (topology, cull mode, etc.).
    pub fn primitive(mut self, primitive: wgpu::PrimitiveState) -> Self {
        self.primitive = primitive;
        self
    }

    /// Set the depth/stencil state. Disabled by default.
    pub fn depth_stencil(mut self, depth_stencil: wgpu::DepthStencilState) -> Self {
        self.depth_stencil = Some(depth_stencil);
        self
    }

    /// Override the multisample state. Defaults to 1 sample, no alpha-to-coverage.
    pub fn multisample(mut self, multisample: wgpu::MultisampleState) -> Self {
        self.multisample = multisample;
        self
    }

    /// Build the render pipeline.
    ///
    /// # Panics
    ///
    /// Panics if `shader` or `layout` has not been set.
    pub fn build(self) -> wgpu::RenderPipeline {
        let shader = self.shader.expect("Shader module is required");
        let layout = self.layout.expect("Pipeline layout is required");

        self.renderer
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: self.label,
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some(self.vertex_entry),
                    buffers: &self.vertex_buffers,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some(self.fragment_entry),
                    targets: &self.color_targets,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: self.primitive,
                depth_stencil: self.depth_stencil,
                multisample: self.multisample,
                multiview: None,
                cache: None,
            })
    }
}

/// Builder for creating a compute pipeline with common defaults.
///
/// # Example
///
/// ```ignore
/// let pipeline = ComputePipelineBuilder::new(&renderer)
///     .label("My Compute Pipeline")
///     .shader(&shader)
///     .entry("main")
///     .layout(&layout)
///     .build();
/// ```
pub struct ComputePipelineBuilder<'a> {
    renderer: &'a Renderer,
    label: Option<&'a str>,
    shader: Option<&'a wgpu::ShaderModule>,
    entry: &'a str,
    layout: Option<&'a wgpu::PipelineLayout>,
}

impl<'a> ComputePipelineBuilder<'a> {
    /// Create a new compute pipeline builder.
    pub fn new(renderer: &'a Renderer) -> Self {
        Self {
            renderer,
            label: None,
            shader: None,
            entry: "main",
            layout: None,
        }
    }

    /// Set a debug label for the pipeline.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Set the shader module.
    pub fn shader(mut self, shader: &'a wgpu::ShaderModule) -> Self {
        self.shader = Some(shader);
        self
    }

    /// Set the entry point function name.
    ///
    /// Defaults to "main".
    pub fn entry(mut self, entry: &'a str) -> Self {
        self.entry = entry;
        self
    }

    /// Set the pipeline layout.
    pub fn layout(mut self, layout: &'a wgpu::PipelineLayout) -> Self {
        self.layout = Some(layout);
        self
    }

    /// Build the compute pipeline.
    ///
    /// # Panics
    ///
    /// Panics if shader or layout is not set.
    pub fn build(self) -> wgpu::ComputePipeline {
        let shader = self.shader.expect("Shader module is required");
        let layout = self.layout.expect("Pipeline layout is required");

        self.renderer
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: self.label,
                layout: Some(layout),
                module: shader,
                entry_point: Some(self.entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
    }
}
