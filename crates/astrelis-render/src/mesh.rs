//! Mesh abstraction for high-level geometry management.
//!
//! Provides a declarative API for creating and rendering meshes with vertices, indices,
//! and common primitive shapes.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_render::*;
//! use glam::Vec3;
//!
//! // Create a mesh from vertices
//! let mesh = MeshBuilder::new()
//!     .with_positions(vec![
//!         Vec3::new(-0.5, -0.5, 0.0),
//!         Vec3::new(0.5, -0.5, 0.0),
//!         Vec3::new(0.0, 0.5, 0.0),
//!     ])
//!     .with_indices(vec![0, 1, 2])
//!     .build(&ctx);
//!
//! // Draw the mesh
//! mesh.draw(&mut pass);
//!
//! // Or create a primitive
//! let cube = Mesh::cube(&ctx, 1.0);
//! cube.draw_instanced(&mut pass, 10);
//! ```

use crate::GraphicsContext;
use glam::{Vec2, Vec3};
use std::sync::Arc;

/// Vertex format specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertexFormat {
    /// Position only (Vec3)
    Position,
    /// Position + Normal (Vec3 + Vec3)
    PositionNormal,
    /// Position + UV (Vec3 + Vec2)
    PositionUv,
    /// Position + Normal + UV (Vec3 + Vec3 + Vec2)
    PositionNormalUv,
    /// Position + Normal + UV + Color (Vec3 + Vec3 + Vec2 + Vec4)
    PositionNormalUvColor,
}

impl VertexFormat {
    /// Get the size of a single vertex in bytes.
    pub fn vertex_size(&self) -> u64 {
        match self {
            VertexFormat::Position => 12,                    // 3 floats
            VertexFormat::PositionNormal => 24,              // 6 floats
            VertexFormat::PositionUv => 20,                  // 5 floats
            VertexFormat::PositionNormalUv => 32,            // 8 floats
            VertexFormat::PositionNormalUvColor => 48,       // 12 floats
        }
    }

    /// Get the WGPU vertex buffer layout for this format.
    pub fn buffer_layout(&self) -> wgpu::VertexBufferLayout<'static> {
        match self {
            VertexFormat::Position => wgpu::VertexBufferLayout {
                array_stride: 12,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }],
            },
            VertexFormat::PositionNormal => wgpu::VertexBufferLayout {
                array_stride: 24,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 12,
                        shader_location: 1,
                    },
                ],
            },
            VertexFormat::PositionUv => wgpu::VertexBufferLayout {
                array_stride: 20,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 12,
                        shader_location: 1,
                    },
                ],
            },
            VertexFormat::PositionNormalUv => wgpu::VertexBufferLayout {
                array_stride: 32,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 12,
                        shader_location: 1,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 24,
                        shader_location: 2,
                    },
                ],
            },
            VertexFormat::PositionNormalUvColor => wgpu::VertexBufferLayout {
                array_stride: 48,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 12,
                        shader_location: 1,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 24,
                        shader_location: 2,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 32,
                        shader_location: 3,
                    },
                ],
            },
        }
    }
}

/// A mesh containing vertex and optional index data.
pub struct Mesh {
    /// Vertex buffer
    vertex_buffer: wgpu::Buffer,
    /// Optional index buffer
    index_buffer: Option<wgpu::Buffer>,
    /// Vertex format
    vertex_format: VertexFormat,
    /// Primitive topology
    topology: wgpu::PrimitiveTopology,
    /// Index format (if indexed)
    index_format: Option<wgpu::IndexFormat>,
    /// Number of vertices
    vertex_count: u32,
    /// Number of indices (if indexed)
    index_count: Option<u32>,
    /// Graphics context reference
    _context: Arc<GraphicsContext>,
}

