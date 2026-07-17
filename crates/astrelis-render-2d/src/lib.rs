//! Batched sprite and finite tilemap rendering for Astrelis.

#![warn(missing_docs)]

mod camera;
mod scene;

pub use camera::Camera2D;
pub use scene::{DrawList2D, SpriteDraw, TileAtlas, Tilemap, TilemapDraw};

use std::{
    collections::HashMap,
    error::Error,
    fmt,
    mem::size_of,
    sync::atomic::{AtomicU64, Ordering},
};

use astrelis_core::{
    color::Color,
    geometry::{Physical, Point, Rect, Size},
    math::{Mat4, Vec2},
};
use astrelis_gpu as gpu;
use astrelis_render::{Antialiasing, CompositedRenderTarget, RenderStats, RenderTarget};
use bytemuck::{Pod, Zeroable};

const SHADER: &str = include_str!("shader.wgsl");
static NEXT_RENDERER: AtomicU64 = AtomicU64::new(1);

/// Device-bound 2D renderer configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RendererOptions {
    /// Edge antialiasing mode.
    pub antialiasing: Antialiasing,
}

impl Default for RendererOptions {
    fn default() -> Self {
        Self {
            antialiasing: Antialiasing::Msaa4,
        }
    }
}

/// Generational texture reference owned by one [`Renderer2D`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureHandle {
    owner: u64,
    slot: u32,
    generation: u32,
}

impl TextureHandle {
    #[cfg(test)]
    pub(crate) const fn testing(slot: u32) -> Self {
        Self {
            owner: 0,
            slot,
            generation: 0,
        }
    }
}

/// Texture registration filtering options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureOptions {
    /// Magnification/minification filter.
    pub filtering: gpu::FilterMode,
    /// Horizontal address policy.
    pub address_u: gpu::AddressMode,
    /// Vertical address policy.
    pub address_v: gpu::AddressMode,
}

struct TextureSlot {
    generation: u32,
    resource: Option<TextureResource>,
}

struct TextureResource {
    _texture: Option<gpu::Texture>,
    _view: gpu::TextureView,
    bind_group: gpu::BindGroup,
    size: Size<Physical, u32>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    basis_x: [f32; 2],
    basis_y: [f32; 2],
    translation: [f32; 2],
    size: [f32; 2],
    pivot: [f32; 2],
    uv_rect: [f32; 4],
    color: [f32; 4],
}

struct Prepared {
    texture: TextureHandle,
    layer: i32,
    order: usize,
    instance: Instance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PipelineKey(gpu::TextureFormat, u32);

struct Attachments {
    key: (u32, u32, gpu::TextureFormat, u32),
    _texture: Option<gpu::Texture>,
    view: Option<gpu::TextureView>,
}

struct FrameTarget {
    view: gpu::TextureView,
    allocation_size: Size<Physical, u32>,
    render_size: Size<Physical, u32>,
    origin: Point<Physical, u32>,
    scissor: Rect<Physical, u32>,
    scale_factor: f32,
    clear_color: Color,
    samples: u32,
    load: bool,
}

/// Sprite renderer tied to one device and queue.
pub struct Renderer2D {
    owner: u64,
    device: gpu::Device,
    queue: gpu::Queue,
    options: RendererOptions,
    camera_layout: gpu::BindGroupLayout,
    texture_layout: gpu::BindGroupLayout,
    camera_buffer: gpu::Buffer,
    camera_bind_group: gpu::BindGroup,
    pipelines: HashMap<PipelineKey, gpu::RenderPipeline>,
    attachments: Vec<Attachments>,
    textures: Vec<TextureSlot>,
}

impl Renderer2D {
    /// Creates a renderer for one matching device/queue pair.
    pub fn new(
        device: gpu::Device,
        queue: gpu::Queue,
        options: RendererOptions,
    ) -> Result<Self, RenderError> {
        if device.id() != queue.device_id() {
            return Err(RenderError::new("device and queue do not match"));
        }
        let camera_layout = device.create_bind_group_layout(gpu::BindGroupLayoutDescriptor {
            label: Some("render-2d camera layout".into()),
            entries: vec![gpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: gpu::ShaderStages::VERTEX,
                ty: gpu::BindingType::Buffer {
                    ty: gpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            }],
        });
        let texture_layout = device.create_bind_group_layout(gpu::BindGroupLayoutDescriptor {
            label: Some("render-2d texture layout".into()),
            entries: vec![
                gpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Texture {
                        sample_type: gpu::TextureSampleType::Float,
                        view_dimension: gpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                },
                gpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Sampler(gpu::SamplerBindingType::Filtering),
                },
            ],
        });
        let identity: astrelis_core::math::packed::Mat4 = Mat4::IDENTITY.into();
        let camera_buffer = device.create_buffer_init(
            &queue,
            Some("render-2d camera".into()),
            bytemuck::bytes_of(&identity),
            gpu::BufferUsages::UNIFORM,
        )?;
        let camera_bind_group = device.create_bind_group(gpu::BindGroupDescriptor {
            label: Some("render-2d camera bind group".into()),
            layout: camera_layout.clone(),
            entries: vec![gpu::BindGroupEntry {
                binding: 0,
                resource: gpu::BindingResource::Buffer(gpu::BufferBinding {
                    buffer: camera_buffer.clone(),
                    offset: 0,
                    size: None,
                }),
            }],
        })?;
        Ok(Self {
            owner: NEXT_RENDERER.fetch_add(1, Ordering::Relaxed),
            device,
            queue,
            options,
            camera_layout,
            texture_layout,
            camera_buffer,
            camera_bind_group,
            pipelines: HashMap::new(),
            attachments: Vec::new(),
            textures: Vec::new(),
        })
    }

