//! SDF text renderer with support for effects.
//!
//! Uses a Signed Distance Field glyph atlas for resolution-independent
//! text rendering and visual effects (shadows, outlines, glows).

use astrelis_core::math::Vec2;
use astrelis_gpu::Gpu;
use astrelis_text::{FontSystem, SdfConfig, Text, TextEffects};

use crate::atlas::GlyphPlacement;
use crate::atlas::sdf::{SdfAtlas, SdfCacheKey};
use crate::config::TextRendererConfig;

use super::vertex::TextVertex;
use super::{SdfParams, SharedContext, TextBuffer, orthographic_projection};

/// A batch of vertices sharing the same SDF effect parameters.
struct DrawBatch {
    params: SdfParams,
    vertices: Vec<TextVertex>,
    indices: Vec<u16>,
}

/// SDF text renderer (~8 MB with default atlas).
///
/// Best for large text (>=24px), titles, and text with effects
/// (shadows, outlines, glows).
pub struct SdfTextRenderer {
    pub(crate) shared: SharedContext,
    pub(crate) atlas: SdfAtlas,
    pub(crate) gpu_atlas_texture: wgpu::Texture,
    pub(crate) atlas_bind_group: wgpu::BindGroup,
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) params_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) batches: Vec<DrawBatch>,
    pub(crate) config: TextRendererConfig,
}

