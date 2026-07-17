//! CPU mesh data and procedural primitives.

use astrelis_core::math::{Vec2, Vec3, Vec4};
use bytemuck::{Pod, Zeroable};

/// Fixed lit-mesh vertex format.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct MeshVertex {
    /// Object-space position.
    pub position: [f32; 3],
    /// Object-space unit normal.
    pub normal: [f32; 3],
    /// Texture coordinates.
    pub uv: [f32; 2],
    /// Straight-alpha vertex color.
    pub color: [f32; 4],
}

/// Validated CPU-side indexed triangle mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshData {
    /// Mesh vertices.
    pub vertices: Vec<MeshVertex>,
    /// Triangle-list indices.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Validates finite attributes, triangle indices, and unit-ish normals.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.vertices.is_empty()
            || self.indices.is_empty()
            || !self.indices.len().is_multiple_of(3)
        {
            return Err("mesh must contain vertices and complete indexed triangles");
        }
        if self
            .indices
            .iter()
            .any(|&index| index as usize >= self.vertices.len())
        {
            return Err("mesh index is out of range");
        }
        for vertex in &self.vertices {
            let position = Vec3::from(vertex.position);
            let normal = Vec3::from(vertex.normal);
            if !position.is_finite()
                || !normal.is_finite()
                || !Vec2::from(vertex.uv).is_finite()
                || !Vec4::from(vertex.color).is_finite()
                || normal.length_squared() < 0.25
            {
                return Err("mesh attributes must be finite and normals nonzero");
            }
        }
        Ok(())
    }

    pub(crate) fn bounding_sphere(&self) -> (Vec3, f32) {
        let center = self
            .vertices
            .iter()
            .fold(Vec3::ZERO, |sum, vertex| sum + Vec3::from(vertex.position))
            / self.vertices.len() as f32;
        let radius = self
            .vertices
            .iter()
            .map(|vertex| Vec3::from(vertex.position).distance(center))
            .fold(0.0, f32::max);
        (center, radius)
    }
}

/// Creates a +Y-facing XZ plane centered at the origin.
pub fn plane(width: f32, depth: f32) -> MeshData {
    let positions = [
        [-width * 0.5, 0.0, -depth * 0.5],
        [-width * 0.5, 0.0, depth * 0.5],
        [width * 0.5, 0.0, depth * 0.5],
        [width * 0.5, 0.0, -depth * 0.5],
    ];
    MeshData {
        vertices: positions
            .into_iter()
            .enumerate()
            .map(|(index, position)| MeshVertex {
                position,
                normal: [0.0, 1.0, 0.0],
                uv: [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]][index],
                color: [1.0; 4],
            })
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

/// Creates a cube with independent face normals and UVs.
pub fn cube(size: f32) -> MeshData {
    let h = size * 0.5;
    let faces = [
        (
            Vec3::X,
            [
                Vec3::new(h, -h, h),
                Vec3::new(h, h, h),
                Vec3::new(h, h, -h),
                Vec3::new(h, -h, -h),
            ],
        ),
        (
            Vec3::NEG_X,
            [
                Vec3::new(-h, -h, -h),
                Vec3::new(-h, h, -h),
                Vec3::new(-h, h, h),
                Vec3::new(-h, -h, h),
            ],
        ),
        (
            Vec3::Y,
            [
                Vec3::new(-h, h, h),
                Vec3::new(-h, h, -h),
                Vec3::new(h, h, -h),
                Vec3::new(h, h, h),
            ],
        ),
        (
            Vec3::NEG_Y,
            [
                Vec3::new(-h, -h, -h),
                Vec3::new(-h, -h, h),
                Vec3::new(h, -h, h),
                Vec3::new(h, -h, -h),
            ],
        ),
        (
            Vec3::Z,
            [
                Vec3::new(-h, -h, h),
                Vec3::new(-h, h, h),
                Vec3::new(h, h, h),
                Vec3::new(h, -h, h),
            ],
        ),
        (
            Vec3::NEG_Z,
            [
                Vec3::new(h, -h, -h),
                Vec3::new(h, h, -h),
                Vec3::new(-h, h, -h),
                Vec3::new(-h, -h, -h),
            ],
        ),
    ];
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (face, (normal, positions)) in faces.into_iter().enumerate() {
        let base = (face * 4) as u32;
        for (index, position) in positions.into_iter().enumerate() {
            vertices.push(MeshVertex {
                position: position.to_array(),
                normal: normal.to_array(),
                uv: [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]][index],
                color: [1.0; 4],
            });
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    MeshData { vertices, indices }
}

/// Creates a latitude-longitude sphere with smooth normals.
pub fn uv_sphere(radius: f32, sectors: u32, stacks: u32) -> MeshData {
    let sectors = sectors.max(3);
    let stacks = stacks.max(2);
    let mut vertices = Vec::new();
    for stack in 0..=stacks {
        let v = stack as f32 / stacks as f32;
        let phi = v * std::f32::consts::PI;
        for sector in 0..=sectors {
            let u = sector as f32 / sectors as f32;
            let theta = u * std::f32::consts::TAU;
            let normal = Vec3::new(theta.cos() * phi.sin(), phi.cos(), theta.sin() * phi.sin());
            vertices.push(MeshVertex {
                position: (normal * radius).to_array(),
                normal: normal.to_array(),
                uv: [u, v],
                color: [1.0; 4],
            });
        }
    }
    let mut indices = Vec::new();
    let row = sectors + 1;
    for stack in 0..stacks {
        for sector in 0..sectors {
            let a = stack * row + sector;
            let b = a + row;
            indices.extend_from_slice(&[a, b + 1, b, a, a + 1, b + 1]);
        }
    }
    MeshData { vertices, indices }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_meshes_are_valid() {
        for mesh in [plane(2.0, 3.0), cube(2.0), uv_sphere(1.0, 12, 6)] {
            mesh.validate().unwrap();
        }
    }

    #[test]
    fn procedural_triangle_winding_matches_vertex_normals() {
        for mesh in [plane(2.0, 3.0), cube(2.0), uv_sphere(1.0, 12, 6)] {
            for triangle in mesh.indices.chunks_exact(3) {
                let a = &mesh.vertices[triangle[0] as usize];
                let b = &mesh.vertices[triangle[1] as usize];
                let c = &mesh.vertices[triangle[2] as usize];
                let geometric = (Vec3::from(b.position) - Vec3::from(a.position))
                    .cross(Vec3::from(c.position) - Vec3::from(a.position));
                if geometric.length_squared() < 1.0e-8 {
                    continue;
                }
                let shading = Vec3::from(a.normal) + Vec3::from(b.normal) + Vec3::from(c.normal);
                assert!(
                    geometric.dot(shading) > 0.0,
                    "triangle {triangle:?} has winding opposite its normals"
                );
            }
        }
    }
}
