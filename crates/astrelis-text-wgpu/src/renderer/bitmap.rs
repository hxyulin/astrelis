//! Bitmap text renderer.
//!
//! Uses a grayscale glyph atlas for crisp text at small sizes.

use astrelis_core::math::Vec2;
use astrelis_gpu_wgpu::WgpuDevice;
use astrelis_text::{FontSystem, Text};

use crate::atlas::bitmap::BitmapAtlas;
use crate::atlas::GlyphPlacement;
use crate::config::TextRendererConfig;

use super::vertex::TextVertex;
use super::{SharedContext, TextBuffer, orthographic_projection};

/// Bitmap-only text renderer (~8 MB with default atlas).
///
/// Best for small text (<24px) without effects. Uses an R8Unorm
/// texture atlas with grayscale glyph bitmaps.
pub struct BitmapTextRenderer {
    pub(crate) shared: SharedContext,
    pub(crate) atlas: BitmapAtlas,
    pub(crate) gpu_atlas_texture: wgpu::Texture,
    pub(crate) atlas_bind_group: wgpu::BindGroup,
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) vertices: Vec<TextVertex>,
    pub(crate) indices: Vec<u16>,
    pub(crate) config: TextRendererConfig,
}

impl BitmapTextRenderer {
    /// Create a new bitmap text renderer.
    pub fn new(
        device: &WgpuDevice,
        font_system: FontSystem,
        config: TextRendererConfig,
    ) -> Self {
        astrelis_profiling::profile_function!();
        let shared = SharedContext::new(device, font_system);
        let atlas_size = config.atlas_size;
        let atlas = BitmapAtlas::new(atlas_size);

        let dev = device.wgpu_device();

        // Create atlas texture
        let gpu_atlas_texture = dev.create_texture(&wgpu::TextureDescriptor {
            label: Some("bitmap_text_atlas"),
            size: wgpu::Extent3d {
                width: atlas_size,
                height: atlas_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let atlas_view = gpu_atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = dev.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("bitmap_text_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create atlas bind group layout
        let atlas_bind_group_layout =
            dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bitmap_text_atlas_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
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

        let atlas_bind_group = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bitmap_text_atlas_bind_group"),
            layout: &atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create pipeline
        let shader = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bitmap_text_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/text.wgsl").into()),
        });

        let pipeline_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bitmap_text_pipeline_layout"),
            bind_group_layouts: &[Some(&atlas_bind_group_layout), Some(&shared.uniform_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bitmap_text_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TextVertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: config.depth_format.map(|format| wgpu::DepthStencilState {
                format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            shared,
            atlas,
            gpu_atlas_texture,
            atlas_bind_group,
            pipeline,
            vertices: Vec::new(),
            indices: Vec::new(),
            config,
        }
    }

    /// Prepare text for rendering.
    pub fn prepare(&mut self, text: &Text) -> TextBuffer {
        astrelis_profiling::profile_function!();
        let mut fs = self.shared.font_system.write().unwrap();
        let mut buffer = TextBuffer::new(&mut fs, 1.0);
        buffer.set_text(&mut fs, text, 1.0);
        buffer.layout(&mut fs);
        buffer
    }

    /// Queue text for drawing at the given position.
    pub fn draw_text(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        astrelis_profiling::profile_function!();
        let mut fs = self.shared.font_system.write().unwrap();
        buffer.layout(&mut fs);

        let scale = buffer.scale;

        for run in buffer.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0., 0.), 1.0);
                let cache_key = physical.cache_key;

                // Rasterize glyph if not in atlas
                if self.atlas.get(&cache_key).is_none() {
                    let image = self.shared.swash_cache.get_image(&mut fs, cache_key);
                    if let Some(image) = image {
                        if image.placement.width > 0 && image.placement.height > 0 {
                            // Convert to single-channel grayscale (handles SubpixelMask/Color formats)
                            let grayscale = astrelis_text::swash_image_to_grayscale(&image);
                            let placement = GlyphPlacement {
                                left: image.placement.left,
                                top: image.placement.top,
                                width: image.placement.width,
                                height: image.placement.height,
                            };
                            self.atlas.insert(
                                cache_key,
                                &grayscale,
                                image.placement.width,
                                image.placement.height,
                                placement,
                            );
                        }
                    }
                }

                // Generate quad vertices
                if let Some((entry, placement)) = self.atlas.get(&cache_key) {
                    let x = position.x + physical.x as f32 / scale + placement.left as f32;
                    let y = position.y + (run.line_y - placement.top as f32) / scale;
                    let w = entry.width as f32;
                    let h = entry.height as f32;

                    let (u0, v0, u1, v1) = entry.uv(self.config.atlas_size, self.config.atlas_size);
                    let color = glyph.color_opt.map_or(
                        [1.0, 1.0, 1.0, 1.0],
                        |c| {
                            [
                                c.r() as f32 / 255.0,
                                c.g() as f32 / 255.0,
                                c.b() as f32 / 255.0,
                                c.a() as f32 / 255.0,
                            ]
                        },
                    );

                    let base_idx = self.vertices.len() as u16;
                    self.vertices.extend_from_slice(&[
                        TextVertex { position: [x, y], tex_coords: [u0, v0], color },
                        TextVertex { position: [x + w, y], tex_coords: [u1, v0], color },
                        TextVertex { position: [x + w, y + h], tex_coords: [u1, v1], color },
                        TextVertex { position: [x, y + h], tex_coords: [u0, v1], color },
                    ]);
                    self.indices.extend_from_slice(&[
                        base_idx,
                        base_idx + 1,
                        base_idx + 2,
                        base_idx,
                        base_idx + 2,
                        base_idx + 3,
                    ]);
                }
            }
        }
    }

    /// Set the viewport dimensions.
    pub fn resize(&mut self, width: f32, height: f32) {
        self.shared.set_viewport(width, height);
    }

    /// Render all queued text.
    pub fn render(
        &mut self,
        device: &WgpuDevice,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        astrelis_profiling::profile_function!();
        if self.vertices.is_empty() {
            return;
        }

        self.shared.set_viewport(width as f32, height as f32);

        let dev = device.wgpu_device();
        let queue = device.wgpu_queue();

        // Upload atlas if dirty
        if self.atlas.dirty {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.gpu_atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.atlas.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.config.atlas_size),
                    rows_per_image: Some(self.config.atlas_size),
                },
                wgpu::Extent3d {
                    width: self.config.atlas_size,
                    height: self.config.atlas_size,
                    depth_or_array_layers: 1,
                },
            );
            self.atlas.dirty = false;
        }

        // Create vertex/index buffers
        use wgpu::util::DeviceExt;
        let vertex_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text_vertex_buffer"),
            contents: bytemuck::cast_slice(&self.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text_index_buffer"),
            contents: bytemuck::cast_slice(&self.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Create projection uniform
        let projection = orthographic_projection(width as f32, height as f32);
        let uniform_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text_uniform_buffer"),
            contents: bytemuck::cast_slice(&projection),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let uniform_bind_group = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_uniform_bind_group"),
            layout: &self.shared.uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Render pass
        let num_indices = self.indices.len() as u32;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bitmap_text_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.atlas_bind_group, &[]);
            pass.set_bind_group(1, &uniform_bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..num_indices, 0, 0..1);
        }

        // Clear staging buffers
        self.vertices.clear();
        self.indices.clear();
    }
}
