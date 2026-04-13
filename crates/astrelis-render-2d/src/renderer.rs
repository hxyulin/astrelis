//! The main 2D renderer with begin/end drawing API.

use astrelis_core::color::Color;
use astrelis_core::math::Vec2;
use astrelis_gpu::buffer::{BufferDescriptor, BufferUsages};
use astrelis_gpu::resources::{BindGroup, Sampler, TextureView};
use astrelis_gpu::texture::{
    Extent3d, SamplerDescriptor, TextureDescriptor, TextureUsages, TextureViewDescriptor,
};
use astrelis_gpu::types::{TextureDimension, TextureFormat};
use astrelis_gpu::Gpu;

use crate::batch::BatchRenderStats;
use crate::camera::Camera2D;
use crate::instance::{DrawType, Instance2D};
use crate::pipeline::Pipeline2D;
use crate::shapes;
use crate::sprite::{SpriteOptions, SpriteRegion};

/// A draw command before sorting and batching.
struct DrawCommand {
    instance: Instance2D,
    z_index: i32,
    texture_id: u32,
}

/// Handle to a texture registered with the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(u32);

/// A registered texture and its bind group.
struct RegisteredTexture {
    bind_group: BindGroup,
    width: u32,
    height: u32,
}

/// Immediate-mode 2D renderer with automatic batching.
///
/// # Usage
///
/// ```ignore
/// renderer.begin(&camera);
/// renderer.draw_sprite(tex, position, &SpriteOptions::default());
/// renderer.draw_rect_filled(pos, size, Color::RED);
/// renderer.end(&gpu, &mut encoder, surface_view, &camera);
/// ```
pub struct Renderer2D {
    pipeline: Pipeline2D,
    white_texture_bind_group: BindGroup,
    camera_buffer: astrelis_gpu::Buffer,
    camera_bind_group: BindGroup,
    textures: Vec<RegisteredTexture>,
    commands: Vec<DrawCommand>,
    current_z_index: i32,
    instance_buffer: astrelis_gpu::Buffer,
    instance_buffer_capacity: usize,
    stats: BatchRenderStats,
}

impl Renderer2D {
    /// Creates a new 2D renderer.
    pub fn new(gpu: &Gpu, surface_format: TextureFormat) -> Self {
        astrelis_profiling::profile_function!();
        let device = gpu.device();

        let pipeline = Pipeline2D::new(device, surface_format);

        // Create 1x1 white pixel texture using raw wgpu for the queue write.
        let white_tex = device.create_texture(&TextureDescriptor {
            label: Some("render2d_white"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        });

        // Use raw wgpu queue to write texture data (avoiding the
        // BufferCopyView API which requires a buffer reference).
        gpu.raw_queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: white_tex.raw(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let white_view = device.create_texture_view(&white_tex, &TextureViewDescriptor::default());
        let white_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("render2d_nearest"),
            ..Default::default()
        });
        let white_texture_bind_group =
            pipeline.create_texture_bind_group(device, &white_view, &white_sampler);

        // Camera uniform buffer (64 bytes for a mat4x4).
        let camera_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("render2d_camera"),
            size: 64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = pipeline.create_camera_bind_group(device, &camera_buffer);

        // Initial instance buffer.
        let initial_capacity = 1024;
        let instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("render2d_instances"),
            size: (initial_capacity * std::mem::size_of::<Instance2D>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            white_texture_bind_group,
            camera_buffer,
            camera_bind_group,
            textures: Vec::new(),
            commands: Vec::new(),
            current_z_index: 0,
            instance_buffer,
            instance_buffer_capacity: initial_capacity,
            stats: BatchRenderStats::default(),
        }
    }

    /// Registers a texture for use with `draw_sprite`.
    ///
    /// Returns a handle that can be passed to drawing methods.
    pub fn register_texture(
        &mut self,
        gpu: &Gpu,
        view: &TextureView,
        sampler: &Sampler,
        width: u32,
        height: u32,
    ) -> TextureHandle {
        let bind_group =
            self.pipeline
                .create_texture_bind_group(gpu.device(), view, sampler);
        let idx = self.textures.len() as u32 + 1; // +1 because 0 = white pixel
        self.textures.push(RegisteredTexture {
            bind_group,
            width,
            height,
        });
        TextureHandle(idx)
    }

