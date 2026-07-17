//! Basic forward-lit three-dimensional scene rendering for Astrelis.

#![warn(missing_docs)]

mod camera;
mod mesh;
mod scene;

pub use camera::Camera3D;
pub use mesh::{MeshData, MeshVertex, cube, plane, uv_sphere};
pub use scene::{
    AlphaMode, DebugLine, DirectionalLight, DrawList3D, Lighting, MaterialDescriptor, MeshDraw,
};

use std::{
    collections::HashMap,
    error::Error,
    fmt,
    mem::size_of,
    sync::atomic::{AtomicU64, Ordering},
};

use astrelis_core::{
    geometry::{Physical, Size},
    math::{Mat3, Vec3},
};
use astrelis_gpu as gpu;
use astrelis_render::{Antialiasing, RenderStats, RenderTarget};
use bytemuck::{Pod, Zeroable};

const SHADER: &str = include_str!("shader.wgsl");
static NEXT_RENDERER: AtomicU64 = AtomicU64::new(1);

/// Device-bound 3D renderer configuration.
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

macro_rules! resource_handle {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name {
            owner: u64,
            slot: u32,
            generation: u32,
        }
    };
}

resource_handle!(
    TextureHandle,
    "Generational 3D texture handle owned by one renderer."
);
resource_handle!(
    MeshHandle,
    "Generational GPU mesh handle owned by one renderer."
);
resource_handle!(
    MaterialHandle,
    "Generational Lambert material handle owned by one renderer."
);

/// Texture filtering and addressing options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureOptions {
    /// Magnification/minification filter.
    pub filtering: gpu::FilterMode,
    /// Horizontal address mode.
    pub address_u: gpu::AddressMode,
    /// Vertical address mode.
    pub address_v: gpu::AddressMode,
}