impl Mesh {
    /// Draw the mesh.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        if let Some(ref index_buffer) = self.index_buffer {
            let index_format = self.index_format.expect("Index format must be set");
            pass.set_index_buffer(index_buffer.slice(..), index_format);
            pass.draw_indexed(0..self.index_count.unwrap(), 0, 0..1);
        } else {
            pass.draw(0..self.vertex_count, 0..1);
        }
    }

    /// Draw the mesh instanced.
    pub fn draw_instanced<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, instances: u32) {
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        if let Some(ref index_buffer) = self.index_buffer {
            let index_format = self.index_format.expect("Index format must be set");
            pass.set_index_buffer(index_buffer.slice(..), index_format);
            pass.draw_indexed(0..self.index_count.unwrap(), 0, 0..instances);
        } else {
            pass.draw(0..self.vertex_count, 0..instances);
        }
    }

    /// Get the vertex format.
    pub fn vertex_format(&self) -> VertexFormat {
        self.vertex_format
    }

    /// Get the primitive topology.
    pub fn topology(&self) -> wgpu::PrimitiveTopology {
        self.topology
    }

    /// Get the vertex count.
    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    /// Get the index count (if indexed).
    pub fn index_count(&self) -> Option<u32> {
        self.index_count
    }

    // ===== Primitive Generators =====

    /// Create a unit cube mesh (1x1x1) centered at origin.
    pub fn cube(ctx: Arc<GraphicsContext>, size: f32) -> Self {
        let half = size / 2.0;

        let positions = vec![
            // Front face
            Vec3::new(-half, -half, half),
            Vec3::new(half, -half, half),
            Vec3::new(half, half, half),
            Vec3::new(-half, half, half),
            // Back face
            Vec3::new(-half, -half, -half),
            Vec3::new(-half, half, -half),
            Vec3::new(half, half, -half),
            Vec3::new(half, -half, -half),
            // Top face
            Vec3::new(-half, half, -half),
            Vec3::new(-half, half, half),
            Vec3::new(half, half, half),
            Vec3::new(half, half, -half),
            // Bottom face
            Vec3::new(-half, -half, -half),
            Vec3::new(half, -half, -half),
            Vec3::new(half, -half, half),
            Vec3::new(-half, -half, half),
            // Right face
            Vec3::new(half, -half, -half),
            Vec3::new(half, half, -half),
            Vec3::new(half, half, half),
            Vec3::new(half, -half, half),
            // Left face
            Vec3::new(-half, -half, -half),
            Vec3::new(-half, -half, half),
            Vec3::new(-half, half, half),
            Vec3::new(-half, half, -half),
        ];

        let normals = vec![
            // Front
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            // Back
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, -1.0),
            // Top
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            // Bottom
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            // Right
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            // Left
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
        ];

        let uvs = vec![
            // Front
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 0.0),
            // Back
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 1.0),
            // Top
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
            // Bottom
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
            // Right
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 1.0),
            // Left
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 0.0),
        ];

        #[rustfmt::skip]
        let indices: Vec<u32> = vec![
            0, 1, 2, 2, 3, 0,       // Front
            4, 5, 6, 6, 7, 4,       // Back
            8, 9, 10, 10, 11, 8,    // Top
            12, 13, 14, 14, 15, 12, // Bottom
            16, 17, 18, 18, 19, 16, // Right
            20, 21, 22, 22, 23, 20, // Left
        ];

        MeshBuilder::new()
            .with_positions(positions)
            .with_normals(normals)
            .with_uvs(uvs)
            .with_indices(indices)
            .build(ctx)
    }

    /// Create a plane mesh (XZ plane) centered at origin.
    pub fn plane(ctx: Arc<GraphicsContext>, width: f32, depth: f32) -> Self {
        let hw = width / 2.0;
        let hd = depth / 2.0;

        let positions = vec![
            Vec3::new(-hw, 0.0, -hd),
            Vec3::new(hw, 0.0, -hd),
            Vec3::new(hw, 0.0, hd),
            Vec3::new(-hw, 0.0, hd),
        ];

        let normals = vec![
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];

        let uvs = vec![
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 0.0),
        ];

        let indices = vec![0, 1, 2, 2, 3, 0];

        MeshBuilder::new()
            .with_positions(positions)
            .with_normals(normals)
            .with_uvs(uvs)
            .with_indices(indices)
            .build(ctx)
    }

    /// Create a sphere mesh using UV sphere generation.
    pub fn sphere(ctx: Arc<GraphicsContext>, radius: f32, segments: u32, rings: u32) -> Self {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();

        // Generate vertices
        for ring in 0..=rings {
            let theta = ring as f32 * std::f32::consts::PI / rings as f32;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            for segment in 0..=segments {
                let phi = segment as f32 * 2.0 * std::f32::consts::PI / segments as f32;
                let sin_phi = phi.sin();
                let cos_phi = phi.cos();

                let x = sin_theta * cos_phi;
                let y = cos_theta;
                let z = sin_theta * sin_phi;

                positions.push(Vec3::new(x * radius, y * radius, z * radius));
                normals.push(Vec3::new(x, y, z));
                uvs.push(Vec2::new(
                    segment as f32 / segments as f32,
                    ring as f32 / rings as f32,
                ));
            }
        }

        // Generate indices
        for ring in 0..rings {
            for segment in 0..segments {
                let first = ring * (segments + 1) + segment;
                let second = first + segments + 1;

                indices.push(first);
                indices.push(second);
                indices.push(first + 1);

                indices.push(second);
                indices.push(second + 1);
                indices.push(first + 1);
            }
        }

        MeshBuilder::new()
            .with_positions(positions)
            .with_normals(normals)
            .with_uvs(uvs)
            .with_indices(indices)
            .build(ctx)
    }
}

/// Builder for creating meshes.
pub struct MeshBuilder {
    positions: Vec<Vec3>,
    normals: Option<Vec<Vec3>>,
    uvs: Option<Vec<Vec2>>,
    colors: Option<Vec<[f32; 4]>>,
    indices: Option<Vec<u32>>,
    topology: wgpu::PrimitiveTopology,
}

