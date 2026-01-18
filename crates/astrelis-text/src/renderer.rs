use std::sync::{Arc, RwLock};

use astrelis_core::alloc::HashMap;
use astrelis_core::math::Vec2;
use astrelis_core::profiling::profile_function;
use cosmic_text::{Buffer, CacheKey, Color as CosmicColor, Metrics, Shaping, SwashCache};

use astrelis_render::{GraphicsContext, Renderer, Viewport, wgpu};

use crate::{
    font::FontSystem,
    text::{Text, TextMetrics, color_to_cosmic},
};

/// A cached text buffer with layout information.
pub struct TextBuffer {
    buffer: Buffer,
    needs_layout: bool,
}

impl TextBuffer {
    fn new(font_system: &mut cosmic_text::FontSystem) -> Self {
        let mut buffer = Buffer::new(font_system, Metrics::new(16.0, 20.0));
        buffer.set_wrap(font_system, cosmic_text::Wrap::Word);
        Self {
            buffer,
            needs_layout: true,
        }
    }

    fn set_text(&mut self, font_system: &mut cosmic_text::FontSystem, text: &Text, scale: f32) {
        let metrics = Metrics::new(
            text.get_font_size() * scale,
            text.get_font_size() * scale * text.get_line_height(),
        );
        self.buffer.set_metrics(font_system, metrics);

        let attrs = text
            .get_font_attrs()
            .to_cosmic()
            .color(color_to_cosmic(text.get_color()));

        self.buffer
            .set_text(font_system, text.get_content(), attrs, Shaping::Advanced);

        // Set buffer size for wrapping
        self.buffer
            .set_size(font_system, text.get_max_width().map(|w| w * scale), text.get_max_height().map(|h| h * scale));

        // Set wrapping mode
        self.buffer
            .set_wrap(font_system, text.get_wrap().to_cosmic());

        // Set alignment for all lines
        let align = Some(text.get_align().to_cosmic());
        for line in &mut self.buffer.lines {
            line.set_align(align);
        }

        self.needs_layout = true;
    }

    fn layout(&mut self, font_system: &mut cosmic_text::FontSystem) {
        profile_function!();
        if self.needs_layout {
            self.buffer.shape_until_scroll(font_system, false);
            self.needs_layout = false;
        }
    }

    pub fn bounds(&self) -> (f32, f32) {
        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;

        for run in self.buffer.layout_runs() {
            width = width.max(run.line_w);
            height += run.line_height;
        }

        (width, height)
    }
}

/// Vertex data for text rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
    color: [f32; 4],
}

/// Glyph atlas entry with UV coordinates.
#[derive(Debug, Clone)]
pub struct AtlasEntry {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl AtlasEntry {
    fn uv_coords(&self, atlas_size: u32) -> (f32, f32, f32, f32) {
        let u0 = self.x as f32 / atlas_size as f32;
        let v0 = self.y as f32 / atlas_size as f32;
        let u1 = (self.x + self.width) as f32 / atlas_size as f32;
        let v1 = (self.y + self.height) as f32 / atlas_size as f32;
        (u0, v0, u1, v1)
    }
}

/// Glyph placement information for correct positioning.
#[derive(Debug, Clone, Copy)]
pub struct GlyphPlacement {
    /// Left bearing offset (horizontal offset from origin)
    pub left: f32,
    /// Top bearing offset (vertical offset from baseline)
    pub top: f32,
    /// Glyph width in pixels
    pub width: f32,
    /// Glyph height in pixels
    pub height: f32,
}

/// Simple row-based atlas packer.
struct AtlasPacker {
    size: u32,
    current_x: u32,
    current_y: u32,
    row_height: u32,
}

impl AtlasPacker {
    fn new(size: u32) -> Self {
        Self {
            size,
            current_x: 0,
            current_y: 0,
            row_height: 0,
        }
    }