    /// Begins a new frame. Clears all draw commands and resets the z-index.
    pub fn begin(&mut self, _camera: &Camera2D) {
        self.commands.clear();
        self.current_z_index = 0;
    }

    /// Sets the z-index for subsequent draw calls.
    ///
    /// Higher values are drawn on top. Default is 0.
    pub fn set_z_index(&mut self, z: i32) {
        self.current_z_index = z;
    }

    /// Draws a sprite at the given position.
    pub fn draw_sprite(&mut self, texture: TextureHandle, position: Vec2, opts: &SpriteOptions) {
        let tex_idx = texture.0;
        let reg = &self.textures[(tex_idx - 1) as usize];
        let width = reg.width as f32 * opts.scale.x;
        let height = reg.height as f32 * opts.scale.y;

        let (mut u_min, mut u_max) = (0.0_f32, 1.0_f32);
        let (mut v_min, mut v_max) = (0.0_f32, 1.0_f32);
        if opts.flip_x {
            std::mem::swap(&mut u_min, &mut u_max);
        }
        if opts.flip_y {
            std::mem::swap(&mut v_min, &mut v_max);
        }

        let origin_offset = Vec2::new(width * opts.origin.x, height * opts.origin.y);
        let draw_pos = position - origin_offset;

        self.push_command(
            Instance2D {
                position: draw_pos.into(),
                size: [width, height],
                uv_min: [u_min, v_min],
                uv_max: [u_max, v_max],
                color: [opts.tint.r, opts.tint.g, opts.tint.b, opts.tint.a],
                rotation: opts.rotation,
                z_depth: 0.0,
                texture_index: tex_idx,
                draw_type: DrawType::Sprite as u32,
            },
            tex_idx,
        );
    }

    /// Draws a sprite sub-region (atlas rectangle) at the given position.
    pub fn draw_sprite_region(
        &mut self,
        texture: TextureHandle,
        region: SpriteRegion,
        position: Vec2,
        opts: &SpriteOptions,
    ) {
        let tex_idx = texture.0;
        let reg = &self.textures[(tex_idx - 1) as usize];
        let tex_w = reg.width as f32;
        let tex_h = reg.height as f32;

        let width = region.width * opts.scale.x;
        let height = region.height * opts.scale.y;

        let mut u_min = region.x / tex_w;
        let mut u_max = (region.x + region.width) / tex_w;
        let mut v_min = region.y / tex_h;
        let mut v_max = (region.y + region.height) / tex_h;

        if opts.flip_x {
            std::mem::swap(&mut u_min, &mut u_max);
        }
        if opts.flip_y {
            std::mem::swap(&mut v_min, &mut v_max);
        }

        let origin_offset = Vec2::new(width * opts.origin.x, height * opts.origin.y);
        let draw_pos = position - origin_offset;

        self.push_command(
            Instance2D {
                position: draw_pos.into(),
                size: [width, height],
                uv_min: [u_min, v_min],
                uv_max: [u_max, v_max],
                color: [opts.tint.r, opts.tint.g, opts.tint.b, opts.tint.a],
                rotation: opts.rotation,
                z_depth: 0.0,
                texture_index: tex_idx,
                draw_type: DrawType::Sprite as u32,
            },
            tex_idx,
        );
    }

    /// Draws a filled rectangle.
    pub fn draw_rect_filled(&mut self, position: Vec2, size: Vec2, color: Color) {
        let inst = shapes::filled_rect(position, size, color, 0.0);
        self.push_command(inst, 0);
    }

    /// Draws an outlined rectangle.
    pub fn draw_rect(&mut self, position: Vec2, size: Vec2, color: Color, thickness: f32) {
        let edges = shapes::outlined_rect(position, size, color, thickness, 0.0);
        for inst in edges {
            self.push_command(inst, 0);
        }
    }

    /// Draws a filled circle.
    pub fn draw_circle_filled(&mut self, center: Vec2, radius: f32, color: Color) {
        let inst = shapes::filled_circle(center, radius, color, 0.0);
        self.push_command(inst, 0);
    }

    /// Draws a line segment.
    pub fn draw_line(&mut self, start: Vec2, end: Vec2, thickness: f32, color: Color) {
        let inst = shapes::line(start, end, thickness, color, 0.0);
        self.push_command(inst, 0);
    }

