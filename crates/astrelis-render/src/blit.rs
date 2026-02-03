//! Texture blitting utilities for fullscreen quad rendering.
//!
//! This module provides a simple API for rendering textures to the screen,
//! useful for video backgrounds, post-processing, and image display.

use astrelis_core::profiling::profile_function;

use crate::capability::{GpuRequirements, RenderCapability};
use crate::context::GraphicsContext;
use crate::types::{GpuTexture, TypedBuffer};
use crate::Renderer;
use std::sync::Arc;

/// A renderer for blitting textures to the screen.
///
/// This provides an easy way to render a texture as a fullscreen quad,
/// useful for video backgrounds, splash screens, or post-processing effects.
///
/// # Example
///
/// ```ignore
/// let blit_renderer = BlitRenderer::new(context);
///
/// // In render loop:
/// blit_renderer.blit(&mut render_pass, &texture_view);
/// ```
impl RenderCapability for BlitRenderer {
    fn requirements() -> GpuRequirements {
        GpuRequirements::none()
    }

    fn name() -> &'static str {
        "BlitRenderer"
    }
}

pub struct BlitRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    vertex_buffer: TypedBuffer<f32>,
    context: Arc<GraphicsContext>,
}

impl BlitRenderer {
    /// Create a new blit renderer.
    ///
    /// # Arguments
    ///
    /// * `context` - The graphics context
    /// * `target_format` - The format of the render target (typically the surface format)
    pub fn new(context: Arc<GraphicsContext>, target_format: wgpu::TextureFormat) -> Self {
        Self::new_with_options(context, target_format, BlitOptions::default())
    }

    /// Create a new blit renderer with custom options.
    pub fn new_with_options(
        context: Arc<GraphicsContext>,
        target_format: wgpu::TextureFormat,
        options: BlitOptions,
    ) -> Self {
        profile_function!();
        let renderer = Renderer::new(context.clone());

        // Create shader
        let shader = renderer.create_shader(
            Some("Blit Shader"),
            include_str!("shaders/blit.wgsl"),
        );

        // Create sampler
        let sampler = context.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Blit Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: options.filter_mode,
            min_filter: options.filter_mode,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout =
            context
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Blit Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        // Create pipeline layout
        let pipeline_layout =
            context
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Blit Pipeline Layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        // Create pipeline
        let pipeline = context
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Blit Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 16,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                    }],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: options.blend_state,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
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
                multiview: None,
                cache: None,
            });

        // Create fullscreen quad vertex buffer
        #[rustfmt::skip]
        let vertices: [f32; 24] = [
            // Position (clip space)  UV
            -1.0, -1.0,               0.0, 1.0,
             1.0, -1.0,               1.0, 1.0,
             1.0,  1.0,               1.0, 0.0,
            -1.0, -1.0,               0.0, 1.0,
             1.0,  1.0,               1.0, 0.0,
            -1.0,  1.0,               0.0, 0.0,
        ];

        let vertex_buffer = renderer.create_typed_vertex_buffer(Some("Blit Vertex Buffer"), &vertices);

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            vertex_buffer,
            context,
        }
    }

    /// Create a bind group for a texture.
    ///
    /// You can cache this bind group if you're blitting the same texture repeatedly.
    pub fn create_bind_group(&self, texture_view: &wgpu::TextureView) -> wgpu::BindGroup {
        self.context
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Blit Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
    }

    /// Blit a texture to the render target as a fullscreen quad.
    ///
    /// # Arguments
    ///
    /// * `render_pass` - The render pass to draw to
    /// * `texture_view` - The texture to blit
    ///
    /// Note: This creates a new bind group each call. For better performance
    /// with frequently-blitted textures, use `create_bind_group` and `blit_with_bind_group`.
    pub fn blit(&self, render_pass: &mut wgpu::RenderPass, texture_view: &wgpu::TextureView) {
        profile_function!();
        let bind_group = self.create_bind_group(texture_view);
        self.blit_with_bind_group(render_pass, &bind_group);
    }

    /// Blit using a pre-created bind group.
    ///
    /// More efficient than `blit` when the same texture is blitted multiple times.
    pub fn blit_with_bind_group(
        &self,
        render_pass: &mut wgpu::RenderPass,
        bind_group: &wgpu::BindGroup,
    ) {
        render_pass.push_debug_group("BlitRenderer::blit");
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice());
        render_pass.draw(0..6, 0..1);
        render_pass.pop_debug_group();
    }

    /// Get the bind group layout for custom pipelines.
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}