impl SdfTextRenderer {
    /// Create a new SDF text renderer.
    pub fn new(
        gpu: &Gpu,
        font_system: FontSystem,
        config: TextRendererConfig,
    ) -> Self {
        astrelis_profiling::profile_function!();
        let shared = SharedContext::new(gpu, font_system);
        let atlas_size = config.atlas_size;
        let atlas = SdfAtlas::new(atlas_size);

        let dev = gpu.raw_device();

        // Create atlas texture
        let gpu_atlas_texture = dev.create_texture(&wgpu::TextureDescriptor {
            label: Some("sdf_text_atlas"),
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
            label: Some("sdf_text_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Atlas bind group layout
        let atlas_bind_group_layout =
            dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sdf_text_atlas_layout"),
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
            label: Some("sdf_text_atlas_bind_group"),
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

        // SDF params bind group layout (per-batch buffers created at render time)
        let params_bind_group_layout =
            dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sdf_params_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Pipeline
        let shader = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sdf_text_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/text_sdf.wgsl").into()),
        });

        let pipeline_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sdf_text_pipeline_layout"),
            bind_group_layouts: &[
                Some(&atlas_bind_group_layout),
                Some(&shared.uniform_bind_group_layout),
                Some(&params_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let pipeline = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sdf_text_pipeline"),
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
            params_bind_group_layout,
            batches: Vec::new(),
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

    /// Queue text for drawing at the given position (no effects).
    pub fn draw_text(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        astrelis_profiling::profile_function!();
        self.emit_glyphs(buffer, position, [1.0, 1.0, 1.0, 1.0], Vec2::ZERO, SdfParams::default());
    }

    /// Queue text with effects for drawing.
    ///
    /// **Note:** Effects rendering is not yet implemented. This method currently
    /// renders the text without any effects applied. The effects parameter is
    /// accepted for API stability.
    pub fn draw_text_with_effects(
        &mut self,
        buffer: &mut TextBuffer,
        position: Vec2,
        _effects: &TextEffects,
    ) {
        astrelis_profiling::profile_function!();
        // TODO: implement SDF-based text effects (shadow, outline, glow)
        self.emit_glyphs(buffer, position, [1.0, 1.0, 1.0, 1.0], Vec2::ZERO, SdfParams::default());
    }

    /// Emit glyph quads into a new batch with the given color override, position offset, and params.
    fn emit_glyphs(
        &mut self,
        buffer: &mut TextBuffer,
        position: Vec2,
        color_override: [f32; 4],
        offset: Vec2,
        params: SdfParams,
    ) {
        self.batches.push(DrawBatch {
            params,
            vertices: Vec::new(),
            indices: Vec::new(),
        });

        let mut fs = self.shared.font_system.write().unwrap();
        buffer.layout(&mut fs);

        let scale = buffer.scale;
        let base_size = self.config.sdf.base_size;
        let spread = self.config.sdf.default_spread;

        for run in buffer.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0., 0.), 1.0);
                let cache_key = physical.cache_key;
                let sdf_key = SdfCacheKey::from_cache_key(&cache_key);

                // Rasterize SDF glyph if not in atlas
                if self.atlas.get(&sdf_key).is_none() {
                    let base_key = cosmic_text::CacheKey {
                        font_size_bits: base_size.to_bits(),
                        ..cache_key
                    };

                    let image = self.shared.swash_cache.get_image(&mut fs, base_key);
                    if let Some(image) = image {
                        if image.placement.width > 0 && image.placement.height > 0 {
                            let grayscale = astrelis_text::swash_image_to_grayscale(&image);
                            let w = image.placement.width as usize;
                            let h = image.placement.height as usize;

                            let sdf_data = if self.config.sdf.smooth {
                                astrelis_text::generate_sdf_smooth(&grayscale, w, h, spread)
                            } else {
                                astrelis_text::generate_sdf(&grayscale, w, h, spread)
                            };

                            let placement = GlyphPlacement {
                                left: image.placement.left,
                                top: image.placement.top,
                                width: image.placement.width,
                                height: image.placement.height,
                            };

                            self.atlas.insert(
                                sdf_key,
                                &sdf_data,
                                image.placement.width,
                                image.placement.height,
                                placement,
                                base_size,
                                spread,
                            );
                        }
                    }
                }

                // Generate quad vertices with size scaling
                if let Some(sdf_entry) = self.atlas.get(&sdf_key) {
                    let target_size = glyph.font_size;
                    let size_scale = target_size / sdf_entry.base_size;

                    let placement = &sdf_entry.placement;
                    let entry = &sdf_entry.atlas_entry;

                    let x = position.x + offset.x + physical.x as f32 / scale
                        + placement.left as f32 * size_scale;
                    let y = position.y + offset.y
                        + (run.line_y - placement.top as f32 * size_scale) / scale;
                    let w = entry.width as f32 * size_scale;
                    let h = entry.height as f32 * size_scale;

                    let (u0, v0, u1, v1) =
                        entry.uv(self.config.atlas_size, self.config.atlas_size);

                    // Use color override if non-white, otherwise use glyph color
                    let color = if color_override != [1.0, 1.0, 1.0, 1.0] {
                        color_override
                    } else {
                        glyph.color_opt.map_or([1.0, 1.0, 1.0, 1.0], |c| {
                            [
                                c.r() as f32 / 255.0,
                                c.g() as f32 / 255.0,
                                c.b() as f32 / 255.0,
                                c.a() as f32 / 255.0,
                            ]
                        })
                    };

                    let batch = self.batches.last_mut().unwrap();
                    let base_idx = batch.vertices.len() as u16;
                    batch.vertices.extend_from_slice(&[
                        TextVertex { position: [x, y], tex_coords: [u0, v0], color },
                        TextVertex { position: [x + w, y], tex_coords: [u1, v0], color },
                        TextVertex { position: [x + w, y + h], tex_coords: [u1, v1], color },
                        TextVertex { position: [x, y + h], tex_coords: [u0, v1], color },
                    ]);
                    batch.indices.extend_from_slice(&[
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

    /// Get the SDF config.
    pub fn sdf_config(&self) -> &SdfConfig {
        &self.config.sdf
    }

    /// Set the SDF config.
    pub fn set_sdf_config(&mut self, sdf_config: SdfConfig) {
        self.config.sdf = sdf_config;
    }

    /// Render all queued text.
    ///
    /// Issues one draw call per effect batch — text drawn with different
    /// effects gets its own SdfParams uniform upload.
    pub fn render(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        astrelis_profiling::profile_function!();
        if self.batches.is_empty() {
            return;
        }

        self.shared.set_viewport(width as f32, height as f32);

        let dev = gpu.raw_device();
        let queue = gpu.raw_queue();

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

        // Shared projection uniform
        use wgpu::util::DeviceExt;
        let projection = orthographic_projection(width as f32, height as f32);
        let uniform_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sdf_text_uniform_buffer"),
            contents: bytemuck::cast_slice(&projection),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let uniform_bind_group = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sdf_text_uniform_bind_group"),
            layout: &self.shared.uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Drain batches and render each with its own params
        let batches: Vec<DrawBatch> = self.batches.drain(..).collect();

        for batch in &batches {
            if batch.vertices.is_empty() {
                continue;
            }

            // Create per-batch SDF params buffer and bind group
            let params_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sdf_params_buffer"),
                contents: bytemuck::bytes_of(&batch.params),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let params_bind_group = dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sdf_params_bind_group"),
                layout: &self.params_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                }],
            });

            let vertex_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sdf_text_vertex_buffer"),
                contents: bytemuck::cast_slice(&batch.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let index_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sdf_text_index_buffer"),
                contents: bytemuck::cast_slice(&batch.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let num_indices = batch.indices.len() as u32;
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("sdf_text_render_pass"),
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
                pass.set_bind_group(2, &params_bind_group, &[]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..num_indices, 0, 0..1);
            }
        }
    }
}