struct Slot<T> {
    generation: u32,
    value: Option<T>,
}
struct TextureResource {
    _texture: Option<gpu::Texture>,
    view: gpu::TextureView,
    sampler: gpu::Sampler,
    _size: Size<Physical, u32>,
}
struct MeshResource {
    vertex: gpu::Buffer,
    index: gpu::Buffer,
    indices: u32,
    center: Vec3,
    radius: f32,
}
struct MaterialResource {
    _buffer: gpu::Buffer,
    bind_group: gpu::BindGroup,
    alpha: AlphaMode,
    double_sided: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameUniform {
    view_projection: [f32; 16],
    camera_position: [f32; 4],
    ambient: [f32; 4],
    light_direction_intensity: [f32; 4],
    light_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaterialUniform {
    base_color: [f32; 4],
    alpha: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MeshInstance {
    model: [f32; 16],
    normal_0: [f32; 4],
    normal_1: [f32; 4],
    normal_2: [f32; 4],
    tint: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LineVertex {
    position: [f32; 3],
    color: [f32; 4],
}

struct Prepared {
    mesh: MeshHandle,
    material: MaterialHandle,
    instance: MeshInstance,
    distance: f32,
    blended: bool,
    order: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MeshPipelineKey(gpu::TextureFormat, u32, u8, bool);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct LinePipelineKey(gpu::TextureFormat, u32);
struct Attachments {
    key: (u32, u32, gpu::TextureFormat, u32),
    _color_texture: Option<gpu::Texture>,
    color: Option<gpu::TextureView>,
    _depth_texture: gpu::Texture,
    depth: gpu::TextureView,
}

/// Basic forward renderer tied to one device and queue.
pub struct Renderer3D {
    owner: u64,
    device: gpu::Device,
    queue: gpu::Queue,
    options: RendererOptions,
    frame_layout: gpu::BindGroupLayout,
    material_layout: gpu::BindGroupLayout,
    frame_buffer: gpu::Buffer,
    frame_bind_group: gpu::BindGroup,
    white_view: gpu::TextureView,
    white_sampler: gpu::Sampler,
    textures: Vec<Slot<TextureResource>>,
    meshes: Vec<Slot<MeshResource>>,
    materials: Vec<Slot<MaterialResource>>,
    mesh_pipelines: HashMap<MeshPipelineKey, gpu::RenderPipeline>,
    line_pipelines: HashMap<LinePipelineKey, gpu::RenderPipeline>,
    attachments: Vec<Attachments>,
}

impl Renderer3D {
    /// Creates a renderer for one matching device/queue pair.
    pub fn new(
        device: gpu::Device,
        queue: gpu::Queue,
        options: RendererOptions,
    ) -> Result<Self, RenderError> {
        if device.id() != queue.device_id() {
            return Err(RenderError::new("device and queue do not match"));
        }
        let frame_layout = device.create_bind_group_layout(gpu::BindGroupLayoutDescriptor {
            label: Some("render-3d frame layout".into()),
            entries: vec![gpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: gpu::ShaderStages::VERTEX | gpu::ShaderStages::FRAGMENT,
                ty: gpu::BindingType::Buffer {
                    ty: gpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            }],
        });
        let material_layout = device.create_bind_group_layout(gpu::BindGroupLayoutDescriptor {
            label: Some("render-3d material layout".into()),
            entries: vec![
                gpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Buffer {
                        ty: gpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                },
                gpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Texture {
                        sample_type: gpu::TextureSampleType::Float,
                        view_dimension: gpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                },
                gpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Sampler(gpu::SamplerBindingType::Filtering),
                },
            ],
        });
        let frame_buffer = device.create_buffer_init(
            &queue,
            Some("render-3d frame".into()),
            bytemuck::bytes_of(&FrameUniform::zeroed()),
            gpu::BufferUsages::UNIFORM,
        )?;
        let frame_bind_group = device.create_bind_group(gpu::BindGroupDescriptor {
            label: Some("render-3d frame bind group".into()),
            layout: frame_layout.clone(),
            entries: vec![gpu::BindGroupEntry {
                binding: 0,
                resource: gpu::BindingResource::Buffer(gpu::BufferBinding {
                    buffer: frame_buffer.clone(),
                    offset: 0,
                    size: None,
                }),
            }],
        })?;
        let white_texture = device.create_texture(gpu::TextureDescriptor {
            label: Some("render-3d white texture".into()),
            size: gpu::Extent3d::d2(1, 1),
            mip_level_count: 1,
            sample_count: 1,
            dimension: gpu::TextureDimension::D2,
            format: gpu::TextureFormat::Rgba8UnormSrgb,
            usage: gpu::TextureUsages::TEXTURE_BINDING | gpu::TextureUsages::COPY_DST,
        });
        queue.write_texture(
            &gpu::TextureCopy {
                texture: white_texture.clone(),
                mip_level: 0,
                origin: Default::default(),
            },
            &[255; 4],
            gpu::TextureDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            gpu::Extent3d::d2(1, 1),
        )?;
        let white_view = white_texture.create_view(Default::default());
        let white_sampler = device.create_sampler(gpu::SamplerDescriptor {
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });
        Ok(Self {
            owner: NEXT_RENDERER.fetch_add(1, Ordering::Relaxed),
            device,
            queue,
            options,
            frame_layout,
            material_layout,
            frame_buffer,
            frame_bind_group,
            white_view,
            white_sampler,
            textures: Vec::new(),
            meshes: Vec::new(),
            materials: Vec::new(),
            mesh_pipelines: HashMap::new(),
            line_pipelines: HashMap::new(),
            attachments: Vec::new(),
        })
    }

    /// Uploads an immutable straight-alpha RGBA8 sRGB texture.
    pub fn create_texture_rgba8(
        &mut self,
        size: Size<Physical, u32>,
        bytes: &[u8],
        options: TextureOptions,
    ) -> Result<TextureHandle, RenderError> {
        let expected = u64::from(size.width)
            .checked_mul(u64::from(size.height))
            .and_then(|v| v.checked_mul(4))
            .ok_or_else(|| RenderError::new("texture size overflow"))?;
        if size.width == 0 || size.height == 0 || bytes.len() as u64 != expected {
            return Err(RenderError::new(
                "invalid RGBA8 texture dimensions or byte length",
            ));
        }
        let texture = self.device.create_texture(gpu::TextureDescriptor {
            label: Some("render-3d uploaded texture".into()),
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

    /// Removes a texture handle. Materials already created from it retain their GPU binding.
    pub fn remove_texture(&mut self, handle: TextureHandle) -> Result<(), RenderError> {
        remove_slot(
            self.owner,
            &mut self.textures,
            handle.owner,
            handle.slot,
            handle.generation,
        )
    }

    /// Uploads a validated indexed mesh.
    pub fn create_mesh(&mut self, data: &MeshData) -> Result<MeshHandle, RenderError> {
        data.validate().map_err(RenderError::new)?;
        let vertex = self.device.create_buffer_init(
            &self.queue,
            Some("render-3d mesh vertices".into()),
            bytemuck::cast_slice(&data.vertices),
            gpu::BufferUsages::VERTEX,
        )?;
        let index = self.device.create_buffer_init(
            &self.queue,
            Some("render-3d mesh indices".into()),
            bytemuck::cast_slice(&data.indices),
            gpu::BufferUsages::INDEX,
        )?;
        let (center, radius) = data.bounding_sphere();
        Ok(insert_slot(
            self.owner,
            &mut self.meshes,
            MeshResource {
                vertex,
                index,
                indices: data.indices.len() as u32,
                center,
                radius,
            },
            |owner, slot, generation| MeshHandle {
                owner,
                slot,
                generation,
            },
        ))
    }

    /// Removes a mesh handle.
    pub fn remove_mesh(&mut self, handle: MeshHandle) -> Result<(), RenderError> {
        remove_slot(
            self.owner,
            &mut self.meshes,
            handle.owner,
            handle.slot,
            handle.generation,
        )
    }

    /// Creates a retained Lambert material.
    pub fn create_material(
        &mut self,
        descriptor: MaterialDescriptor,
    ) -> Result<MaterialHandle, RenderError> {
        if !descriptor.base_color.r.is_finite()
            || !descriptor.base_color.g.is_finite()
            || !descriptor.base_color.b.is_finite()
            || !descriptor.base_color.a.is_finite()
        {
            return Err(RenderError::new("material color must be finite"));
        }
        let cutoff = match descriptor.alpha_mode {
            AlphaMode::Mask(value) if value.is_finite() && (0.0..=1.0).contains(&value) => value,
            AlphaMode::Mask(_) => {
                return Err(RenderError::new("alpha cutoff must be within zero and one"));
            }
            _ => 0.0,
        };
        let mode = match descriptor.alpha_mode {
            AlphaMode::Opaque => 0.0,
            AlphaMode::Mask(_) => 1.0,
            AlphaMode::Blend => 2.0,
        };
        let uniform = MaterialUniform {
            base_color: descriptor.base_color.into(),
            alpha: [cutoff, mode, 0.0, 0.0],
        };
        let buffer = self.device.create_buffer_init(
            &self.queue,
            Some("render-3d material".into()),
            bytemuck::bytes_of(&uniform),
            gpu::BufferUsages::UNIFORM,
        )?;
        let (view, sampler) = if let Some(handle) = descriptor.albedo {
            let texture = get_slot(
                self.owner,
                &self.textures,
                handle.owner,
                handle.slot,
                handle.generation,
                "texture",
            )?;
            (texture.view.clone(), texture.sampler.clone())
        } else {
            (self.white_view.clone(), self.white_sampler.clone())
        };
        let bind_group = self.device.create_bind_group(gpu::BindGroupDescriptor {
            label: Some("render-3d material bind group".into()),
            layout: self.material_layout.clone(),
            entries: vec![
                gpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu::BindingResource::Buffer(gpu::BufferBinding {
                        buffer: buffer.clone(),
                        offset: 0,
                        size: None,
                    }),
                },
                gpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu::BindingResource::TextureView(view),
                },
                gpu::BindGroupEntry {
                    binding: 2,
                    resource: gpu::BindingResource::Sampler(sampler),
                },
            ],
        })?;
        Ok(insert_slot(
            self.owner,
            &mut self.materials,
            MaterialResource {
                _buffer: buffer,
                bind_group,
                alpha: descriptor.alpha_mode,
                double_sided: descriptor.double_sided,
            },
            |owner, slot, generation| MaterialHandle {
                owner,
                slot,
                generation,
            },
        ))
    }

    /// Removes a material handle.
    pub fn remove_material(&mut self, handle: MaterialHandle) -> Result<(), RenderError> {
        remove_slot(
            self.owner,
            &mut self.materials,
            handle.owner,
            handle.slot,
            handle.generation,
        )
    }

    /// Clears and renders one camera-specific mesh and debug draw list.
    pub fn render(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        target: &RenderTarget,
        camera: &Camera3D,
        lighting: &Lighting,
        draw_list: &DrawList3D,
    ) -> Result<RenderStats, RenderError> {
        astrelis_profiling::profile_function!();
        target.validate(self.device.id())?;
        if target.is_empty() {
            return Ok(RenderStats::default());
        }
        let aspect = target.render_size.width as f32 / target.render_size.height as f32;
        let view_projection = camera
            .view_projection(aspect)
            .ok_or_else(|| RenderError::new("invalid 3D camera or target aspect"))?;
        if !lighting.ambient_intensity.is_finite()
            || lighting.ambient_intensity < 0.0
            || !lighting.directional.intensity.is_finite()
            || lighting.directional.intensity < 0.0
            || !lighting.directional.direction_to_light.is_finite()
            || lighting.directional.direction_to_light.length_squared() < 1e-8
        {
            return Err(RenderError::new(
                "lighting directions and intensities must be valid",
            ));
        }
        let direction = lighting.directional.direction_to_light.normalize();
        let frame = FrameUniform {
            view_projection: view_projection.to_cols_array(),
            camera_position: camera.position.extend(1.0).to_array(),
            ambient: [
                lighting.ambient_color.r * lighting.ambient_intensity,
                lighting.ambient_color.g * lighting.ambient_intensity,
                lighting.ambient_color.b * lighting.ambient_intensity,
                1.0,
            ],
            light_direction_intensity: [
                direction.x,
                direction.y,
                direction.z,
                lighting.directional.intensity,
            ],
            light_color: lighting.directional.color.into(),
        };
        self.queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(&frame))?;
        let mut prepared = Vec::with_capacity(draw_list.meshes.len());
        let mut stats = RenderStats::default();
        for (order, draw) in draw_list.meshes.iter().enumerate() {
            if !draw.transform.is_finite() {
                return Err(RenderError::new("mesh transform must be finite"));
            }
            let mesh = get_slot(
                self.owner,
                &self.meshes,
                draw.mesh.owner,
                draw.mesh.slot,
                draw.mesh.generation,
                "mesh",
            )?;
            let material = get_slot(
                self.owner,
                &self.materials,
                draw.material.owner,
                draw.material.slot,
                draw.material.generation,
                "material",
            )?;
            let center = draw.transform.transform_point3(mesh.center);
            let scale = draw
                .transform
                .x_axis
                .truncate()
                .length()
                .max(draw.transform.y_axis.truncate().length())
                .max(draw.transform.z_axis.truncate().length());
            if !camera.sphere_visible(center, mesh.radius * scale, aspect) {
                stats.culled += 1;
                continue;
            }
            let normal = Mat3::from_mat4(draw.transform).inverse().transpose();
            let columns = normal.to_cols_array_2d();
            prepared.push(Prepared {
                mesh: draw.mesh,
                material: draw.material,
                instance: MeshInstance {
                    model: draw.transform.to_cols_array(),
                    normal_0: [columns[0][0], columns[0][1], columns[0][2], 0.0],
                    normal_1: [columns[1][0], columns[1][1], columns[1][2], 0.0],
                    normal_2: [columns[2][0], columns[2][1], columns[2][2], 0.0],
                    tint: draw.tint.into(),
                },
                distance: center.distance_squared(camera.position),
                blended: matches!(material.alpha, AlphaMode::Blend),
                order,
            });
        }
        prepared.sort_by(|a, b| match (a.blended, b.blended) {
            (false, false) => (a.material.slot, a.mesh.slot, a.order).cmp(&(
                b.material.slot,
                b.mesh.slot,
                b.order,
            )),
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            (true, true) => b
                .distance
                .total_cmp(&a.distance)
                .then(a.order.cmp(&b.order)),
        });
        let sample_count = self.options.antialiasing.sample_count();
        let material_keys = prepared
            .iter()
            .map(|draw| {
                let material = get_slot(
                    self.owner,
                    &self.materials,
                    draw.material.owner,
                    draw.material.slot,
                    draw.material.generation,
                    "material",
                )?;
                Ok((alpha_key(material.alpha), material.double_sided))
            })
            .collect::<Result<Vec<_>, RenderError>>()?;
        for &(alpha, double_sided) in &material_keys {
            self.ensure_mesh_pipeline(target.view.format(), sample_count, alpha, double_sided)?;
        }
        if !draw_list.lines.is_empty() {
            self.ensure_line_pipeline(target.view.format(), sample_count)?;
        }
        self.ensure_attachments(target, sample_count);
        let instance_data = prepared
            .iter()
            .map(|draw| draw.instance)
            .collect::<Vec<_>>();
        let instance_buffer = if instance_data.is_empty() {
            None
        } else {
            Some(self.device.create_buffer_init(
                &self.queue,
                Some("render-3d frame instances".into()),
                bytemuck::cast_slice(&instance_data),
                gpu::BufferUsages::VERTEX,
            )?)
        };
        let mut line_vertices = Vec::with_capacity(draw_list.lines.len() * 2);
        for line in &draw_list.lines {
            if !line.start.is_finite() || !line.end.is_finite() {
                return Err(RenderError::new("debug line positions must be finite"));
            }
            line_vertices.push(LineVertex {
                position: line.start.to_array(),
                color: line.color.into(),
            });
            line_vertices.push(LineVertex {
                position: line.end.to_array(),
                color: line.color.into(),
            });
        }
        let line_buffer = if line_vertices.is_empty() {
            None
        } else {
            Some(self.device.create_buffer_init(
                &self.queue,
                Some("render-3d debug lines".into()),
                bytemuck::cast_slice(&line_vertices),
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
            .find(|attachments| attachments.key == attachment_key)
            .expect("attachments were ensured");
        let color_view = attachments
            .color
            .clone()
            .unwrap_or_else(|| target.view.clone());
        let resolve_target = attachments.color.as_ref().map(|_| target.view.clone());
        let clear = target.clear_color;
        let mut pass = encoder.begin_render_pass(gpu::RenderPassDescriptor {
            label: Some("render-3d scene".into()),
            color_attachments: vec![Some(gpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target,
                load: gpu::LoadOp::Clear(gpu::Color {
                    r: clear.r as f64,
                    g: clear.g as f64,
                    b: clear.b as f64,
                    a: clear.a as f64,
                }),
                store: gpu::StoreOp::Store,
            })],
            depth_stencil_attachment: Some(gpu::RenderPassDepthStencilAttachment {
                view: attachments.depth.clone(),
                depth_ops: Some(gpu::AttachmentOperations {
                    load: gpu::LoadOpValue::Clear(0.0),
                    store: gpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
        })?;
        pass.set_viewport(
            0.0,
            0.0,
            target.render_size.width as f32,
            target.render_size.height as f32,
            0.0,
            1.0,
        );
        pass.set_scissor_rect(0, 0, target.render_size.width, target.render_size.height);
        pass.set_bind_group(0, &self.frame_bind_group, &[])?;
        if let Some(buffer) = &instance_buffer {
            pass.set_vertex_buffer(1, buffer, 0..buffer.size())?;
            let mut start = 0;
            while start < prepared.len() {
                let draw = &prepared[start];
                let (alpha, double_sided) = material_keys[start];
                let mut end = start + 1;
                while end < prepared.len()
                    && prepared[end].mesh == draw.mesh
                    && prepared[end].material == draw.material
                {
                    end += 1;
                }
                let mesh = get_slot(
                    self.owner,
                    &self.meshes,
                    draw.mesh.owner,
                    draw.mesh.slot,
                    draw.mesh.generation,
                    "mesh",
                )?;
                let material = get_slot(
                    self.owner,
                    &self.materials,
                    draw.material.owner,
                    draw.material.slot,
                    draw.material.generation,
                    "material",
                )?;
                pass.set_pipeline(
                    self.mesh_pipelines
                        .get(&MeshPipelineKey(
                            target.view.format(),
                            sample_count,
                            alpha,
                            double_sided,
                        ))
                        .unwrap(),
                )?;
                pass.set_bind_group(1, &material.bind_group, &[])?;
                pass.set_vertex_buffer(0, &mesh.vertex, 0..mesh.vertex.size())?;
                pass.set_index_buffer(&mesh.index, 0..mesh.index.size(), gpu::IndexFormat::Uint32)?;
                pass.draw_indexed(0..mesh.indices, 0, start as u32..end as u32);
                stats.draw_calls += 1;
                stats.triangles += (mesh.indices / 3) * (end - start) as u32;
                start = end;
            }
        }
        if let Some(buffer) = &line_buffer {
            pass.set_pipeline(
                self.line_pipelines
                    .get(&LinePipelineKey(target.view.format(), sample_count))
                    .unwrap(),
            )?;
            pass.set_vertex_buffer(0, buffer, 0..buffer.size())?;
            pass.draw(0..line_vertices.len() as u32, 0..1);
            stats.draw_calls += 1;
        }
        stats.instances = prepared.len() as u32;
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
            return Err(RenderError::new("incompatible 3D texture registration"));
        }
        let sampler = self.device.create_sampler(gpu::SamplerDescriptor {
            address_mode_u: options.address_u,
            address_mode_v: options.address_v,
            mag_filter: options.filtering,
            min_filter: options.filtering,
            ..Default::default()
        });
        Ok(insert_slot(
            self.owner,
            &mut self.textures,
            TextureResource {
                _texture: texture,
                view,
                sampler,
                _size: size,
            },
            |owner, slot, generation| TextureHandle {
                owner,
                slot,
                generation,
            },
        ))
    }

    fn ensure_mesh_pipeline(
        &mut self,
        format: gpu::TextureFormat,
        samples: u32,
        alpha: u8,
        double_sided: bool,
    ) -> Result<(), RenderError> {
        let key = MeshPipelineKey(format, samples, alpha, double_sided);
        if self.mesh_pipelines.contains_key(&key) {
            return Ok(());
        }
        let shader = self
            .device
            .create_shader_module(gpu::ShaderModuleDescriptor {
                label: Some("render-3d shader".into()),
                wgsl: SHADER.into(),
            });
        let layout = self
            .device
            .create_pipeline_layout(gpu::PipelineLayoutDescriptor {
                label: Some("render-3d mesh pipeline layout".into()),
                bind_group_layouts: vec![self.frame_layout.clone(), self.material_layout.clone()],
            })?;
        let mesh_attributes = [
            (0, 0, gpu::VertexFormat::Float32x3),
            (12, 1, gpu::VertexFormat::Float32x3),
            (24, 2, gpu::VertexFormat::Float32x2),
            (32, 3, gpu::VertexFormat::Float32x4),
        ]
        .into_iter()
        .map(|(offset, shader_location, format)| gpu::VertexAttribute {
            offset,
            shader_location,
            format,
        })
        .collect();
        let instance_attributes = (0..8)
            .map(|index| gpu::VertexAttribute {
                offset: index * 16,
                shader_location: 4 + index as u32,
                format: gpu::VertexFormat::Float32x4,
            })
            .collect();
        let blend = if alpha == 2 {
            Some(gpu::BlendState::PREMULTIPLIED_ALPHA)
        } else {
            None
        };
        let pipeline = self
            .device
            .create_render_pipeline(gpu::RenderPipelineDescriptor {
                label: Some("render-3d mesh pipeline".into()),
                layout: Some(layout),
                vertex: gpu::VertexState {
                    module: shader.clone(),
                    entry_point: "vs_mesh".into(),
                    buffers: vec![
                        gpu::VertexBufferLayout {
                            array_stride: size_of::<MeshVertex>() as u64,
                            step_mode: gpu::VertexStepMode::Vertex,
                            attributes: mesh_attributes,
                        },
                        gpu::VertexBufferLayout {
                            array_stride: size_of::<MeshInstance>() as u64,
                            step_mode: gpu::VertexStepMode::Instance,
                            attributes: instance_attributes,
                        },
                    ],
                },
                primitive: gpu::PrimitiveState {
                    cull_mode: if double_sided {
                        None
                    } else {
                        Some(gpu::Face::Back)
                    },
                    ..Default::default()
                },
                depth_stencil: Some(depth_state(alpha != 2)),
                multisample: gpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(gpu::FragmentState {
                    module: shader,
                    entry_point: "fs_mesh".into(),
                    targets: vec![Some(gpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: gpu::ColorWrites::ALL,
                    })],
                }),
            })?;
        self.mesh_pipelines.insert(key, pipeline);
        Ok(())
    }

    fn ensure_line_pipeline(
        &mut self,
        format: gpu::TextureFormat,
        samples: u32,
    ) -> Result<(), RenderError> {
        let key = LinePipelineKey(format, samples);
        if self.line_pipelines.contains_key(&key) {
            return Ok(());
        }
        let shader = self
            .device
            .create_shader_module(gpu::ShaderModuleDescriptor {
                label: Some("render-3d line shader".into()),
                wgsl: SHADER.into(),
            });
        let layout = self
            .device
            .create_pipeline_layout(gpu::PipelineLayoutDescriptor {
                label: Some("render-3d line pipeline layout".into()),
                bind_group_layouts: vec![self.frame_layout.clone()],
            })?;
        let pipeline = self
            .device
            .create_render_pipeline(gpu::RenderPipelineDescriptor {
                label: Some("render-3d line pipeline".into()),
                layout: Some(layout),
                vertex: gpu::VertexState {
                    module: shader.clone(),
                    entry_point: "vs_line".into(),
                    buffers: vec![gpu::VertexBufferLayout {
                        array_stride: size_of::<LineVertex>() as u64,
                        step_mode: gpu::VertexStepMode::Vertex,
                        attributes: vec![
                            gpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: gpu::VertexFormat::Float32x3,
                            },
                            gpu::VertexAttribute {
                                offset: 12,
                                shader_location: 1,
                                format: gpu::VertexFormat::Float32x4,
                            },
                        ],
                    }],
                },
                primitive: gpu::PrimitiveState {
                    topology: gpu::PrimitiveTopology::LineList,
                    ..Default::default()
                },
                depth_stencil: Some(depth_state(false)),
                multisample: gpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(gpu::FragmentState {
                    module: shader,
                    entry_point: "fs_line".into(),
                    targets: vec![Some(gpu::ColorTargetState {
                        format,
                        blend: Some(gpu::BlendState::PREMULTIPLIED_ALPHA),
                        write_mask: gpu::ColorWrites::ALL,
                    })],
                }),
            })?;
        self.line_pipelines.insert(key, pipeline);
        Ok(())
    }

    fn ensure_attachments(&mut self, target: &RenderTarget, samples: u32) {
        let key = (
            target.allocation_size.width,
            target.allocation_size.height,
            target.view.format(),
            samples,
        );
        if self.attachments.iter().any(|value| value.key == key) {
            return;
        }
        let (color_texture, color) = if samples > 1 {
            let texture = self.device.create_texture(gpu::TextureDescriptor {
                label: Some("render-3d multisample color".into()),
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
        let depth_texture = self.device.create_texture(gpu::TextureDescriptor {
            label: Some("render-3d reverse-z depth".into()),
            size: gpu::Extent3d::d2(target.allocation_size.width, target.allocation_size.height),
            mip_level_count: 1,
            sample_count: samples,
            dimension: gpu::TextureDimension::D2,
            format: gpu::TextureFormat::Depth32Float,
            usage: gpu::TextureUsages::RENDER_ATTACHMENT,
        });
        let depth = depth_texture.create_view(Default::default());
        if self.attachments.len() == 4 {
            self.attachments.remove(0);
        }
        self.attachments.push(Attachments {
            key,
            _color_texture: color_texture,
            color,
            _depth_texture: depth_texture,
            depth,
        });
    }
}

fn depth_state(write: bool) -> gpu::DepthStencilState {
    gpu::DepthStencilState {
        format: gpu::TextureFormat::Depth32Float,
        depth_write_enabled: write,
        depth_compare: gpu::CompareFunction::GreaterEqual,
        stencil: gpu::StencilState {
            front: gpu::StencilFaceState::IGNORE,
            back: gpu::StencilFaceState::IGNORE,
            read_mask: 0,
            write_mask: 0,
        },
        bias_constant: 0,
        bias_slope_scale: 0.0,
        bias_clamp: 0.0,
    }
}

fn alpha_key(mode: AlphaMode) -> u8 {
    match mode {
        AlphaMode::Opaque => 0,
        AlphaMode::Mask(_) => 1,
        AlphaMode::Blend => 2,
    }
}

fn insert_slot<T, H>(
    owner: u64,
    slots: &mut Vec<Slot<T>>,
    value: T,
    make: impl FnOnce(u64, u32, u32) -> H,
) -> H {
    if let Some((index, slot)) = slots
        .iter_mut()
        .enumerate()
        .find(|(_, slot)| slot.value.is_none())
    {
        slot.value = Some(value);
        return make(owner, index as u32, slot.generation);
    }
    let index = slots.len() as u32;
    slots.push(Slot {
        generation: 0,
        value: Some(value),
    });
    make(owner, index, 0)
}

fn get_slot<'a, T>(
    owner: u64,
    slots: &'a [Slot<T>],
    handle_owner: u64,
    index: u32,
    generation: u32,
    kind: &str,
) -> Result<&'a T, RenderError> {
    if owner != handle_owner {
        return Err(RenderError::new(format!(
            "{kind} handle belongs to another renderer"
        )));
    }
    let slot = slots
        .get(index as usize)
        .ok_or_else(|| RenderError::new(format!("invalid {kind} handle")))?;
    if slot.generation != generation {
        return Err(RenderError::new(format!("stale {kind} handle")));
    }
    slot.value
        .as_ref()
        .ok_or_else(|| RenderError::new(format!("removed {kind} handle")))
}

fn remove_slot<T>(
    owner: u64,
    slots: &mut [Slot<T>],
    handle_owner: u64,
    index: u32,
    generation: u32,
) -> Result<(), RenderError> {
    if owner != handle_owner {
        return Err(RenderError::new(
            "resource handle belongs to another renderer",
        ));
    }
    let slot = slots
        .get_mut(index as usize)
        .ok_or_else(|| RenderError::new("invalid resource handle"))?;
    if slot.generation != generation || slot.value.is_none() {
        return Err(RenderError::new("stale or removed resource handle"));
    }
    slot.value = None;
    slot.generation = slot.generation.wrapping_add(1);
    Ok(())
}

/// 3D renderer failure.
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