impl MeshBuilder {
    /// Create a new mesh builder.
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: None,
            uvs: None,
            colors: None,
            indices: None,
            topology: wgpu::PrimitiveTopology::TriangleList,
        }
    }

    /// Set positions.
    pub fn with_positions(mut self, positions: Vec<Vec3>) -> Self {
        self.positions = positions;
        self
    }

    /// Set normals.
    pub fn with_normals(mut self, normals: Vec<Vec3>) -> Self {
        self.normals = Some(normals);
        self
    }

    /// Set UVs.
    pub fn with_uvs(mut self, uvs: Vec<Vec2>) -> Self {
        self.uvs = Some(uvs);
        self
    }

    /// Set vertex colors.
    pub fn with_colors(mut self, colors: Vec<[f32; 4]>) -> Self {
        self.colors = Some(colors);
        self
    }

    /// Set indices.
    pub fn with_indices(mut self, indices: Vec<u32>) -> Self {
        self.indices = Some(indices);
        self
    }

    /// Set primitive topology (default: TriangleList).
    pub fn with_topology(mut self, topology: wgpu::PrimitiveTopology) -> Self {
        self.topology = topology;
        self
    }

    /// Generate flat normals (per-triangle normals).
    pub fn generate_flat_normals(mut self) -> Self {
        if self.indices.is_none() {
            panic!("Cannot generate flat normals without indices");
        }

        let indices = self.indices.as_ref().unwrap();
        let mut normals = vec![Vec3::ZERO; self.positions.len()];

        for triangle in indices.chunks(3) {
            let i0 = triangle[0] as usize;
            let i1 = triangle[1] as usize;
            let i2 = triangle[2] as usize;

            let v0 = self.positions[i0];
            let v1 = self.positions[i1];
            let v2 = self.positions[i2];

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let normal = edge1.cross(edge2).normalize();

            normals[i0] = normal;
            normals[i1] = normal;
            normals[i2] = normal;
        }

        self.normals = Some(normals);
        self
    }

    /// Generate smooth normals (averaged per-vertex normals).
    pub fn generate_smooth_normals(mut self) -> Self {
        if self.indices.is_none() {
            panic!("Cannot generate smooth normals without indices");
        }

        let indices = self.indices.as_ref().unwrap();
        let mut normals = vec![Vec3::ZERO; self.positions.len()];
        let mut counts = vec![0u32; self.positions.len()];

        for triangle in indices.chunks(3) {
            let i0 = triangle[0] as usize;
            let i1 = triangle[1] as usize;
            let i2 = triangle[2] as usize;

            let v0 = self.positions[i0];
            let v1 = self.positions[i1];
            let v2 = self.positions[i2];

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let normal = edge1.cross(edge2);

            normals[i0] += normal;
            normals[i1] += normal;
            normals[i2] += normal;

            counts[i0] += 1;
            counts[i1] += 1;
            counts[i2] += 1;
        }

        // Average and normalize
        for (i, normal) in normals.iter_mut().enumerate() {
            if counts[i] > 0 {
                *normal = (*normal / counts[i] as f32).normalize();
            }
        }

        self.normals = Some(normals);
        self
    }

    /// Build the mesh.
    pub fn build(self, ctx: Arc<GraphicsContext>) -> Mesh {
        // Determine vertex format
        let vertex_format = match (&self.normals, &self.uvs, &self.colors) {
            (None, None, None) => VertexFormat::Position,
            (Some(_), None, None) => VertexFormat::PositionNormal,
            (None, Some(_), None) => VertexFormat::PositionUv,
            (Some(_), Some(_), None) => VertexFormat::PositionNormalUv,
            (Some(_), Some(_), Some(_)) => VertexFormat::PositionNormalUvColor,
            _ => panic!("Invalid vertex format combination"),
        };

        // Build vertex data
        let mut vertex_data = Vec::new();
        for i in 0..self.positions.len() {
            // Position
            vertex_data.extend_from_slice(bytemuck::bytes_of(&self.positions[i]));

            // Normal
            if let Some(ref normals) = self.normals {
                vertex_data.extend_from_slice(bytemuck::bytes_of(&normals[i]));
            }

            // UV
            if let Some(ref uvs) = self.uvs {
                vertex_data.extend_from_slice(bytemuck::bytes_of(&uvs[i]));
            }

            // Color
            if let Some(ref colors) = self.colors {
                vertex_data.extend_from_slice(bytemuck::bytes_of(&colors[i]));
            }
        }

        // Create vertex buffer
        let vertex_buffer = ctx.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mesh Vertex Buffer"),
            size: vertex_data.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue().write_buffer(&vertex_buffer, 0, &vertex_data);

        // Create index buffer if present
        let (index_buffer, index_format, index_count) = if let Some(ref indices) = self.indices {
            let buffer = ctx.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("Mesh Index Buffer"),
                size: (indices.len() * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            ctx.queue()
                .write_buffer(&buffer, 0, bytemuck::cast_slice(indices));
            (
                Some(buffer),
                Some(wgpu::IndexFormat::Uint32),
                Some(indices.len() as u32),
            )
        } else {
            (None, None, None)
        };

        Mesh {
            vertex_buffer,
            index_buffer,
            vertex_format,
            topology: self.topology,
            index_format,
            vertex_count: self.positions.len() as u32,
            index_count,
            _context: ctx,
        }
    }
}

impl Default for MeshBuilder {
    fn default() -> Self {
        Self::new()
    }
}