    /// Uploads one immutable straight-alpha RGBA8 sRGB texture.
    pub fn create_texture_rgba8(
        &mut self,
        size: Size<Physical, u32>,
        bytes: &[u8],
        options: TextureOptions,
    ) -> Result<TextureHandle, RenderError> {
        let expected = u64::from(size.width)
            .checked_mul(u64::from(size.height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| RenderError::new("texture size overflow"))?;
        if size.width == 0 || size.height == 0 || bytes.len() as u64 != expected {
            return Err(RenderError::new(
                "invalid RGBA8 texture dimensions or byte length",
            ));
        }
        let texture = self.device.create_texture(gpu::TextureDescriptor {
            label: Some("render-2d uploaded texture".into()),
            size: gpu::Extent3d::d2(size.width, size.height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: gpu::TextureDimension::D2,
            format: gpu::TextureFormat::Rgba8UnormSrgb,
            usage: gpu::TextureUsages::TEXTURE_BINDING | gpu::TextureUsages::COPY_DST,
        });
        self.queue.write_texture(
            &gpu::TextureCopy {
                texture: texture.clone(),
                mip_level: 0,
                origin: Default::default(),
            },
            bytes,
            gpu::TextureDataLayout {
                offset: 0,
                bytes_per_row: Some(size.width * 4),
                rows_per_image: Some(size.height),
            },
            gpu::Extent3d::d2(size.width, size.height),
        )?;
        let view = texture.create_view(Default::default());
        self.insert_texture(Some(texture), view, size, options)
    }

    /// Registers an existing single-sampled filterable 2D texture view.
    pub fn register_texture(
        &mut self,
        view: gpu::TextureView,
        size: Size<Physical, u32>,
        options: TextureOptions,
    ) -> Result<TextureHandle, RenderError> {
        self.insert_texture(None, view, size, options)
    }

    /// Removes a texture. Existing handles become stale.
    pub fn remove_texture(&mut self, handle: TextureHandle) -> Result<(), RenderError> {
        let slot = self.texture_slot_mut(handle)?;
        slot.resource = None;
        slot.generation = slot.generation.wrapping_add(1);
        Ok(())
    }

    /// Clears and renders one camera-specific draw list.
    pub fn render(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        target: &RenderTarget,
        camera: &Camera2D,
        draw_list: &DrawList2D,
    ) -> Result<RenderStats, RenderError> {
        target.validate(self.device.id())?;
        self.render_impl(
            encoder,
            FrameTarget {
                view: target.view.clone(),
                allocation_size: target.allocation_size,
                render_size: target.render_size,
                origin: Point::new(0, 0),
                scissor: Rect::from_xywh(0, 0, target.render_size.width, target.render_size.height),
                scale_factor: target.scale_factor,
                clear_color: target.clear_color,
                samples: self.options.antialiasing.sample_count(),
                load: false,
            },
            camera,
            draw_list,
        )
    }

    /// Records a scene into a compositor-owned rectangular frame region.
    pub fn render_composited(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        target: &CompositedRenderTarget,
        camera: &Camera2D,
        draw_list: &DrawList2D,
    ) -> Result<RenderStats, RenderError> {
        target.validate(self.device.id())?;
        self.render_impl(
            encoder,
            FrameTarget {
                view: target.view.clone(),
                allocation_size: target.size,
                render_size: target.viewport.size,
                origin: target.viewport.origin,
                scissor: target.scissor,
                scale_factor: target.scale_factor,
                clear_color: target.clear_color,
                samples: target.view.sample_count(),
                load: true,
            },
            camera,
            draw_list,
        )
    }

    fn render_impl(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        target: FrameTarget,
        camera: &Camera2D,
        draw_list: &DrawList2D,
    ) -> Result<RenderStats, RenderError> {
        astrelis_profiling::profile_function!();
        if target.render_size.width == 0 || target.render_size.height == 0 {
            return Ok(RenderStats::default());
        }
        let logical_size = Vec2::new(
            target.render_size.width as f32 / target.scale_factor,
            target.render_size.height as f32 / target.scale_factor,
        );
        let matrix = camera
            .view_projection(logical_size)
            .ok_or_else(|| RenderError::new("invalid 2D camera or viewport"))?;
        let packed: astrelis_core::math::packed::Mat4 = matrix.into();
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&packed))?;
        let (view_min, view_max) = camera.visible_bounds(logical_size).unwrap();
        let mut prepared = Vec::with_capacity(draw_list.sprites.len());
        let mut stats = RenderStats::default();
        for (order, sprite) in draw_list.sprites.iter().enumerate() {
            if !sprite.size.is_finite()
                || sprite.size.min_element() <= 0.0
                || !sprite.pivot.is_finite()
            {
                return Err(RenderError::new(
                    "sprite geometry must be finite and non-empty",
                ));
            }
            let texture = self.texture(sprite.texture)?;
            let (min, max) = scene::transformed_bounds(
                sprite.transform,
                -sprite.pivot * sprite.size,
                (Vec2::ONE - sprite.pivot) * sprite.size,
            );
            if max.x < view_min.x || max.y < view_min.y || min.x > view_max.x || min.y > view_max.y
            {
                stats.culled += 1;
                continue;
            }
            let uv = if let Some(source) = sprite.source {
                if source.origin.x < 0.0
                    || source.origin.y < 0.0
                    || source.size.width <= 0.0
                    || source.size.height <= 0.0
                    || source.origin.x + source.size.width > texture.size.width as f32
                    || source.origin.y + source.size.height > texture.size.height as f32
                {
                    return Err(RenderError::new(
                        "sprite source rectangle exceeds its texture",
                    ));
                }
                [
                    source.origin.x / texture.size.width as f32,
                    source.origin.y / texture.size.height as f32,
                    (source.origin.x + source.size.width) / texture.size.width as f32,
                    (source.origin.y + source.size.height) / texture.size.height as f32,
                ]
            } else {
                [0.0, 0.0, 1.0, 1.0]
            };
            let columns = sprite.transform.matrix2.to_cols_array();
            prepared.push(Prepared {
                texture: sprite.texture,
                layer: sprite.layer,
                order,
                instance: Instance {
                    basis_x: [columns[0], columns[1]],
                    basis_y: [columns[2], columns[3]],
                    translation: sprite.transform.translation.to_array(),
                    size: sprite.size.to_array(),
                    pivot: sprite.pivot.to_array(),
                    uv_rect: uv,
                    color: [sprite.tint.r, sprite.tint.g, sprite.tint.b, sprite.tint.a],
                },
            });
        }
        prepared.sort_by_key(|draw| (draw.layer, draw.order));
        let sample_count = target.samples;
        self.ensure_pipeline(target.view.format(), sample_count)?;
        if !target.load {
            self.ensure_attachments(
                &RenderTarget {
                    view: target.view.clone(),
                    allocation_size: target.allocation_size,
                    render_size: target.render_size,
                    scale_factor: target.scale_factor,
                    clear_color: target.clear_color,
                },
                sample_count,
            );
        }
        let instances = prepared
            .iter()
            .map(|draw| draw.instance)
            .collect::<Vec<_>>();
        let instance_buffer = if instances.is_empty() {
            None
        } else {
            Some(self.device.create_buffer_init(
                &self.queue,
                Some("render-2d frame instances".into()),
                bytemuck::cast_slice(&instances),
                gpu::BufferUsages::VERTEX,
            )?)
        };
        let attachment_key = (
            target.allocation_size.width,
            target.allocation_size.height,
            target.view.format(),
            sample_count,
        );
        let attachments = self
            .attachments
            .iter()
            .find(|attachments| attachments.key == attachment_key);
        let color_view = if target.load {
            target.view.clone()
        } else {
            attachments
                .expect("attachments were ensured")
                .view
                .clone()
                .unwrap_or_else(|| target.view.clone())
        };
        let resolve_target = if target.load {
            None
        } else {
            attachments.and_then(|value| value.view.as_ref().map(|_| target.view.clone()))
        };
        let clear = target.clear_color;
        let mut pass = encoder.begin_render_pass(gpu::RenderPassDescriptor {
            label: Some("render-2d scene".into()),
            color_attachments: vec![Some(gpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target,
                load: if target.load {
                    gpu::LoadOp::Load
                } else {
                    gpu::LoadOp::Clear(gpu::Color {
                        r: clear.r as f64,
                        g: clear.g as f64,
                        b: clear.b as f64,
                        a: clear.a as f64,
                    })
                },
                store: gpu::StoreOp::Store,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
        })?;
        pass.set_viewport(
            target.origin.x as f32,
            target.origin.y as f32,
            target.render_size.width as f32,
            target.render_size.height as f32,
            0.0,
            1.0,
        );
        pass.set_scissor_rect(
            target.scissor.origin.x,
            target.scissor.origin.y,
            target.scissor.size.width,
            target.scissor.size.height,
        );
        if let Some(buffer) = &instance_buffer {
            pass.set_pipeline(
                self.pipelines
                    .get(&PipelineKey(target.view.format(), sample_count))
                    .unwrap(),
            )?;
            pass.set_bind_group(0, &self.camera_bind_group, &[])?;
            pass.set_vertex_buffer(0, buffer, 0..buffer.size())?;
            let mut start = 0;
            while start < prepared.len() {
                let texture = prepared[start].texture;
                let mut end = start + 1;
                while end < prepared.len() && prepared[end].texture == texture {
                    end += 1;
                }
                pass.set_bind_group(1, &self.texture(texture)?.bind_group, &[])?;
                pass.draw(0..6, start as u32..end as u32);
                stats.draw_calls += 1;
                start = end;
            }
        }
        stats.instances = prepared.len() as u32;
        stats.triangles = stats.instances * 2;
        Ok(stats)
    }

    fn insert_texture(
        &mut self,
        texture: Option<gpu::Texture>,
        view: gpu::TextureView,
        size: Size<Physical, u32>,
        options: TextureOptions,
    ) -> Result<TextureHandle, RenderError> {
        if view.device_id() != self.device.id()
            || view.sample_count() != 1
            || view.dimension() != gpu::TextureDimension::D2
            || size.width == 0
            || size.height == 0
        {
            return Err(RenderError::new("incompatible 2D texture registration"));
        }
        let sampler = self.device.create_sampler(gpu::SamplerDescriptor {
            address_mode_u: options.address_u,
            address_mode_v: options.address_v,
            mag_filter: options.filtering,
            min_filter: options.filtering,
            ..Default::default()
        });
        let bind_group = self.device.create_bind_group(gpu::BindGroupDescriptor {
            label: Some("render-2d texture bind group".into()),
            layout: self.texture_layout.clone(),
            entries: vec![
                gpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu::BindingResource::TextureView(view.clone()),
                },
                gpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu::BindingResource::Sampler(sampler),
                },
            ],
        })?;
        let resource = TextureResource {
            _texture: texture,
            _view: view,
            bind_group,
            size,
        };
        if let Some((index, slot)) = self
            .textures
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.resource.is_none())
        {
            slot.resource = Some(resource);
            return Ok(TextureHandle {
                owner: self.owner,
                slot: index as u32,
                generation: slot.generation,
            });
        }
        let slot = self.textures.len() as u32;
        self.textures.push(TextureSlot {
            generation: 0,
            resource: Some(resource),
        });
        Ok(TextureHandle {
            owner: self.owner,
            slot,
            generation: 0,
        })
    }

    fn texture(&self, handle: TextureHandle) -> Result<&TextureResource, RenderError> {
        if handle.owner != self.owner {
            return Err(RenderError::new(
                "texture handle belongs to another renderer",
            ));
        }
        let slot = self
            .textures
            .get(handle.slot as usize)
            .ok_or_else(|| RenderError::new("invalid texture handle"))?;
        if slot.generation != handle.generation {
            return Err(RenderError::new("stale texture handle"));
        }
        slot.resource
            .as_ref()
            .ok_or_else(|| RenderError::new("removed texture handle"))
    }

    fn texture_slot_mut(&mut self, handle: TextureHandle) -> Result<&mut TextureSlot, RenderError> {
        if handle.owner != self.owner {
            return Err(RenderError::new(
                "texture handle belongs to another renderer",
            ));
        }
        let slot = self
            .textures
            .get_mut(handle.slot as usize)
            .ok_or_else(|| RenderError::new("invalid texture handle"))?;
        if slot.generation != handle.generation || slot.resource.is_none() {
            return Err(RenderError::new("stale or removed texture handle"));
        }
        Ok(slot)
    }

    fn ensure_pipeline(
        &mut self,
        format: gpu::TextureFormat,
        samples: u32,
    ) -> Result<(), RenderError> {
        let key = PipelineKey(format, samples);
        if self.pipelines.contains_key(&key) {
            return Ok(());
        }
        let shader = self
            .device
            .create_shader_module(gpu::ShaderModuleDescriptor {
                label: Some("render-2d shader".into()),
                wgsl: SHADER.into(),
            });
        let layout = self
            .device
            .create_pipeline_layout(gpu::PipelineLayoutDescriptor {
                label: Some("render-2d pipeline layout".into()),
                bind_group_layouts: vec![self.camera_layout.clone(), self.texture_layout.clone()],
            })?;
        let attributes = [
            (0, 0, gpu::VertexFormat::Float32x2),
            (8, 1, gpu::VertexFormat::Float32x2),
            (16, 2, gpu::VertexFormat::Float32x2),
            (24, 3, gpu::VertexFormat::Float32x2),
            (32, 4, gpu::VertexFormat::Float32x2),
            (40, 5, gpu::VertexFormat::Float32x4),
            (56, 6, gpu::VertexFormat::Float32x4),
        ]
        .into_iter()
        .map(|(offset, shader_location, format)| gpu::VertexAttribute {
            offset,
            shader_location,
            format,
        })
        .collect();
        let pipeline = self
            .device
            .create_render_pipeline(gpu::RenderPipelineDescriptor {
                label: Some("render-2d pipeline".into()),
                layout: Some(layout),
                vertex: gpu::VertexState {
                    module: shader.clone(),
                    entry_point: "vs_main".into(),
                    buffers: vec![gpu::VertexBufferLayout {
                        array_stride: size_of::<Instance>() as u64,
                        step_mode: gpu::VertexStepMode::Instance,
                        attributes,
                    }],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: gpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(gpu::FragmentState {
                    module: shader,
                    entry_point: "fs_main".into(),
                    targets: vec![Some(gpu::ColorTargetState {
                        format,
                        blend: Some(gpu::BlendState::PREMULTIPLIED_ALPHA),
                        write_mask: gpu::ColorWrites::ALL,
                    })],
                }),
            })?;
        self.pipelines.insert(key, pipeline);
        Ok(())
    }

    fn ensure_attachments(&mut self, target: &RenderTarget, samples: u32) {
        let key = (
            target.allocation_size.width,
            target.allocation_size.height,
            target.view.format(),
            samples,
        );
        if self
            .attachments
            .iter()
            .any(|attachments| attachments.key == key)
        {
            return;
        }
        let (texture, view) = if samples > 1 {
            let texture = self.device.create_texture(gpu::TextureDescriptor {
                label: Some("render-2d multisample color".into()),
                size: gpu::Extent3d::d2(
                    target.allocation_size.width,
                    target.allocation_size.height,
                ),
                mip_level_count: 1,
                sample_count: samples,
                dimension: gpu::TextureDimension::D2,
                format: target.view.format(),
                usage: gpu::TextureUsages::RENDER_ATTACHMENT,
            });
            let view = texture.create_view(Default::default());
            (Some(texture), Some(view))
        } else {
            (None, None)
        };
        if self.attachments.len() == 4 {
            self.attachments.remove(0);
        }
        self.attachments.push(Attachments {
            key,
            _texture: texture,
            view,
        });
    }
}

/// 2D renderer failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderError(String);

impl RenderError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for RenderError {}

impl From<gpu::GpuError> for RenderError {
    fn from(value: gpu::GpuError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<astrelis_render::TargetError> for RenderError {
    fn from(value: astrelis_render::TargetError) -> Self {
        Self::new(value.to_string())
    }
}
