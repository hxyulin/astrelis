//! The 3D renderer: draw list, depth target, debug lines.

use astrelis_core::color::Color;
use astrelis_core::geometry::{Physical, Size};
use astrelis_core::math::{Mat4, Vec3};
use astrelis_gpu::buffer::{BufferDescriptor, BufferInitDescriptor, BufferUsages};
use astrelis_gpu::pipeline::{VertexAttribute, VertexBufferLayout};
use astrelis_gpu::resources::{BindGroup, Buffer, Texture, TextureView};
use astrelis_gpu::texture::{
    Extent3d, TextureDescriptor, TextureUsages, TextureViewDescriptor,
};
use astrelis_gpu::types::{
    IndexFormat, LoadOp, StoreOp, TextureDimension, TextureFormat, VertexFormat, VertexStepMode,
};
use astrelis_gpu::Gpu;

use crate::camera::Camera3D;
use crate::debug::{axes_segments, grid_segments};
use crate::mesh::{MeshData, MeshHandle};
use crate::pipeline::{Pipeline3D, DEPTH_FORMAT};

/// A debug-line vertex (28 bytes): world-space position + color.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LineVertex {
    /// World-space position of this endpoint.
    pub position: [f32; 3],
    /// RGBA color.
    pub color: [f32; 4],
}

impl LineVertex {
    /// Vertex buffer layout matching the WGSL line vertex inputs.
    #[must_use]
    pub(crate) fn layout() -> VertexBufferLayout<'static> {
        const ATTRS: [VertexAttribute; 2] = [
            VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 },
            VertexAttribute { format: VertexFormat::Float32x4, offset: 12, shader_location: 1 },
        ];
        VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

/// Per-draw GPU data: world matrix + tint, indexed by instance index.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawData {
    /// Column-major world transform.
    world: [[f32; 4]; 4],
    /// RGBA tint multiplied with per-vertex color.
    tint: [f32; 4],
}

/// A queued draw: sort key (mesh id) + the GPU-visible payload.
struct DrawCmd {
    mesh: u32,
    data: DrawData,
}

/// Uploaded mesh buffers; `index_count` always equals the source
/// `MeshData::indices` length and drives `draw_indexed`.
struct GpuMesh {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
}

struct DepthTarget {
    /// Kept alive for the view; never read directly.
    _texture: Texture,
    view: TextureView,
    size: (u32, u32),
}

/// Statistics from the last [`Renderer3D::end`] call.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStats {
    /// Instanced draw calls issued (one per mesh run).
    pub draw_calls: u32,
    /// Total mesh instances drawn.
    pub instances: u32,
    /// Debug line segments drawn.
    pub lines: u32,
}

/// An unlit 3D renderer.
///
/// # Usage
///
/// ```ignore
/// let cube = renderer.create_mesh(&gpu, &primitives::cube(1.0));
/// // per frame:
/// renderer.begin(&camera);
/// renderer.draw_mesh(cube, world_matrix, tint);
/// renderer.draw_grid(10.0, 1.0, grid_color);
/// renderer.end(&gpu, &mut encoder, frame.view(), surface_size);
/// ```
///
/// Draws issued outside `begin`/`end` accumulate into the next frame.
/// The renderer owns its depth buffer (reverse-Z, cleared each pass);
/// the color target is loaded, not cleared — clear it in a prior pass.
pub struct Renderer3D {
    pipeline: Pipeline3D,
    camera_buffer: Buffer,
    camera_bind_group: BindGroup,
    view_proj: Mat4,
    meshes: Vec<GpuMesh>,
    draws: Vec<DrawCmd>,
    draw_buffer: Buffer,
    draw_buffer_capacity: usize,
    draw_bind_group: BindGroup,
    lines: Vec<LineVertex>,
    line_buffer: Buffer,
    line_buffer_capacity: usize,
    depth: Option<DepthTarget>,
    stats: RenderStats,
}