/// Options for configuring the blit renderer.
#[derive(Debug, Clone)]
pub struct BlitOptions {
    /// Filter mode for texture sampling (Linear or Nearest)
    pub filter_mode: wgpu::FilterMode,
    /// Blend state for the blit operation
    pub blend_state: Option<wgpu::BlendState>,
}

impl Default for BlitOptions {
    fn default() -> Self {
        Self {
            filter_mode: wgpu::FilterMode::Linear,
            blend_state: Some(wgpu::BlendState::REPLACE),
        }
    }
}

impl BlitOptions {
    /// Create options for opaque blitting (no blending).
    pub fn opaque() -> Self {
        Self {
            filter_mode: wgpu::FilterMode::Linear,
            blend_state: Some(wgpu::BlendState::REPLACE),
        }
    }

    /// Create options for alpha-blended blitting.
    pub fn alpha_blend() -> Self {
        Self {
            filter_mode: wgpu::FilterMode::Linear,
            blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
        }
    }

    /// Create options for nearest-neighbor filtering (pixel art).
    pub fn nearest() -> Self {
        Self {
            filter_mode: wgpu::FilterMode::Nearest,
            blend_state: Some(wgpu::BlendState::REPLACE),
        }
    }

    /// Set the filter mode.
    pub fn with_filter(mut self, filter: wgpu::FilterMode) -> Self {
        self.filter_mode = filter;
        self
    }

    /// Set the blend state.
    pub fn with_blend(mut self, blend: Option<wgpu::BlendState>) -> Self {
        self.blend_state = blend;
        self
    }
}

/// Helper to upload texture data from CPU to GPU.
///
/// Useful for video frame upload or dynamic texture updates.
pub struct TextureUploader {
    /// GPU texture with cached view and metadata.
    texture: GpuTexture,
}

impl TextureUploader {
    /// Create a new texture uploader with the specified dimensions.
    ///
    /// # Arguments
    ///
    /// * `context` - The graphics context
    /// * `width` - Texture width in pixels
    /// * `height` - Texture height in pixels
    /// * `format` - Texture format (e.g., Rgba8UnormSrgb for standard images)
    pub fn new(
        context: &GraphicsContext,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let texture = GpuTexture::new_2d(
            context.device(),
            Some("Uploadable Texture"),
            width,
            height,
            format,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );

        Self { texture }
    }

    /// Upload pixel data to the texture.
    ///
    /// # Arguments
    ///
    /// * `context` - The graphics context
    /// * `data` - Raw pixel data (must match texture format and dimensions)
    pub fn upload(&self, context: &GraphicsContext, data: &[u8]) {
        use crate::extension::AsWgpu;
        let bytes_per_pixel = self.texture.format().block_copy_size(None).unwrap_or(4);
        let bytes_per_row = self.texture.width() * bytes_per_pixel;

        context.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self.texture.as_wgpu(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(self.texture.height()),
            },
            wgpu::Extent3d {
                width: self.texture.width(),
                height: self.texture.height(),
                depth_or_array_layers: 1,
            },
        );
    }

    /// Upload a subregion of the texture.
    ///
    /// # Arguments
    ///
    /// * `context` - The graphics context
    /// * `data` - Raw pixel data for the region
    /// * `x`, `y` - Top-left corner of the region
    /// * `width`, `height` - Dimensions of the region
    pub fn upload_region(
        &self,
        context: &GraphicsContext,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        use crate::extension::AsWgpu;
        let bytes_per_pixel = self.texture.format().block_copy_size(None).unwrap_or(4);
        let bytes_per_row = width * bytes_per_pixel;

        context.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self.texture.as_wgpu(),
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Resize the texture (creates a new texture internally).
    pub fn resize(&mut self, context: &GraphicsContext, width: u32, height: u32) {
        if self.texture.width() == width && self.texture.height() == height {
            return;
        }

        *self = Self::new(context, width, height, self.texture.format());
    }

    /// Get the texture view for rendering.
    pub fn view(&self) -> &wgpu::TextureView {
        self.texture.view()
    }

    /// Get the underlying texture.
    pub fn texture(&self) -> &wgpu::Texture {
        use crate::extension::AsWgpu;
        self.texture.as_wgpu()
    }

    /// Get the texture dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }

    /// Get the texture format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.texture.format()
    }
}
