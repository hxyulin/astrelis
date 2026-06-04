//! Procedural mesh generators.
//!
//! All meshes are centered at the origin with CCW winding viewed
//! from outside and white vertex colors (tint or repaint as needed).

use astrelis_core::math::Vec3;

use crate::mesh::{MeshData, Vertex};

/// Axis-aligned cube with `size` as the full edge length.
///
/// Per-face normals/UVs: 24 vertices, 36 indices.
pub fn cube(size: f32) -> MeshData {
    let h = size / 2.0;
    // (face normal, u axis, v axis), chosen so u × v = normal —
    // that makes the (-1,-1)→(1,-1)→(1,1)→(-1,1) corner order CCW
    // from outside.
    const FACES: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ];
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (normal, u_axis, v_axis) in FACES {
        let n = Vec3::from(normal);
        let u = Vec3::from(u_axis);
        let v = Vec3::from(v_axis);
        let base = vertices.len() as u32;
        for (cu, cv, uv) in [
            (-1.0, -1.0, [0.0, 1.0]),
            (1.0, -1.0, [1.0, 1.0]),
            (1.0, 1.0, [1.0, 0.0]),
            (-1.0, 1.0, [0.0, 0.0]),
        ] {
            vertices.push(Vertex {
                position: ((n + u * cu + v * cv) * h).to_array(),
                normal,
                uv,
                color: [1.0; 4],
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    MeshData { vertices, indices }
}

/// UV sphere: `sectors` longitudinal segments, `stacks` latitudinal.
///
/// Smooth normals (= normalized position), equirectangular UVs.
/// Pole rows produce degenerate triangles; they rasterize to nothing
/// and keep the index count exactly `sectors * stacks * 6`.
pub fn uv_sphere(radius: f32, sectors: u32, stacks: u32) -> MeshData {
    let mut vertices = Vec::with_capacity(((sectors + 1) * (stacks + 1)) as usize);
    for stack in 0..=stacks {
        let v = stack as f32 / stacks as f32;
        let phi = v * std::f32::consts::PI; // 0 at the +Y pole
        for sector in 0..=sectors {
            let u = sector as f32 / sectors as f32;
            let theta = u * std::f32::consts::TAU;
            let n = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
            vertices.push(Vertex {
                position: (n * radius).to_array(),
                normal: n.to_array(),
                uv: [u, v],
                color: [1.0; 4],
            });
        }
    }
    let mut indices = Vec::with_capacity((sectors * stacks * 6) as usize);
    for stack in 0..stacks {
        for sector in 0..sectors {
            let i0 = stack * (sectors + 1) + sector;
            let i1 = i0 + sectors + 1;
            indices.extend_from_slice(&[i0, i0 + 1, i1, i0 + 1, i1 + 1, i1]);
        }
    }
    MeshData { vertices, indices }
}

/// XZ plane facing +Y, centered at the origin.
pub fn plane(width: f32, depth: f32) -> MeshData {
    let hw = width / 2.0;
    let hd = depth / 2.0;
    let n = [0.0, 1.0, 0.0];
    let vertices = vec![
        Vertex { position: [-hw, 0.0, -hd], normal: n, uv: [0.0, 0.0], color: [1.0; 4] },
        Vertex { position: [-hw, 0.0, hd], normal: n, uv: [0.0, 1.0], color: [1.0; 4] },
        Vertex { position: [hw, 0.0, hd], normal: n, uv: [1.0, 1.0], color: [1.0; 4] },
        Vertex { position: [hw, 0.0, -hd], normal: n, uv: [1.0, 0.0], color: [1.0; 4] },
    ];
    let indices = vec![0, 1, 2, 0, 2, 3];
    MeshData { vertices, indices }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_core::math::Vec3;

    fn tri_cross(data: &MeshData, tri: &[u32]) -> (Vec3, Vec3) {
        let a = Vec3::from(data.vertices[tri[0] as usize].position);
        let b = Vec3::from(data.vertices[tri[1] as usize].position);
        let c = Vec3::from(data.vertices[tri[2] as usize].position);
        ((b - a).cross(c - a), (a + b + c) / 3.0)
    }

    #[test]
    fn cube_counts_and_extents() {
        let data = cube(2.0);
        assert_eq!(data.vertices.len(), 24);
        assert_eq!(data.indices.len(), 36);
        for v in &data.vertices {
            for c in v.position {
                assert!(c.abs() <= 1.0 + 1e-6, "half edge = size/2");
            }
        }
        assert!(data.indices.iter().all(|&i| (i as usize) < 24));
    }

    #[test]
    fn cube_normals_unit_outward() {
        let data = cube(2.0);
        for v in &data.vertices {
            let n = Vec3::from(v.normal);
            let p = Vec3::from(v.position);
            assert!((n.length() - 1.0).abs() < 1e-6);
            assert!(n.dot(p) > 0.0, "normal points away from center");
        }
    }

    #[test]
    fn cube_winding_ccw_from_outside() {
        let data = cube(2.0);
        for tri in data.indices.chunks(3) {
            let (cross, centroid) = tri_cross(&data, tri);
            assert!(cross.dot(centroid) > 0.0, "CCW viewed from outside");
        }
    }

    #[test]
    fn sphere_counts_radius_normals() {
        let data = uv_sphere(2.0, 16, 8);
        assert_eq!(data.vertices.len(), (16 + 1) * (8 + 1));
        assert_eq!(data.indices.len(), 16 * 8 * 6);
        for v in &data.vertices {
            let p = Vec3::from(v.position);
            let n = Vec3::from(v.normal);
            assert!((p.length() - 2.0).abs() < 1e-5, "on the sphere surface");
            assert!((n - p / 2.0).length() < 1e-5, "normal = normalized position");
        }
    }

    #[test]
    fn sphere_winding_ccw_from_outside() {
        let data = uv_sphere(1.0, 16, 8);
        for tri in data.indices.chunks(3) {
            let (cross, centroid) = tri_cross(&data, tri);
            if cross.length() < 1e-6 {
                continue; // degenerate or near-pole triangle — harmless
            }
            assert!(cross.dot(centroid) > 0.0, "CCW viewed from outside");
        }
    }

    #[test]
    fn plane_is_flat_up_facing_ccw() {
        let data = plane(4.0, 2.0);
        assert_eq!(data.vertices.len(), 4);
        assert_eq!(data.indices.len(), 6);
        for v in &data.vertices {
            assert_eq!(v.position[1], 0.0);
            assert_eq!(v.normal, [0.0, 1.0, 0.0]);
            assert!(v.position[0].abs() <= 2.0 && v.position[2].abs() <= 1.0);
        }
        for tri in data.indices.chunks(3) {
            let (cross, _) = tri_cross(&data, tri);
            assert!(cross.dot(Vec3::Y) > 0.0, "CCW viewed from +Y");
        }
    }
}