    fn pack(&mut self, width: u32, height: u32) -> Option<AtlasEntry> {
        // Try to fit in current row
        if self.current_x + width > self.size {
            // Move to next row
            self.current_x = 0;
            self.current_y += self.row_height;
            self.row_height = 0;
        }

        // Check if we have vertical space
        if self.current_y + height > self.size {
            return None; // Atlas full
        }

        let entry = AtlasEntry {
            x: self.current_x,
            y: self.current_y,
            width,
            height,
        };

        self.current_x += width;
        self.row_height = self.row_height.max(height);

        Some(entry)
    }
}

/// Font renderer for rendering text with WGPU.
pub struct FontRenderer {
    renderer: Renderer,
    viewport: Viewport,
    font_system: Arc<RwLock<cosmic_text::FontSystem>>,
    swash_cache: Arc<RwLock<SwashCache>>,

    // GPU resources
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    atlas_texture: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,

    // Atlas management
    atlas_size: u32,
    atlas_data: Vec<u8>,
    atlas_entries: HashMap<CacheKey, AtlasEntry>,
    atlas_packer: AtlasPacker,
    atlas_dirty: bool,

    // Staging data
    vertices: Vec<TextVertex>,
    indices: Vec<u16>,
}

impl FontRenderer {
    /// Measure text dimensions without rendering.
    pub fn measure_text(&self, text: &Text) -> (f32, f32) {
        profile_function!();
        let scale = self.viewport.scale_factor as f32;
        let mut font_system = self.font_system.write().unwrap();
        let mut buffer = TextBuffer::new(&mut font_system);
        // we increase the text size in order to make it sharper on high-DPI displays
        buffer.set_text(&mut font_system, text, scale);
        buffer.layout(&mut font_system);
        let (width, height) = buffer.bounds();
        (width / scale, height / scale)
    }

    /// Get font metrics for the given text style.
    ///
    /// Returns metrics including ascent, descent, line height, and baseline offset
    /// which are useful for precise text positioning and baseline alignment.
    pub fn get_text_metrics(&self, text: &Text) -> TextMetrics {
        profile_function!();
        let scale = self.viewport.scale_factor as f32;
        let font_size = text.get_font_size();
        let line_height_multiplier = text.get_line_height();

        // Create metrics for the given font size and line height
        let metrics = Metrics::new(
            font_size * scale,
            font_size * scale * line_height_multiplier,
        );

        // The line_height from cosmic_text includes both ascent and descent
        let line_height = metrics.line_height / scale;

        // For cosmic-text, the ascent is typically about 80% of font size
        // and descent is about 20% of font size (these are approximations)
        // We can get better metrics by actually querying the font
        let ascent = font_size * 0.8;  // Approximate ascent
        let descent = font_size * 0.2; // Approximate descent

        TextMetrics {
            ascent,
            descent,
            line_height,
            baseline_offset: ascent, // Baseline is at ascent distance from top
        }
    }

    /// Get the baseline offset from the top of the text bounding box.
    ///
    /// This is the distance from the top of the text's bounding box to the baseline
    /// of the first line of text. Useful for aligning text by baseline.
    pub fn get_baseline_offset(&self, text: &Text) -> f32 {
        let metrics = self.get_text_metrics(text);
        metrics.baseline_offset
    }

    /// Create a new font renderer.
    pub fn new(context: &'static GraphicsContext, font_system: FontSystem) -> Self {
        Self::new_with_atlas_size(context, font_system, 2048)
    }

