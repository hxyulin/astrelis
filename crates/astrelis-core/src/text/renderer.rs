use std::collections::HashMap;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};

use crate::{graphics::Color, math::Vec2, text::Font};

/// Glyph atlas for caching rendered glyphs
struct GlyphAtlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl GlyphAtlas {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            width,
            height,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    fn can_fit(&self, width: u32, height: u32) -> bool {
        if self.cursor_x + width <= self.width {
            self.cursor_y + height.max(self.row_height) <= self.height
        } else {
            self.cursor_y + self.row_height + height <= self.height
        }
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<GlyphRect> {
        if !self.can_fit(width, height) {
            return None;
        }

        if self.cursor_x + width > self.width {
            self.cursor_y += self.row_height;
            self.cursor_x = 0;
            self.row_height = 0;
        }

        let rect = GlyphRect {
            x: self.cursor_x,
            y: self.cursor_y,
            width,
            height,
        };

        self.cursor_x += width;
        self.row_height = self.row_height.max(height);

        Some(rect)
    }

    fn upload_glyph(&self, queue: &wgpu::Queue, rect: &GlyphRect, data: &[u8]) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: rect.x,
                    y: rect.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(rect.width),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: rect.width,
                height: rect.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

#[derive(Clone, Copy)]
struct GlyphRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

// Use cosmic-text's cache key directly
type GlyphKey = cosmic_text::CacheKey;

#[derive(Clone, Copy)]
struct CachedGlyph {
    rect: GlyphRect,
    bearing_x: f32,
    bearing_y: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
    color: [f32; 4],
}

impl TextVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Text renderer using cosmic-text for shaping and GPU rasterization
pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,

    atlas: GlyphAtlas,
    glyph_cache: HashMap<GlyphKey, CachedGlyph>,

    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,

    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertices: Vec<TextVertex>,
    indices: Vec<u16>,

    // Pending glyph uploads (data to upload to atlas)
    pending_uploads: Vec<(GlyphRect, Vec<u8>)>,

    // Screen dimensions for coordinate conversion
    screen_width: f32,
    screen_height: f32,
}

impl TextRenderer {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();

        let atlas = GlyphAtlas::new(device, 2048, 2048);

        // Create sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Text Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text Bind Group Layout"),
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

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/text.wgsl").into()),
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TextVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
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