    /// Sorts, batches, and submits all accumulated draw commands.
    ///
    /// Encodes a render pass into the given encoder targeting the
    /// provided surface view. Uses `LoadOp::Load` so prior rendering
    /// (e.g. a clear pass) is preserved.
    pub fn end(
        &mut self,
        gpu: &Gpu,
        encoder: &mut astrelis_gpu::CommandEncoder<'_>,
        target_view: &TextureView,
        camera: &Camera2D,
    ) {
        astrelis_profiling::profile_function!();

        if self.commands.is_empty() {
            self.stats = BatchRenderStats::default();
            return;
        }

        // Update camera uniform.
        let vp = camera.view_projection();
        gpu.device()
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&vp));

        // Sort: primary by z_index, secondary by texture_id for batching.
        self.commands.sort_by(|a, b| {
            a.z_index
                .cmp(&b.z_index)
                .then(a.texture_id.cmp(&b.texture_id))
        });

        // Assign z_depth based on sorted order.
        let count = self.commands.len();
        for (i, cmd) in self.commands.iter_mut().enumerate() {
            cmd.instance.z_depth = i as f32 / count.max(1) as f32;
        }

        // Collect instances.
        let instances: Vec<Instance2D> = self.commands.iter().map(|c| c.instance).collect();
        let instance_data = bytemuck::cast_slice::<Instance2D, u8>(&instances);

        // Grow instance buffer if needed.
        if instances.len() > self.instance_buffer_capacity {
            let new_capacity = instances.len().next_power_of_two();
            self.instance_buffer = gpu.device().create_buffer(&BufferDescriptor {
                label: Some("render2d_instances"),
                size: (new_capacity * std::mem::size_of::<Instance2D>()) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_buffer_capacity = new_capacity;
        }

        // Upload instance data.
        gpu.device()
            .write_buffer(&self.instance_buffer, 0, instance_data);

        // Record render pass.
        let mut stats = BatchRenderStats {
            instance_count: instances.len() as u32,
            draw_calls: 0,
            texture_switches: 0,
        };

        {
            let mut pass =
                encoder.begin_render_pass(&astrelis_gpu::command::RenderPassDescriptor {
                    label: Some("render2d"),
                    color_attachments: &[astrelis_gpu::command::ColorAttachment {
                        view: target_view,
                        resolve_target: None,
                        load_op: astrelis_gpu::types::LoadOp::Load,
                        store_op: astrelis_gpu::types::StoreOp::Store,
                    }],
                    depth_stencil_attachment: None,
                });

            pass.set_pipeline(&self.pipeline.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, &self.instance_buffer, 0, None);

            // Batch by texture: group consecutive same-texture runs
            // into single instanced draw calls.
            let mut batch_start: u32 = 0;
            let mut current_tex = self.commands[0].texture_id;
            self.bind_texture(&mut pass, current_tex);
            stats.texture_switches += 1;

            for i in 1..=self.commands.len() {
                let tex = if i < self.commands.len() {
                    self.commands[i].texture_id
                } else {
                    u32::MAX // sentinel to flush last batch
                };

                if tex != current_tex {
                    let batch_end = i as u32;
                    // Draw 6 vertices (one quad) per instance.
                    pass.draw(0..6, batch_start..batch_end);
                    stats.draw_calls += 1;

                    if tex != u32::MAX {
                        self.bind_texture(&mut pass, tex);
                        stats.texture_switches += 1;
                        current_tex = tex;
                    }
                    batch_start = i as u32;
                }
            }
        }

        self.stats = stats;
    }

    /// Returns statistics from the last `end()` call.
    pub fn stats(&self) -> BatchRenderStats {
        self.stats
    }

    fn push_command(&mut self, instance: Instance2D, texture_id: u32) {
        self.commands.push(DrawCommand {
            instance,
            z_index: self.current_z_index,
            texture_id,
        });
    }

    fn bind_texture(&self, pass: &mut astrelis_gpu::RenderPass<'_>, texture_id: u32) {
        if texture_id == 0 {
            pass.set_bind_group(1, &self.white_texture_bind_group, &[]);
        } else {
            let idx = (texture_id - 1) as usize;
            if let Some(reg) = self.textures.get(idx) {
                pass.set_bind_group(1, &reg.bind_group, &[]);
            }
        }
    }
}