    /// Create a new font renderer with a custom atlas size.
    pub fn new_with_atlas_size(
        context: &'static GraphicsContext,
        font_system: FontSystem,
        atlas_size: u32,
    ) -> Self {
        let renderer = Renderer::new(context);
        let swash_cache = Arc::new(RwLock::new(SwashCache::new()));

        // Create shader
        let shader =
            renderer.create_shader(Some("Text Shader"), include_str!("../shaders/text.wgsl"));

        // Create atlas texture
        let atlas_texture = renderer.create_texture(&wgpu::TextureDescriptor {
            label: Some("Text Atlas"),
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

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = renderer.create_linear_sampler(Some("Text Sampler"));

        // Initialize atlas data
        let atlas_data = vec![0u8; (atlas_size * atlas_size) as usize];

        // Create bind group layout
        let bind_group_layout = renderer.create_bind_group_layout(
            Some("Text Bind Group Layout"),
            &[
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
        );

        let bind_group = renderer.create_bind_group(
            Some("Text Bind Group"),
            &bind_group_layout,
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        );

        // Create uniform bind group layout for projection matrix
        let uniform_bind_group_layout = renderer.create_bind_group_layout(
            Some("Text Uniform Layout"),
            &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        );

        // Create pipeline layout with both bind groups
        let pipeline_layout = renderer.create_pipeline_layout(
            Some("Text Pipeline Layout"),
            &[&bind_group_layout, &uniform_bind_group_layout],
            &[],
        );

        // Create pipeline
        let pipeline = renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4,
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        Self {
            viewport: Viewport::default(),
            renderer,
            font_system: font_system.inner(),
            swash_cache,
            pipeline,
            bind_group_layout,
            uniform_bind_group_layout,
            atlas_texture,
            atlas_view,
            sampler,
            bind_group,
            atlas_size,
            atlas_data,
            atlas_entries: HashMap::new(),
            atlas_packer: AtlasPacker::new(atlas_size),
            atlas_dirty: false,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Ensure a glyph is in the atlas, rasterizing and uploading if needed.
    fn ensure_glyph(&mut self, cache_key: CacheKey) -> Option<&AtlasEntry> {
        // Check if already in atlas
        if self.atlas_entries.contains_key(&cache_key) {
            return self.atlas_entries.get(&cache_key);
        }

        // Rasterize the glyph
        let mut font_system = self.font_system.write().unwrap();
        let mut swash_cache = self.swash_cache.write().unwrap();
        let image = match swash_cache.get_image(&mut font_system, cache_key) {
            Some(img) => img,
            None => return None,
        };

        let width = image.placement.width;
        let height = image.placement.height;

        if width == 0 || height == 0 {
            return None;
        }

        // Try to pack into atlas
        let entry = self.atlas_packer.pack(width, height)?;

        // Copy glyph data into atlas
        for y in 0..height {
            for x in 0..width {
                let src_idx = (y * width + x) as usize;
                let dst_idx = ((entry.y + y) * self.atlas_size + (entry.x + x)) as usize;
                if src_idx < image.data.len() && dst_idx < self.atlas_data.len() {
                    self.atlas_data[dst_idx] = image.data[src_idx];
                }
            }
        }

        self.atlas_dirty = true;
        self.atlas_entries.insert(cache_key, entry.clone());
        self.atlas_entries.get(&cache_key)
    }

    /// Upload atlas data to GPU if dirty.
    fn upload_atlas(&mut self) {
        if !self.atlas_dirty {
            return;
        }

        self.renderer.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.atlas_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.atlas_size),
                rows_per_image: Some(self.atlas_size),
            },
            wgpu::Extent3d {
                width: self.atlas_size,
                height: self.atlas_size,
                depth_or_array_layers: 1,
            },
        );

        self.atlas_dirty = false;
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        if viewport.scale_factor != self.viewport.scale_factor {
            tracing::trace!(
                "FontRenderer scale factor changed: {} -> {}",
                self.viewport.scale_factor,
                viewport.scale_factor
            );
            // Clear atlas and repack on scale factor change
            self.atlas_entries.clear();
            self.atlas_packer = AtlasPacker::new(self.atlas_size);
            self.atlas_dirty = true;
        }
        self.viewport = viewport;
    }

    /// Prepare text for rendering. Returns a TextBuffer handle.
    ///
    /// This buffer can be cached and reused for rendering the same text multiple times,
    /// but must be revalidated if the text content, style, or scale factor changes.
    pub fn prepare(&mut self, text: &Text) -> TextBuffer {
        profile_function!();
        let mut font_system = self.font_system.write().unwrap();
        let mut buffer = TextBuffer::new(&mut font_system);
        buffer.set_text(&mut font_system, text, self.viewport.scale_factor as f32);
        buffer
    }

    /// Draw text at a position.
    ///
    /// The position represents the **top-left corner** of the text's bounding box.
    /// This is consistent with UI layout conventions (CSS, Flutter) where elements
    /// are positioned by their top-left corner.
    pub fn draw_text(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        profile_function!();

        let scale = self.viewport.scale_factor as f32;
        let mut font_system = self.font_system.write().unwrap();
        buffer.layout(&mut font_system);
        drop(font_system);

        // Render glyphs
        for run in buffer.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                // Use run.line_y for proper multi-line positioning
                let physical_glyph = glyph.physical((position.x, position.y + run.line_y), 1.0);
                let cache_key = physical_glyph.cache_key;

                // Ensure glyph is in atlas
                let entry = match self.ensure_glyph(cache_key) {
                    Some(e) => e.clone(),
                    None => continue,
                };

                // Get glyph placement info
                let mut font_system = self.font_system.write().unwrap();
                let mut swash_cache = self.swash_cache.write().unwrap();

                if let Some(image) = swash_cache.get_image(&mut font_system, cache_key) {
                    let x = physical_glyph.x as f32 + image.placement.left as f32;
                    let y = physical_glyph.y as f32 - image.placement.top as f32;
                    let w = image.placement.width as f32;
                    let h = image.placement.height as f32;

                    let x = x / scale;
                    let y = y / scale;
                    let w = w / scale;
                    let h = h / scale;

                    drop(font_system);
                    drop(swash_cache);

                    let (u0, v0, u1, v1) = entry.uv_coords(self.atlas_size);

                    let color = glyph.color_opt.unwrap_or(CosmicColor::rgb(255, 255, 255));
                    let color_f = [
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                        color.a() as f32 / 255.0,
                    ];

                    // TODO: Do we want to do pixel snapping here?
                    let x = (x * scale).round() / scale;
                    let y = (y * scale).round() / scale;

                    // Create quad
                    let idx = self.vertices.len() as u16;

                    self.vertices.push(TextVertex {
                        position: [x, y],
                        tex_coords: [u0, v0],
                        color: color_f,
                    });
                    self.vertices.push(TextVertex {
                        position: [x + w, y],
                        tex_coords: [u1, v0],
                        color: color_f,
                    });
                    self.vertices.push(TextVertex {
                        position: [x + w, y + h],
                        tex_coords: [u1, v1],
                        color: color_f,
                    });
                    self.vertices.push(TextVertex {
                        position: [x, y + h],
                        tex_coords: [u0, v1],
                        color: color_f,
                    });

                    // Create indices for two triangles
                    self.indices
                        .extend_from_slice(&[idx, idx + 1, idx + 2, idx, idx + 2, idx + 3]);
                }
            }
        }
    }