        // Create vertex and index buffers
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Vertex Buffer"),
            size: 65536,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Index Buffer"),
            size: 65536,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            font_system,
            swash_cache,
            atlas,
            glyph_cache: HashMap::new(),
            pipeline,
            bind_group_layout,
            bind_group,
            sampler,
            vertex_buffer,
            index_buffer,
            vertices: Vec::new(),
            indices: Vec::new(),
            pending_uploads: Vec::new(),
            screen_width: 800.0,
            screen_height: 600.0,
        }
    }

    /// Set screen dimensions for coordinate conversion
    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Register a font with the renderer
    pub fn register_font(&mut self, font: &mut Font) {
        font.register(&mut self.font_system);
    }

    /// Draw text at the specified position
    pub fn draw_text(
        &mut self,
        text: &str,
        position: Vec2,
        font_size: f32,
        color: Color,
        font_family: &str,
    ) {
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        buffer.set_size(&mut self.font_system, None, None);
        buffer.set_text(
            &mut self.font_system,
            text,
            Attrs::new().family(cosmic_text::Family::Name(font_family)),
            Shaping::Advanced,
        );

        self.draw_buffer(&buffer, position, color);
    }

    /// Draw a cosmic-text buffer at the specified position
    pub fn draw_buffer(&mut self, buffer: &Buffer, position: Vec2, color: Color) {
        let cursor_x = position.x;
        let mut cursor_y = position.y;

        for line in buffer.lines.iter() {
            if let Some(layout) = line.layout_opt().as_ref() {
                for layout_line in layout.iter() {
                    for glyph in layout_line.glyphs.iter() {
                        let physical = glyph.physical((cursor_x, cursor_y), 1.0);

                        // Add glyph to vertex buffer
                        self.add_glyph(
                            physical.cache_key,
                            Vec2::new(physical.x as f32, physical.y as f32),
                            color,
                        );
                    }
                }
            }
            cursor_y += buffer.metrics().line_height;
        }
    }

    fn add_glyph(&mut self, cache_key: cosmic_text::CacheKey, position: Vec2, color: Color) {
        // Check if glyph is cached
        if !self.glyph_cache.contains_key(&cache_key) {
            self.cache_glyph(cache_key);
        }

        if let Some(cached) = self.glyph_cache.get(&cache_key).copied() {
            self.add_glyph_quad(&cached, position, color);
        }
    }

    fn cache_glyph(&mut self, cache_key: cosmic_text::CacheKey) {
        use cosmic_text::SwashContent;

        if let Some(image) = self.swash_cache.get_image(&mut self.font_system, cache_key) {
            let width = image.placement.width;
            let height = image.placement.height;

            if width == 0 || height == 0 {
                return;
            }

            // Allocate space in atlas
            if let Some(rect) = self.atlas.allocate(width, height) {
                // Convert image data based on content type
                let data: Vec<u8> = match image.content {
                    SwashContent::Mask => image.data.to_vec(),
                    SwashContent::Color => {
                        // Convert RGBA to grayscale
                        image
                            .data
                            .chunks(4)
                            .map(|rgba| rgba[3]) // Use alpha channel
                            .collect()
                    }
                    SwashContent::SubpixelMask => {
                        // Convert subpixel to grayscale
                        image
                            .data
                            .chunks(3)
                            .map(|rgb| ((rgb[0] as u32 + rgb[1] as u32 + rgb[2] as u32) / 3) as u8)
                            .collect()
                    }
                };

                // Queue glyph data for upload
                self.pending_uploads.push((rect, data));

                let cached = CachedGlyph {
                    rect,
                    bearing_x: image.placement.left as f32,
                    bearing_y: image.placement.top as f32,
                };

                self.glyph_cache.insert(cache_key, cached);
            }
        }
    }

    fn add_glyph_quad(&mut self, glyph: &CachedGlyph, position: Vec2, color: Color) {
        let x0 = position.x + glyph.bearing_x;
        let y0 = position.y - glyph.bearing_y;
        let x1 = x0 + glyph.rect.width as f32;
        let y1 = y0 + glyph.rect.height as f32;

        // Convert screen pixel coordinates to NDC (-1 to 1)
        let to_ndc_x = |x: f32| (x / self.screen_width) * 2.0 - 1.0;
        let to_ndc_y = |y: f32| 1.0 - (y / self.screen_height) * 2.0;

        let ndc_x0 = to_ndc_x(x0);
        let ndc_y0 = to_ndc_y(y0);
        let ndc_x1 = to_ndc_x(x1);
        let ndc_y1 = to_ndc_y(y1);

        let u0 = glyph.rect.x as f32 / self.atlas.width as f32;
        let v0 = glyph.rect.y as f32 / self.atlas.height as f32;
        let u1 = (glyph.rect.x + glyph.rect.width) as f32 / self.atlas.width as f32;
        let v1 = (glyph.rect.y + glyph.rect.height) as f32 / self.atlas.height as f32;

        let color_array = color.as_array();

        let base_index = self.vertices.len() as u16;

        self.vertices.push(TextVertex {
            position: [ndc_x0, ndc_y0, 0.0],
            tex_coords: [u0, v0],
            color: color_array,
        });
        self.vertices.push(TextVertex {
            position: [ndc_x1, ndc_y0, 0.0],
            tex_coords: [u1, v0],
            color: color_array,
        });
        self.vertices.push(TextVertex {
            position: [ndc_x1, ndc_y1, 0.0],
            tex_coords: [u1, v1],
            color: color_array,
        });
        self.vertices.push(TextVertex {
            position: [ndc_x0, ndc_y1, 0.0],
            tex_coords: [u0, v1],
            color: color_array,
        });

        self.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);
    }

    /// Render all queued text to the render target
    pub fn render(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, view: &wgpu::TextureView) {
        // Upload pending glyph data to atlas
        for (rect, data) in self.pending_uploads.drain(..) {
            self.atlas.upload_glyph(queue, &rect, &data);
        }

        if self.vertices.is_empty() {
            return;
        }

        // Upload vertex and index data
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&self.indices));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Text Renderer Command Encoder"),
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Text Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);

        drop(render_pass);

        queue.submit(std::iter::once(encoder.finish()));

        // Clear for next frame
        self.vertices.clear();
        self.indices.clear();
    }

    /// Get mutable access to the font system for advanced usage
    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }
}