impl Renderer3D {
    /// Creates a new 3D renderer targeting `surface_format`.
    pub fn new(gpu: &Gpu, surface_format: TextureFormat) -> Self {
        astrelis_profiling::profile_function!();
        let device = gpu.device();

        let pipeline = Pipeline3D::new(device, surface_format);

        let camera_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("render3d_camera"),
            size: 64, // mat4x4<f32>
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = pipeline.create_camera_bind_group(device, &camera_buffer);

        let draw_capacity = 256;
        let draw_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("render3d_draws"),
            size: (draw_capacity * std::mem::size_of::<DrawData>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let draw_bind_group = pipeline.create_draw_bind_group(device, &draw_buffer);

        let line_capacity = 1024;
        let line_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("render3d_lines"),
            size: (line_capacity * std::mem::size_of::<LineVertex>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            view_proj: Mat4::IDENTITY,
            meshes: Vec::new(),
            draws: Vec::new(),
            draw_buffer,
            draw_buffer_capacity: draw_capacity,
            draw_bind_group,
            lines: Vec::new(),
            line_buffer,
            line_buffer_capacity: line_capacity,
            depth: None,
            stats: RenderStats::default(),
        }
    }

    /// Uploads mesh data to the GPU and returns a handle.
    ///
    /// The handle is valid for this renderer's lifetime; there is no
    /// way to free an individual mesh in v1.
    pub fn create_mesh(&mut self, gpu: &Gpu, data: &MeshData) -> MeshHandle {
        let device = gpu.device();
        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("render3d_mesh_vertices"),
            contents: bytemuck::cast_slice(&data.vertices),
            usage: BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("render3d_mesh_indices"),
            contents: bytemuck::cast_slice(&data.indices),
            usage: BufferUsages::INDEX,
        });
        let idx = self.meshes.len() as u32;
        self.meshes.push(GpuMesh {
            vertex_buffer,
            index_buffer,
            index_count: data.indices.len() as u32,
        });
        MeshHandle(idx)
    }

    /// Begins a new frame: captures the camera and clears draw lists.
    pub fn begin(&mut self, camera: &Camera3D) {
        self.view_proj = camera.view_projection();
        self.draws.clear();
        self.lines.clear();
    }

    /// Queues a mesh draw with the given world transform and tint.
    ///
    /// # Panics
    ///
    /// Panics if `mesh` was not created by this renderer.
    pub fn draw_mesh(&mut self, mesh: MeshHandle, world: Mat4, tint: Color) {
        assert!(
            (mesh.0 as usize) < self.meshes.len(),
            "invalid MeshHandle({}): only {} meshes registered with this renderer",
            mesh.0,
            self.meshes.len()
        );
        self.draws.push(DrawCmd {
            mesh: mesh.0,
            data: DrawData {
                world: world.to_cols_array_2d(),
                tint: tint.into(),
            },
        });
    }

    /// Queues a world-space debug line segment.
    pub fn draw_line(&mut self, a: Vec3, b: Vec3, color: Color) {
        let color: [f32; 4] = color.into();
        self.lines.push(LineVertex { position: a.to_array(), color });
        self.lines.push(LineVertex { position: b.to_array(), color });
    }

    /// Queues an XZ grid at y=0: lines every `spacing` units out to
    /// `±half_extent`.
    pub fn draw_grid(&mut self, half_extent: f32, spacing: f32, color: Color) {
        for (a, b) in grid_segments(half_extent, spacing) {
            self.draw_line(a, b, color);
        }
    }

    /// Queues RGB = XYZ axis lines of `length`, drawn in `transform`'s
    /// frame.
    pub fn draw_axes(&mut self, transform: Mat4, length: f32) {
        for (a, b, color) in axes_segments(transform, length) {
            self.draw_line(a, b, color);
        }
    }

    /// Flushes all queued draws into one depth-tested render pass.
    ///
    /// `target_size` is the physical pixel size of `target`; the
    /// renderer lazily (re)creates its depth texture to match.
    /// The color attachment is loaded, not cleared.
    pub fn end(
        &mut self,
        gpu: &Gpu,
        encoder: &mut astrelis_gpu::CommandEncoder<'_>,
        target: &TextureView,
        target_size: Size<Physical>,
    ) {
        astrelis_profiling::profile_function!();

        // Round, don't truncate: a fractional physical size (e.g.
        // 1199.9 on a HiDPI surface) must not round down and produce
        // a depth texture one pixel smaller than the color target —
        // mismatched attachment sizes are a wgpu validation error.
        let width = target_size.width.round() as u32;
        let height = target_size.height.round() as u32;
        if width == 0 || height == 0 || (self.draws.is_empty() && self.lines.is_empty()) {
            self.draws.clear();
            self.lines.clear();
            self.stats = RenderStats::default();
            return;
        }

        let device = gpu.device();

        // Lazily (re)create the depth target to match the color target.
        if self.depth.as_ref().map(|d| d.size) != Some((width, height)) {
            let texture = device.create_texture(&TextureDescriptor {
                label: Some("render3d_depth"),
                size: Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT,
            });
            let view = device.create_texture_view(&texture, &TextureViewDescriptor::default());
            self.depth = Some(DepthTarget { _texture: texture, view, size: (width, height) });
        }

        // Camera uniform.
        device.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&self.view_proj.to_cols_array()),
        );

        // Sort by mesh so identical meshes form instanced runs.
        // (Draw order within a depth-tested opaque pass is free.)
        self.draws.sort_unstable_by_key(|d| d.mesh);
        let runs = instance_runs(&self.draws, |d| d.mesh);

        // Upload per-draw data, growing the storage buffer if needed.
        // Growth recreates the bind group too — it references the buffer.
        let draw_data: Vec<DrawData> = self.draws.iter().map(|d| d.data).collect();
        if draw_data.len() > self.draw_buffer_capacity {
            let new_capacity = draw_data.len().next_power_of_two();
            self.draw_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("render3d_draws"),
                size: (new_capacity * std::mem::size_of::<DrawData>()) as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.draw_bind_group =
                self.pipeline.create_draw_bind_group(device, &self.draw_buffer);
            self.draw_buffer_capacity = new_capacity;
        }
        if !draw_data.is_empty() {
            device.write_buffer(&self.draw_buffer, 0, bytemuck::cast_slice(&draw_data));
        }

        // Upload line vertices, growing if needed.
        if self.lines.len() > self.line_buffer_capacity {
            let new_capacity = self.lines.len().next_power_of_two();
            self.line_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("render3d_lines"),
                size: (new_capacity * std::mem::size_of::<LineVertex>()) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.line_buffer_capacity = new_capacity;
        }
        if !self.lines.is_empty() {
            device.write_buffer(&self.line_buffer, 0, bytemuck::cast_slice(&self.lines));
        }

        let mut stats = RenderStats {
            draw_calls: 0,
            instances: self.draws.len() as u32,
            lines: (self.lines.len() / 2) as u32,
        };

        {
            let depth = self.depth.as_ref().expect("depth target created above");
            let mut pass =
                encoder.begin_render_pass(&astrelis_gpu::command::RenderPassDescriptor {
                    label: Some("render3d"),
                    color_attachments: &[astrelis_gpu::command::ColorAttachment {
                        view: target,
                        resolve_target: None,
                        load_op: LoadOp::Load,
                        store_op: StoreOp::Store,
                    }],
                    depth_stencil_attachment: Some(
                        astrelis_gpu::command::DepthStencilAttachment {
                            view: &depth.view,
                            depth_load_op: LoadOp::Clear(0.0), // reverse-Z far
                            // The depth buffer is transient (cleared
                            // every pass, never sampled): Discard lets
                            // tile-based GPUs skip the write-back.
                            depth_store_op: StoreOp::Discard,
                            depth_read_only: false,
                        },
                    ),
                });

            if !runs.is_empty() {
                pass.set_pipeline(&self.pipeline.mesh_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_bind_group(1, &self.draw_bind_group, &[]);
                for (mesh, range) in runs {
                    let m = &self.meshes[mesh as usize];
                    pass.set_vertex_buffer(0, &m.vertex_buffer, 0, None);
                    pass.set_index_buffer(&m.index_buffer, IndexFormat::Uint32, 0, None);
                    pass.draw_indexed(0..m.index_count, 0, range);
                    stats.draw_calls += 1;
                }
            }

            if !self.lines.is_empty() {
                pass.set_pipeline(&self.pipeline.line_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, &self.line_buffer, 0, None);
                pass.draw(0..self.lines.len() as u32, 0..1);
                stats.draw_calls += 1;
            }
        }

        self.draws.clear();
        self.lines.clear();
        astrelis_profiling::profile_counter!("render3d", "draw_calls", stats.draw_calls);
        astrelis_profiling::profile_counter!("render3d", "instances", stats.instances);
        self.stats = stats;
    }

    /// Returns statistics from the last [`end`](Self::end) call.
    pub fn stats(&self) -> RenderStats {
        self.stats
    }
}

/// Groups a slice (pre-sorted by `key`) into (key, instance range)
/// runs — each run becomes one instanced draw call.
///
/// Generic over the element so `end()` can pass `&[DrawCmd]` directly
/// without collecting a scratch `Vec<u32>` of ids every frame.
fn instance_runs<T>(sorted: &[T], key: impl Fn(&T) -> u32) -> Vec<(u32, std::ops::Range<u32>)> {
    let mut runs = Vec::new();
    let mut start = 0usize;
    for i in 1..=sorted.len() {
        if i == sorted.len() || key(&sorted[i]) != key(&sorted[start]) {
            runs.push((key(&sorted[start]), start as u32..i as u32));
            start = i;
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_runs_groups_consecutive_meshes() {
        // Already sorted (the caller sorts): two 0s, one 1, three 2s.
        let runs = instance_runs(&[0u32, 0, 1, 2, 2, 2], |&id| id);
        assert_eq!(runs, vec![(0, 0..2), (1, 2..3), (2, 3..6)]);
    }

    #[test]
    fn instance_runs_empty_and_single() {
        assert!(instance_runs(&[] as &[u32], |&id| id).is_empty());
        assert_eq!(instance_runs(&[7u32], |&id| id), vec![(7, 0..1)]);
    }
}