    /// Render all queued text to the given render pass.
    pub fn render(&mut self, render_pass: &mut wgpu::RenderPass) {
        profile_function!();

        debug_assert!(
            self.viewport.is_valid(),
            "Viewport size must be set before rendering text."
        );

        if self.vertices.is_empty() {
            return;
        }

        // Upload atlas if dirty
        self.upload_atlas();

        // Create buffers
        let vertex_buffer = self
            .renderer
            .create_vertex_buffer(Some("Text Vertex Buffer"), &self.vertices);

        let index_buffer = self
            .renderer
            .create_index_buffer(Some("Text Index Buffer"), &self.indices);

        // Create projection uniform
        let size = self.viewport.to_logical();
        let projection = orthographic_projection(
            size.width,
            size.height,
        );
        let uniform_buffer = self
            .renderer
            .create_uniform_buffer(Some("Text Projection"), &projection);

        // Create uniform bind group
        let uniform_bind_group = self.renderer.create_bind_group(
            Some("Text Uniform Bind Group"),
            &self.uniform_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        );

        // Render
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_bind_group(1, &uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);

        // Clear for next frame
        self.vertices.clear();
        self.indices.clear();
    }

    /// Get the font system.
    pub fn font_system(&self) -> Arc<RwLock<cosmic_text::FontSystem>> {
        self.font_system.clone()
    }

    /// Get the atlas size in pixels.
    pub fn atlas_size(&self) -> u32 {
        self.atlas_size
    }

    /// Ensure a glyph is in the atlas using a cache key.
    ///
    /// This is a public wrapper around the internal ensure_glyph method
    /// for use by the retained rendering system.
    pub fn ensure_glyph_in_atlas(&mut self, cache_key: CacheKey) -> Option<&AtlasEntry> {
        self.ensure_glyph(cache_key)
    }

    /// Get glyph placement information (left/top offsets, width, height).
    ///
    /// Returns the placement metrics needed to correctly position a glyph on screen.
    /// This includes the bearing offsets that position the glyph relative to its baseline.
    pub fn get_glyph_placement(&mut self, cache_key: CacheKey) -> Option<GlyphPlacement> {
        let mut font_system = self.font_system.write().unwrap();
        let mut swash_cache = self.swash_cache.write().unwrap();

        let image = swash_cache
            .get_image(&mut font_system, cache_key)
            .as_ref()?;

        let scale = self.viewport.scale_factor as f32;

        Some(GlyphPlacement {
            left: image.placement.left as f32 / scale,
            top: image.placement.top as f32 / scale,
            width: image.placement.width as f32 / scale,
            height: image.placement.height as f32 / scale,
        })
    }

    /// Ensure a glyph is in the atlas and get its placement info.
    ///
    /// This is a combined operation to avoid multiple mutable borrows.
    /// Returns both the atlas entry and glyph placement information.
    pub fn ensure_glyph_with_placement(
        &mut self,
        cache_key: CacheKey,
    ) -> Option<(AtlasEntry, GlyphPlacement)> {
        // First ensure the glyph is in the atlas
        let atlas_entry = self.ensure_glyph(cache_key)?.clone();

        // Then get the placement info
        let mut font_system = self.font_system.write().unwrap();
        let mut swash_cache = self.swash_cache.write().unwrap();

        let image = swash_cache
            .get_image(&mut font_system, cache_key)
            .as_ref()?;

        let scale = self.viewport.scale_factor as f32;

        let placement = GlyphPlacement {
            left: image.placement.left as f32 / scale,
            top: image.placement.top as f32 / scale,
            width: image.placement.width as f32 / scale,
            height: image.placement.height as f32 / scale,
        };

        Some((atlas_entry, placement))
    }

    /// Get swash cache for external glyph operations.
    pub fn swash_cache(&self) -> Arc<RwLock<cosmic_text::SwashCache>> {
        self.swash_cache.clone()
    }

    /// Get the atlas texture view for binding.
    pub fn atlas_texture_view(&self) -> &wgpu::TextureView {
        &self.atlas_view
    }

    /// Get the atlas sampler for binding.
    pub fn atlas_sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Upload atlas data to GPU if dirty (public wrapper).
    pub fn upload_atlas_if_dirty(&mut self) {
        profile_function!();

        self.upload_atlas();
    }

    /// Check if the atlas has pending changes.
    pub fn is_atlas_dirty(&self) -> bool {
        self.atlas_dirty
    }

    /// Get an atlas entry by cache key (if it exists).
    pub fn get_atlas_entry(&self, cache_key: CacheKey) -> Option<&AtlasEntry> {
        self.atlas_entries.get(&cache_key)
    }
}

/// Create an orthographic projection matrix for screen-space rendering.
///
/// This matrix transforms from screen coordinates (top-left origin, Y down)
/// to normalized device coordinates (NDC) where:
/// - X ranges from -1 (left) to +1 (right)
/// - Y ranges from -1 (bottom) to +1 (top)
///
/// The negative Y scale factor (-2.0 / height) flips the Y axis to convert
/// from top-left origin (UI convention) to bottom-left origin (OpenGL/NDC convention).
fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}
