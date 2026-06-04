# astrelis-render-3d Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unlit/debug 3D rendering: procedural vertex-colored meshes + debug lines, perspective reverse-Z camera, renderer-owned depth buffer, scene-tree-driven demo.

**Architecture:** New layer-2 crate `astrelis-render-3d` mirroring `astrelis-render-2d`'s shape: `Renderer3D::new(gpu, format)` + per-frame `begin/draw_mesh/end`. Retained meshes (`create_mesh` → `MeshHandle`), immediate draw list sorted by mesh in `end()` so runs become instanced draw calls. Two pipelines (mesh: triangle list, depth write, `GreaterEqual`; line: line list, depth test only). Spec: `docs/superpowers/specs/2026-06-04-astrelis-render-3d-design.md`.

**Tech Stack:** Rust 2024, wgpu via `astrelis-gpu` wrappers (never raw wgpu except where render-2d already does), glam via `astrelis_core::math`, bytemuck, WGSL.

**Conventions reminders:**
- Edition 2024, `#![warn(missing_docs)]` — every public item gets a doc comment.
- A PostToolUse hook runs rustfmt on every edit; do not hand-format.
- All deps go through `[workspace.dependencies]`.
- Conventional commits.
- Build/test commands: `cargo build --workspace`, `cargo test -p astrelis-render-3d`, `cargo test --workspace`.

**Key API facts (verified against the codebase, do not re-derive):**
- `GpuDevice::create_buffer(&BufferDescriptor { label, size: u64, usage: BufferUsages, mapped_at_creation: bool }) -> Buffer`; `create_buffer_init(&BufferInitDescriptor { label, contents: &[u8], usage })`; `write_buffer(&Buffer, offset: u64, data: &[u8])`.
- `GpuDevice::create_texture(&TextureDescriptor { label, size: Extent3d, mip_level_count, sample_count, dimension: TextureDimension, format: TextureFormat, usage: TextureUsages })`; `create_texture_view(&Texture, &TextureViewDescriptor::default())`.
- Render pass: `astrelis_gpu::command::{RenderPassDescriptor { label, color_attachments: &[ColorAttachment], depth_stencil_attachment: Option<DepthStencilAttachment> }, ColorAttachment { view, resolve_target, load_op: LoadOp<Color>, store_op }, DepthStencilAttachment { view, depth_load_op: LoadOp<f32>, depth_store_op: StoreOp, depth_read_only: bool }}`.
- `RenderPass::{set_pipeline, set_bind_group(u32, &BindGroup, &[u32]), set_vertex_buffer(slot, &Buffer, offset, size: Option<u64>), set_index_buffer(&Buffer, IndexFormat, offset, size), draw(Range<u32>, Range<u32>), draw_indexed(Range<u32>, i32, Range<u32>)}`.
- `BindingType::StorageBuffer { has_dynamic_offset: bool, min_binding_size: u64, read_only: bool }` and `BindingType::UniformBuffer { has_dynamic_offset, min_binding_size }`.
- `PrimitiveState { topology, strip_index_format, front_face, cull_mode: CullMode, polygon_mode, unclipped_depth }` impls `Default` (TriangleList, Ccw, CullMode::None).
- `DepthStencilState { format, depth_write_enabled, depth_compare }` (no stencil fields).
- `Surface::config() -> Option<&SurfaceConfiguration>` (width/height: u32); `SurfaceFrame::view()` does NOT expose size.
- `astrelis_core::math` re-exports `glam::{Mat4, Quat, Vec2, Vec3, Vec3A, Vec4}` — **Mat3 is missing**; Task 2 adds it.
- `Color { r, g, b, a }` impls `From<Color> for [f32; 4]`.
- Follow `crates/astrelis-render-2d/src/{pipeline.rs,renderer.rs}` for descriptor idioms — they are the canonical style.

---

### Task 1: Crate scaffold

**Files:**
- Create: `crates/astrelis-render-3d/Cargo.toml`
- Create: `crates/astrelis-render-3d/src/lib.rs`
- Modify: root `Cargo.toml` (workspace members + `[workspace.dependencies]`)

- [ ] **Step 1: Create `crates/astrelis-render-3d/Cargo.toml`**

```toml
[package]
name = "astrelis-render-3d"
description = "Unlit/debug 3D rendering pipeline for the Astrelis engine"
version.workspace = true
authors.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
astrelis-core = { workspace = true }
astrelis-gpu = { workspace = true }
astrelis-profiling = { workspace = true }
bytemuck = { workspace = true }
tracing = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 2: Create `crates/astrelis-render-3d/src/lib.rs`**

```rust
//! Unlit/debug 3D rendering for the Astrelis engine.
//!
//! World convention: right-handed, +Y up, −Z forward (glTF-aligned).
//! Depth: reverse-Z with an infinite far plane (`Depth32Float`,
//! compare `GreaterEqual`, cleared to 0.0) for near-uniform float
//! precision over the whole range.
//!
//! Frame flow mirrors `astrelis-render-2d`: upload meshes once with
//! [`Renderer3D::create_mesh`], then per frame call
//! [`Renderer3D::begin`], any number of `draw_*` calls, and
//! [`Renderer3D::end`]. The renderer owns its depth texture; clearing
//! the color target stays the caller's job (compose passes by
//! ordering: clear → 3D → 2D HUD on top).
//!
//! This crate has no scene or app dependencies; scene→renderer glue
//! lives downstream.

#![warn(missing_docs)]

pub mod camera;
pub mod mesh;
pub mod primitives;
```

(`pub use camera::Camera3D;` etc. are added by Tasks 3–4 together with the types; `pipeline`, `renderer`, and `debug` modules are added by later tasks.)

Create module placeholders so the crate compiles:

`crates/astrelis-render-3d/src/camera.rs`:
```rust
//! 3D perspective camera.
```
`crates/astrelis-render-3d/src/mesh.rs`:
```rust
//! Mesh data types.
```
`crates/astrelis-render-3d/src/primitives.rs`:
```rust
//! Procedural mesh generators.
```

- [ ] **Step 3: Register in root `Cargo.toml`**

Add `"crates/astrelis-render-3d"` to `[workspace] members` (alphabetical, next to `astrelis-render-2d`), and to `[workspace.dependencies]`:

```toml
astrelis-render-3d = { path = "crates/astrelis-render-3d", version = "0.3.0" }
```

- [ ] **Step 4: Build**

Run: `cargo build -p astrelis-render-3d`
Expected: compiles clean (warnings allowed only if pre-existing elsewhere).

- [ ] **Step 5: Commit**

```bash
git add crates/astrelis-render-3d Cargo.toml
git commit -m "feat(render-3d): scaffold astrelis-render-3d crate"
```

---

### Task 2: Add Mat3 to core math re-exports

**Files:**
- Modify: `crates/astrelis-core/src/math.rs` (the `pub use glam::{...}` line)

- [ ] **Step 1: Add `Mat3` (and `Mat3A`) to the glam re-export**

Change:
```rust
pub use glam::{Mat4, Quat, Vec2, Vec3, Vec3A, Vec4};
```
to:
```rust
pub use glam::{Mat3, Mat3A, Mat4, Quat, Vec2, Vec3, Vec3A, Vec4};
```

(Needed by `Camera3D::look_at` — `Quat::from_mat3(&Mat3::from_mat4(..))` is the only glam path from a look-at matrix to a quaternion.)

- [ ] **Step 2: Build and commit**

Run: `cargo build -p astrelis-core`
Expected: clean.

```bash
git add crates/astrelis-core/src/math.rs
git commit -m "feat(core): re-export glam Mat3/Mat3A through math"
```

---

### Task 3: Camera3D (TDD)

**Files:**
- Modify: `crates/astrelis-render-3d/src/camera.rs`
- Modify: `crates/astrelis-render-3d/src/lib.rs` (re-export)

- [ ] **Step 1: Write the failing tests**

Append to `camera.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Projects a world point to NDC (perspective divide included).
    fn ndc(camera: &Camera3D, p: Vec3) -> Vec3 {
        let clip = camera.view_projection() * p.extend(1.0);
        Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w)
    }

    #[test]
    fn defaults_are_sane() {
        let cam = Camera3D::new(16.0 / 9.0);
        assert_eq!(cam.position, Vec3::ZERO);
        assert_eq!(cam.rotation, Quat::IDENTITY);
        assert!((cam.fov_y - 60f32.to_radians()).abs() < 1e-6);
        assert!((cam.near - 0.1).abs() < 1e-6);
    }

    #[test]
    fn point_straight_ahead_maps_to_ndc_center() {
        // Identity rotation looks down −Z (right-handed, +Y up).
        let cam = Camera3D::new(1.0);
        let n = ndc(&cam, Vec3::new(0.0, 0.0, -10.0));
        assert!(n.x.abs() < 1e-5 && n.y.abs() < 1e-5, "got {n:?}");
        assert!(n.z > 0.0 && n.z < 1.0, "depth must be inside (0,1), got {}", n.z);
    }

    #[test]
    fn reverse_z_near_is_one_far_is_zero() {
        let cam = Camera3D::new(1.0);
        let near = ndc(&cam, Vec3::new(0.0, 0.0, -cam.near));
        let far = ndc(&cam, Vec3::new(0.0, 0.0, -1.0e6));
        assert!((near.z - 1.0).abs() < 1e-4, "near depth ≈ 1, got {}", near.z);
        assert!(far.z < 1e-4, "distant depth → 0, got {}", far.z);
    }

    #[test]
    fn aspect_scales_x_only() {
        let narrow = Camera3D::new(1.0);
        let wide = Camera3D::new(2.0);
        let p = Vec3::new(1.0, 1.0, -10.0);
        let n1 = ndc(&narrow, p);
        let n2 = ndc(&wide, p);
        assert!((n2.x - n1.x / 2.0).abs() < 1e-5, "x halves when aspect doubles");
        assert!((n2.y - n1.y).abs() < 1e-6, "y unchanged by aspect");
    }

    #[test]
    fn look_at_points_forward_axis_at_target() {
        let mut cam = Camera3D::new(1.0);
        cam.position = Vec3::new(5.0, 0.0, 0.0);
        cam.look_at(Vec3::ZERO, Vec3::Y);
        // Camera forward is rotation * −Z; it must point toward −X.
        let forward = cam.rotation * Vec3::NEG_Z;
        assert!((forward - Vec3::NEG_X).length() < 1e-5, "got forward {forward:?}");
        // And the target must project to NDC center.
        let n = ndc(&cam, Vec3::ZERO);
        assert!(n.x.abs() < 1e-5 && n.y.abs() < 1e-5, "got {n:?}");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p astrelis-render-3d camera -- --nocapture 2>&1 | tail -20`
Expected: FAIL to compile — `Camera3D` not defined.

- [ ] **Step 3: Implement Camera3D**

`camera.rs` body (above the tests):

```rust
//! 3D perspective camera.

use astrelis_core::math::{Mat3, Mat4, Quat, Vec3};

/// A perspective 3D camera.
///
/// Convention: right-handed, +Y up; identity rotation looks down −Z.
/// The projection is reverse-Z with an infinite far plane: the near
/// plane maps to depth 1 and infinity to depth 0, which gives
/// near-uniform float precision over the whole range. Pair with a
/// `GreaterEqual` depth compare and a clear value of 0.0.
pub struct Camera3D {
    /// Camera position in world space.
    pub position: Vec3,
    /// Camera orientation. Identity looks down −Z with +Y up.
    pub rotation: Quat,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// Viewport aspect ratio (width / height). Update on resize.
    ///
    /// Deliberately a bare ratio, not a viewport size: the projection
    /// only needs the ratio, and a size would re-import the
    /// physical/logical-pixel ambiguity.
    pub aspect: f32,
    /// Near plane distance. There is no far plane (infinite reverse-Z).
    pub near: f32,
}

impl Camera3D {
    /// Creates a camera at the origin looking down −Z.
    ///
    /// Defaults: 60° vertical FOV, near plane at 0.1.
    pub fn new(aspect: f32) -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            fov_y: 60f32.to_radians(),
            aspect,
            near: 0.1,
        }
    }

    /// Rotates the camera in place to look at `target`.
    pub fn look_at(&mut self, target: Vec3, up: Vec3) {
        let view = Mat4::look_at_rh(self.position, target, up);
        // The view matrix is the inverse of the camera's world
        // transform; its inverse's rotation part is the camera pose.
        self.rotation = Quat::from_mat3(&Mat3::from_mat4(view.inverse())).normalize();
    }

    /// Computes the combined view-projection matrix.
    pub fn view_projection(&self) -> Mat4 {
        let view = Mat4::from_rotation_translation(self.rotation, self.position).inverse();
        let proj = Mat4::perspective_infinite_reverse_rh(self.fov_y, self.aspect, self.near);
        proj * view
    }
}
```

Re-add to `lib.rs`:
```rust
pub use camera::Camera3D;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p astrelis-render-3d camera 2>&1 | tail -5`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/astrelis-render-3d
git commit -m "feat(render-3d): Camera3D with reverse-Z infinite-far projection"
```

---

### Task 4: Mesh types (TDD)

**Files:**
- Modify: `crates/astrelis-render-3d/src/mesh.rs`
- Modify: `crates/astrelis-render-3d/src/lib.rs` (re-export)

- [ ] **Step 1: Write the failing tests**

Append to `mesh.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_is_48_bytes_pod() {
        assert_eq!(std::mem::size_of::<Vertex>(), 48);
        // Pod round-trip: cast a slice without panicking.
        let v = [Vertex {
            position: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.5, 0.5],
            color: [1.0, 1.0, 1.0, 1.0],
        }];
        let bytes: &[u8] = bytemuck::cast_slice(&v);
        assert_eq!(bytes.len(), 48);
    }

    #[test]
    fn vertex_layout_matches_field_offsets() {
        let layout = Vertex::layout();
        assert_eq!(layout.array_stride, 48);
        let offsets: Vec<u64> = layout.attributes.iter().map(|a| a.offset).collect();
        assert_eq!(offsets, vec![0, 12, 24, 32]);
        let locations: Vec<u32> = layout.attributes.iter().map(|a| a.shader_location).collect();
        assert_eq!(locations, vec![0, 1, 2, 3]);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p astrelis-render-3d mesh 2>&1 | tail -5`
Expected: FAIL to compile — `Vertex` not defined.

- [ ] **Step 3: Implement mesh types**

`mesh.rs` body:

```rust
//! Mesh data types: CPU-side vertices and GPU mesh handles.

use astrelis_gpu::pipeline::{VertexAttribute, VertexBufferLayout, VertexStepMode};
use astrelis_gpu::types::VertexFormat;

/// A single mesh vertex (48 bytes).
///
/// Normals and UVs are present even though v1 rendering is unlit and
/// untextured: generators produce them for free, and adding them
/// later would force regenerating every mesh when lighting lands.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    /// Object-space position.
    pub position: [f32; 3],
    /// Unit normal (unused by the unlit shader; reserved for lighting).
    pub normal: [f32; 3],
    /// Texture coordinates (unused in v1; reserved for texturing).
    pub uv: [f32; 2],
    /// Per-vertex RGBA color, multiplied with the per-draw tint.
    pub color: [f32; 4],
}

impl Vertex {
    /// Vertex buffer layout matching the WGSL vertex inputs.
    pub(crate) fn layout() -> VertexBufferLayout<'static> {
        const ATTRS: [VertexAttribute; 4] = [
            VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 },
            VertexAttribute { format: VertexFormat::Float32x3, offset: 12, shader_location: 1 },
            VertexAttribute { format: VertexFormat::Float32x2, offset: 24, shader_location: 2 },
            VertexAttribute { format: VertexFormat::Float32x4, offset: 32, shader_location: 3 },
        ];
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

/// CPU-side mesh data, ready for upload via `Renderer3D::create_mesh`.
///
/// Plain data — callers may edit (e.g. paint vertex colors) before
/// uploading.
pub struct MeshData {
    /// Vertex list.
    pub vertices: Vec<Vertex>,
    /// Triangle list indices into `vertices` (CCW = front face).
    pub indices: Vec<u32>,
}

/// Handle to a mesh uploaded with `Renderer3D::create_mesh`.
///
/// Plain index, valid for the lifetime of the renderer that created
/// it (no `destroy_mesh` in v1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeshHandle(pub(crate) u32);
```

Note: if `VertexStepMode` lives in `astrelis_gpu::types` rather than `pipeline`, fix the import — check how `crates/astrelis-render-2d/src/instance.rs` imports it and copy that.

Re-add to `lib.rs`:
```rust
pub use mesh::{MeshData, MeshHandle, Vertex};
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p astrelis-render-3d mesh 2>&1 | tail -5`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/astrelis-render-3d
git commit -m "feat(render-3d): Vertex/MeshData/MeshHandle with fixed 48-byte format"
```

---

### Task 5: Primitive generators (TDD)

**Files:**
- Modify: `crates/astrelis-render-3d/src/primitives.rs`

- [ ] **Step 1: Write the failing tests**

Append to `primitives.rs`:

```rust
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
            if cross.length() < 1e-9 {
                continue; // degenerate pole triangle — harmless
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p astrelis-render-3d primitives 2>&1 | tail -5`
Expected: FAIL to compile — generators not defined.

- [ ] **Step 3: Implement the generators**

`primitives.rs` body:

```rust
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
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p astrelis-render-3d primitives 2>&1 | tail -5`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/astrelis-render-3d
git commit -m "feat(render-3d): cube/uv_sphere/plane generators with verified winding"
```

---

### Task 6: Debug line segment helpers (TDD)

**Files:**
- Create: `crates/astrelis-render-3d/src/debug.rs`
- Modify: `crates/astrelis-render-3d/src/lib.rs` (add `mod debug;` — private module)

- [ ] **Step 1: Write the failing tests**

`debug.rs`:

```rust
//! Pure geometry for debug primitives (grid, axes).
//!
//! Kept as data-producing functions so they are unit-testable without
//! a GPU; `Renderer3D` feeds the segments into its line buffer.

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_core::math::{Mat4, Vec3};

    #[test]
    fn grid_segment_count_and_extents() {
        // half_extent 2, spacing 1 → lines at -2,-1,0,1,2 in both
        // directions: 5 + 5 = 10 segments.
        let segs = grid_segments(2.0, 1.0);
        assert_eq!(segs.len(), 10);
        for (a, b) in &segs {
            assert_eq!(a.y, 0.0);
            assert_eq!(b.y, 0.0);
            assert!((*b - *a).length() > 3.9, "segments span the grid");
        }
    }

    #[test]
    fn axes_follow_the_transform() {
        let t = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let [(x0, x1, _), (y0, y1, _), (z0, z1, _)] = axes_segments(t, 2.0);
        let origin = Vec3::new(1.0, 2.0, 3.0);
        assert!((x0 - origin).length() < 1e-6);
        assert!((x1 - (origin + Vec3::X * 2.0)).length() < 1e-6);
        assert!((y0 - origin).length() < 1e-6);
        assert!((y1 - (origin + Vec3::Y * 2.0)).length() < 1e-6);
        assert!((z1 - (origin + Vec3::Z * 2.0)).length() < 1e-6);
        let _ = z0;
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p astrelis-render-3d debug 2>&1 | tail -5`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

Body of `debug.rs` (above the tests):

```rust
use astrelis_core::color::Color;
use astrelis_core::math::{Mat4, Vec3};

/// Grid lines on the XZ plane at y=0, every `spacing` units out to
/// `±half_extent` on both axes.
pub(crate) fn grid_segments(half_extent: f32, spacing: f32) -> Vec<(Vec3, Vec3)> {
    let mut segments = Vec::new();
    let steps = (half_extent / spacing).floor() as i32;
    for i in -steps..=steps {
        let d = i as f32 * spacing;
        segments.push((Vec3::new(d, 0.0, -half_extent), Vec3::new(d, 0.0, half_extent)));
        segments.push((Vec3::new(-half_extent, 0.0, d), Vec3::new(half_extent, 0.0, d)));
    }
    segments
}

/// X/Y/Z axis segments (RGB) of `length`, drawn in `transform`'s frame.
pub(crate) fn axes_segments(transform: Mat4, length: f32) -> [(Vec3, Vec3, Color); 3] {
    let o = transform.transform_point3(Vec3::ZERO);
    [
        (o, transform.transform_point3(Vec3::X * length), Color::new(0.9, 0.2, 0.2, 1.0)),
        (o, transform.transform_point3(Vec3::Y * length), Color::new(0.2, 0.9, 0.2, 1.0)),
        (o, transform.transform_point3(Vec3::Z * length), Color::new(0.25, 0.45, 1.0, 1.0)),
    ]
}
```

Add `mod debug;` to `lib.rs` (not `pub` — internal).

Note: the test for `grid_segments(2.0, 1.0)` expects exactly 10; the
`-steps..=steps` form gives `2*steps+1 = 5` iterations × 2 segments = 10. ✓

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p astrelis-render-3d debug 2>&1 | tail -5`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/astrelis-render-3d
git commit -m "feat(render-3d): pure grid/axes segment generators"
```

---

### Task 7: WGSL shader + Pipeline3D

**Files:**
- Create: `crates/astrelis-render-3d/src/shader.wgsl`
- Create: `crates/astrelis-render-3d/src/pipeline.rs`
- Modify: `crates/astrelis-render-3d/src/lib.rs` (add `mod pipeline;`)

No unit tests possible without a GPU device — this task is compile-verified here and runtime-verified by the demo (Task 10). Keep it small and mirror `render-2d/src/pipeline.rs` exactly in style.

- [ ] **Step 1: Write `shader.wgsl`**

```wgsl
// Unlit 3D shaders. Reverse-Z; camera view_proj maps world → clip.

struct Camera {
    view_proj: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> camera: Camera;

// Per-draw data, indexed by instance_index (the draw list is sorted
// by mesh so each run of identical meshes is one instanced draw).
struct DrawData {
    world: mat4x4<f32>,
    tint: vec4<f32>,
}
@group(1) @binding(0) var<storage, read> draws: array<DrawData>;

struct MeshIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @builtin(instance_index) instance: u32,
}

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_mesh(in: MeshIn) -> VsOut {
    let draw = draws[in.instance];
    var out: VsOut;
    out.clip = camera.view_proj * draw.world * vec4<f32>(in.position, 1.0);
    out.color = in.color * draw.tint;
    return out;
}

struct LineIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_line(in: LineIn) -> VsOut {
    var out: VsOut;
    out.clip = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
```

- [ ] **Step 2: Write `pipeline.rs`**

```rust
//! Render pipeline setup for the 3D renderer.

use astrelis_gpu::bind_group::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingType, ShaderStages,
};
use astrelis_gpu::device::GpuDevice;
use astrelis_gpu::pipeline::{
    ColorTargetState, DepthStencilState, FragmentState, MultisampleState,
    PipelineLayoutDescriptor, PrimitiveState, RenderPipelineDescriptor, VertexState,
};
use astrelis_gpu::resources::{BindGroup, BindGroupLayout, RenderPipeline};
use astrelis_gpu::shader::{ShaderModuleDescriptor, ShaderSource};
use astrelis_gpu::types::{
    BlendState, ColorWrites, CompareFunction, CullMode, PrimitiveTopology, TextureFormat,
};

use crate::mesh::Vertex;
use crate::renderer::LineVertex;

/// Depth format used by the 3D pass (reverse-Z, cleared to 0.0).
pub(crate) const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// GPU resources for the 3D render pipelines.
pub(crate) struct Pipeline3D {
    pub mesh_pipeline: RenderPipeline,
    pub line_pipeline: RenderPipeline,
    pub camera_layout: BindGroupLayout,
    pub draw_layout: BindGroupLayout,
}

impl Pipeline3D {
    /// Creates the mesh and line pipelines.
    pub fn new(device: &GpuDevice, surface_format: TextureFormat) -> Self {
        astrelis_profiling::profile_function!();

        let shader = device
            .create_shader_module(&ShaderModuleDescriptor {
                label: Some("render3d_shader"),
                source: ShaderSource::Wgsl(include_str!("shader.wgsl")),
            })
            .expect("failed to compile render3d shader");

        // Group 0: camera uniform (shared by both pipelines).
        let camera_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("render3d_camera_layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::UniformBuffer {
                    has_dynamic_offset: false,
                    min_binding_size: 0,
                },
                count: None,
            }],
        });

        // Group 1: per-draw storage buffer (mesh pipeline only).
        let draw_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("render3d_draw_layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::StorageBuffer {
                    has_dynamic_offset: false,
                    min_binding_size: 0,
                    read_only: true,
                },
                count: None,
            }],
        });

        let mesh_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("render3d_mesh_pipeline_layout"),
            bind_group_layouts: &[&camera_layout, &draw_layout],
            push_constant_ranges: &[],
        });

        let line_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("render3d_line_pipeline_layout"),
            bind_group_layouts: &[&camera_layout],
            push_constant_ranges: &[],
        });

        // Opaque mesh pass: depth write + reverse-Z GreaterEqual,
        // back-face culling (generators guarantee CCW-from-outside).
        let mesh_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("render3d_mesh_pipeline"),
            layout: Some(&mesh_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_mesh",
                buffers: &[Vertex::layout()],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[ColorTargetState {
                    format: surface_format,
                    blend: None, // opaque
                    write_mask: ColorWrites::ALL,
                }],
            }),
            primitive: PrimitiveState {
                cull_mode: CullMode::Back,
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: CompareFunction::GreaterEqual,
            }),
            multisample: MultisampleState::default(),
        });

        // Debug lines: depth-tested but never depth-written, so they
        // occlude behind geometry without punching holes in it.
        let line_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("render3d_line_pipeline"),
            layout: Some(&line_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_line",
                buffers: &[LineVertex::layout()],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                }],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: CompareFunction::GreaterEqual,
            }),
            multisample: MultisampleState::default(),
        });

        Self {
            mesh_pipeline,
            line_pipeline,
            camera_layout,
            draw_layout,
        }
    }

    /// Creates a bind group for the camera uniform buffer.
    pub fn create_camera_bind_group(
        &self,
        device: &GpuDevice,
        buffer: &astrelis_gpu::Buffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("render3d_camera_bg"),
            layout: &self.camera_layout,
            entries: &[BindGroupEntry::Buffer { binding: 0, buffer, offset: 0, size: None }],
        })
    }

    /// Creates a bind group for the per-draw storage buffer.
    pub fn create_draw_bind_group(
        &self,
        device: &GpuDevice,
        buffer: &astrelis_gpu::Buffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("render3d_draw_bg"),
            layout: &self.draw_layout,
            entries: &[BindGroupEntry::Buffer { binding: 0, buffer, offset: 0, size: None }],
        })
    }
}
```

This imports `crate::renderer::LineVertex`, which doesn't exist yet — Task 8 creates `renderer.rs` in the same commit window. To keep each task buildable, **do Tasks 7 and 8 as one commit** OR add `renderer.rs` with just `LineVertex` in this task. Choose the latter:

Create `crates/astrelis-render-3d/src/renderer.rs` with only:

```rust
//! The 3D renderer: draw list, depth target, debug lines.

use astrelis_gpu::pipeline::{VertexAttribute, VertexBufferLayout, VertexStepMode};
use astrelis_gpu::types::VertexFormat;

/// A debug-line vertex (28 bytes): world-space position + color.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl LineVertex {
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
```

(Same `VertexStepMode` import caveat as Task 4 — copy whatever `instance.rs` in render-2d does.)

Add to `lib.rs`:
```rust
mod pipeline;
mod renderer;
```

- [ ] **Step 3: Build**

Run: `cargo build -p astrelis-render-3d 2>&1 | tail -10`
Expected: clean. (`Pipeline3D` is unused so far — if `dead_code` fires under workspace lints, add `pub(crate)` use or `#[allow(dead_code)]` temporarily and remove it in Task 8; prefer building Tasks 7+8 back-to-back and committing both before lints block.)

- [ ] **Step 4: Commit**

```bash
git add crates/astrelis-render-3d
git commit -m "feat(render-3d): unlit WGSL shaders and mesh/line pipelines"
```

---

### Task 8: Renderer3D

**Files:**
- Modify: `crates/astrelis-render-3d/src/renderer.rs`
- Modify: `crates/astrelis-render-3d/src/lib.rs` (make `renderer` public via re-exports)

- [ ] **Step 1: Write the failing test for run batching**

Append to `renderer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_runs_groups_consecutive_meshes() {
        // Already sorted (the caller sorts): two 0s, one 1, three 2s.
        let runs = instance_runs(&[0, 0, 1, 2, 2, 2]);
        assert_eq!(runs, vec![(0, 0..2), (1, 2..3), (2, 3..6)]);
    }

    #[test]
    fn instance_runs_empty_and_single() {
        assert!(instance_runs(&[]).is_empty());
        assert_eq!(instance_runs(&[7]), vec![(7, 0..1)]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p astrelis-render-3d renderer 2>&1 | tail -5`
Expected: FAIL to compile — `instance_runs` not defined.

- [ ] **Step 3: Implement `instance_runs`, run test**

Add to `renderer.rs`:

```rust
/// Groups a sorted slice of mesh ids into (mesh, instance range)
/// runs — each run becomes one instanced draw call.
fn instance_runs(sorted: &[u32]) -> Vec<(u32, std::ops::Range<u32>)> {
    let mut runs = Vec::new();
    let mut start = 0usize;
    for i in 1..=sorted.len() {
        if i == sorted.len() || sorted[i] != sorted[start] {
            runs.push((sorted[start], start as u32..i as u32));
            start = i;
        }
    }
    runs
}
```

Run: `cargo test -p astrelis-render-3d renderer 2>&1 | tail -5`
Expected: 2 passed.

- [ ] **Step 4: Implement the full Renderer3D**

Replace/extend `renderer.rs` (keep `LineVertex` and `instance_runs` from before; full file below):

```rust
//! The 3D renderer: draw list, depth target, debug lines.

use astrelis_core::color::Color;
use astrelis_core::geometry::{Physical, Size};
use astrelis_core::math::{Mat4, Vec3};
use astrelis_gpu::buffer::{BufferDescriptor, BufferInitDescriptor, BufferUsages};
use astrelis_gpu::pipeline::{VertexAttribute, VertexBufferLayout, VertexStepMode};
use astrelis_gpu::resources::{BindGroup, TextureView};
use astrelis_gpu::texture::{
    Extent3d, TextureDescriptor, TextureDimension, TextureUsages, TextureViewDescriptor,
};
use astrelis_gpu::types::{IndexFormat, LoadOp, StoreOp, TextureFormat, VertexFormat};
use astrelis_gpu::{Buffer, CommandEncoder, Gpu, Texture};

use crate::camera::Camera3D;
use crate::debug::{axes_segments, grid_segments};
use crate::mesh::{MeshData, MeshHandle};
use crate::pipeline::{Pipeline3D, DEPTH_FORMAT};

// NOTE for implementer: the exact import paths above (Buffer,
// CommandEncoder, Gpu, descriptor modules) must be checked against
// what crates/astrelis-render-2d/src/renderer.rs imports — copy its
// import style verbatim, the types are the same ones.

/// A debug-line vertex (28 bytes): world-space position + color.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl LineVertex {
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
    world: [[f32; 4]; 4],
    tint: [f32; 4],
}

struct DrawCmd {
    mesh: u32,
    data: DrawData,
}

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
/// renderer.draw_mesh(cube, world_matrix, Color::WHITE_LIKE_TINT);
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
        encoder: &mut CommandEncoder<'_>,
        target: &TextureView,
        target_size: Size<Physical>,
    ) {
        astrelis_profiling::profile_function!();

        let width = target_size.width as u32;
        let height = target_size.height as u32;
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
        let mesh_ids: Vec<u32> = self.draws.iter().map(|d| d.mesh).collect();
        let runs = instance_runs(&mesh_ids);

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
            self.draw_bind_group = self.pipeline.create_draw_bind_group(device, &self.draw_buffer);
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
            let mut pass = encoder.begin_render_pass(&astrelis_gpu::command::RenderPassDescriptor {
                label: Some("render3d"),
                color_attachments: &[astrelis_gpu::command::ColorAttachment {
                    view: target,
                    resolve_target: None,
                    load_op: LoadOp::Load,
                    store_op: StoreOp::Store,
                }],
                depth_stencil_attachment: Some(astrelis_gpu::command::DepthStencilAttachment {
                    view: &depth.view,
                    depth_load_op: LoadOp::Clear(0.0), // reverse-Z far
                    depth_store_op: StoreOp::Store,
                    depth_read_only: false,
                }),
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
```

Then in `lib.rs`, expose the renderer:

```rust
pub mod renderer;

pub use renderer::{RenderStats, Renderer3D};
```

(`renderer` was `mod renderer;` from Task 7 — change to `pub mod` and keep `LineVertex` `pub(crate)`.)

**Implementer notes:**
- Exact import paths for `Buffer`, `CommandEncoder`, `Gpu`, `TextureView`, descriptor types: copy from `crates/astrelis-render-2d/src/renderer.rs` — same types, canonical paths.
- `@builtin(instance_index)` includes the `first_instance` of the draw call in WebGPU/wgpu — that's what makes per-run instance ranges index the storage buffer correctly.
- `draw_indexed(0..index_count, 0, range)` with non-zero `first_instance` requires no special wgpu feature for indexing via `instance_index` in the shader.
- `profile_counter!` value type: check the macro definition in `astrelis-profiling/src/lib.rs` (~line 223) and cast `stats.draw_calls`/`stats.instances` accordingly.

- [ ] **Step 5: Run all crate tests + workspace build**

Run: `cargo test -p astrelis-render-3d 2>&1 | tail -5 && cargo build --workspace 2>&1 | tail -5`
Expected: all tests pass (camera 5, mesh 2, primitives 6, debug 2, renderer 2), workspace builds.

- [ ] **Step 6: Commit**

```bash
git add crates/astrelis-render-3d
git commit -m "feat(render-3d): Renderer3D with sorted instanced draw list and owned reverse-Z depth"
```

---

### Task 9: Facade re-exports

**Files:**
- Modify: `crates/astrelis/Cargo.toml` (add dep)
- Modify: `crates/astrelis/src/lib.rs` (module + prelude re-exports)

- [ ] **Step 1: Add the dependency**

In `crates/astrelis/Cargo.toml` `[dependencies]`, after `astrelis-render-2d`:

```toml
astrelis-render-3d = { workspace = true }
```

- [ ] **Step 2: Add re-exports in `crates/astrelis/src/lib.rs`**

After the `render_2d` re-export:

```rust
/// 3D rendering: unlit meshes, debug lines, perspective camera.
pub use astrelis_render_3d as render_3d;
```

And in the `prelude` module, after the 2D rendering line:

```rust
// 3D rendering.
pub use astrelis_render_3d::{Camera3D, MeshData, MeshHandle, Renderer3D, Vertex};
```

(No name collisions: checked against `astrelis_render_2d` exports and `math::*` glob.)

- [ ] **Step 3: Build and commit**

Run: `cargo build -p astrelis 2>&1 | tail -5`
Expected: clean.

```bash
git add crates/astrelis
git commit -m "feat: re-export astrelis-render-3d through the facade crate"
```

---

### Task 10: `render_3d_demo` example

**Files:**
- Create: `crates/astrelis/examples/render_3d_demo.rs`

Mirror `crates/astrelis/examples/scene_demo.rs` exactly in structure (read it first). Plugin registration order matters: `Render3DSetupPlugin` before `ScenePopulatePlugin`, because populate's startup reads the `DemoMeshes` resource that setup's startup inserts (startups run in plugin-add order).

- [ ] **Step 1: Write the example**

```rust
//! Unlit 3D demo: a spinning rainbow cube with two orbiting spheres,
//! a ground grid and origin axes, driven by the scene tree. The glue
//! component (`MeshInstance`) is defined HERE, not in the engine.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis --example render_3d_demo
//! ```

use astrelis::prelude::*;

use astrelis::core::geometry::{Physical, Size};
use astrelis::gpu::{Gpu, GpuError, Surface};
use astrelis::render_3d::primitives;

/// User-defined drawable component — engine knows nothing about it.
struct MeshInstance {
    mesh: MeshHandle,
    tint: Color,
}

/// Marks nodes that spin around their +Y axis.
struct Spin {
    speed: f32,
}

/// Handles to the demo's uploaded meshes.
struct DemoMeshes {
    cube: MeshHandle,
    sphere: MeshHandle,
}

/// Creates the renderer, camera, and uploads the demo meshes.
struct Render3DSetupPlugin;

impl Plugin for Render3DSetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup(|resources, _ctx| {
            let gpu = resources.get::<Gpu>();
            let surface = resources.get::<Surface>();
            let format = surface.preferred_format();
            let mut renderer = Renderer3D::new(&gpu, format);
            let camera = Camera3D::new(1280.0 / 720.0);

            // Rainbow cube: paint vertex colors from positions.
            let mut cube_data = primitives::cube(2.0);
            for v in &mut cube_data.vertices {
                let p = Vec3::from(v.position).normalize() * 0.5 + Vec3::splat(0.5);
                v.color = [p.x, p.y, p.z, 1.0];
            }
            let cube = renderer.create_mesh(&gpu, &cube_data);
            let sphere = renderer.create_mesh(&gpu, &primitives::uv_sphere(0.5, 24, 12));

            drop(gpu);
            drop(surface);

            resources.insert(renderer);
            resources.insert(camera);
            resources.insert(DemoMeshes { cube, sphere });
        });
    }
}

/// Builds the scene: spinning cube hub with two orbiting spheres.
struct ScenePopulatePlugin;

impl Plugin for ScenePopulatePlugin {
    fn build(&self, app: &mut App) {
        app.add_startup(|resources, _ctx| {
            let meshes = resources.get::<DemoMeshes>();
            let cube = meshes.cube;
            let sphere = meshes.sphere;
            drop(meshes);

            let mut scene = resources.get_mut::<Scene>();

            // Hub cube at the origin; its spin drags the spheres around.
            let hub = scene
                .spawn()
                .name("hub")
                .position(Vec3::new(0.0, 1.0, 0.0))
                .with(MeshInstance { mesh: cube, tint: Color::new(1.0, 1.0, 1.0, 1.0) })
                .with(Spin { speed: 0.6 })
                .id();

            for i in 0..2 {
                let angle = std::f32::consts::PI * i as f32;
                scene
                    .spawn_child(hub)
                    .name(format!("orbiter{i}"))
                    .position(Vec3::new(angle.cos() * 3.0, 0.0, angle.sin() * 3.0))
                    .with(MeshInstance {
                        mesh: sphere,
                        tint: if i == 0 {
                            Color::new(0.3, 0.7, 1.0, 1.0)
                        } else {
                            Color::new(1.0, 0.6, 0.2, 1.0)
                        },
                    })
                    .id();
            }
        });

        app.add_system(Phase::Update, update_scene);
        app.add_system(Phase::Render, render_scene);
    }
}

fn update_scene(resources: &Resources) {
    let time = resources.get::<Time>();
    let t = time.elapsed_secs() as f32;

    {
        let mut scene = resources.get_mut::<Scene>();
        // Collect-first: mutating while iterating would alias the borrow.
        let spinners: Vec<(NodeId, f32)> =
            scene.iter::<Spin>().map(|(id, s)| (id, s.speed)).collect();
        for (id, speed) in spinners {
            scene.set_rotation(id, Quat::from_rotation_y(t * speed));
        }
    }

    // Slow orbit camera around the scene.
    let mut camera = resources.get_mut::<Camera3D>();
    let a = t * 0.25;
    camera.position = Vec3::new(a.sin() * 9.0, 4.0, a.cos() * 9.0);
    camera.look_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
}

fn render_scene(resources: &Resources) {
    let gpu = resources.get::<Gpu>();
    let mut surface = resources.get_mut::<Surface>();
    let mut renderer = resources.get_mut::<Renderer3D>();
    let mut camera = resources.get_mut::<Camera3D>();
    let scene = resources.get::<Scene>();

    let frame = match surface.acquire() {
        Ok(f) => f,
        Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost | GpuError::Timeout) => return,
        Err(e) => panic!("failed to acquire surface: {e}"),
    };

    // Physical size of the swapchain — drives depth-buffer sizing and
    // the camera aspect.
    let config = surface.config().expect("surface is configured");
    let size = Size::<Physical>::new(config.width as f32, config.height as f32);
    camera.aspect = size.width / size.height;

    let mut encoder = gpu.device().create_command_encoder(Some("frame"));

    // Clear pass (color only; the 3D pass owns and clears its depth).
    {
        let _pass = encoder.begin_render_pass(&astrelis::gpu::command::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[astrelis::gpu::command::ColorAttachment {
                view: frame.view(),
                resolve_target: None,
                load_op: astrelis::gpu::types::LoadOp::Clear(Color::new(0.05, 0.06, 0.09, 1.0)),
                store_op: astrelis::gpu::types::StoreOp::Store,
            }],
            depth_stencil_attachment: None,
        });
    }

    renderer.begin(&camera);

    // The glue: world transforms from the scene, draws to the renderer.
    for (id, inst) in scene.iter::<MeshInstance>() {
        if scene.is_world_visible(id) != Some(true) {
            continue;
        }
        let world = scene.world_transform(id).expect("mesh node is live");
        renderer.draw_mesh(inst.mesh, world, inst.tint);
    }

    renderer.draw_grid(10.0, 1.0, Color::new(0.25, 0.27, 0.32, 1.0));
    renderer.draw_axes(Mat4::IDENTITY, 1.5);

    renderer.end(&gpu, &mut encoder, frame.view(), size);

    gpu.submit(std::iter::once(encoder));
    frame.present();
}

fn main() {
    astrelis::core::logging::init_default();
    App::new()
        .add_default_plugins()
        .add_plugin(ScenePlugin)
        .add_plugin(Render3DSetupPlugin)
        .add_plugin(ScenePopulatePlugin)
        .run();
}
```

**Implementer notes:**
- Verify `scene.set_rotation(id, Quat)` exists (it's used in `astrelis-scene` docs); if its name differs, use the `local_transform`/`set_transform` round-trip like `scene_demo.rs:89-92`.
- Verify `world_transform(id)` returns `Option<Mat4>` (it does in `scene_demo.rs`).
- Verify `SurfaceConfiguration` field names (`width`/`height`) against `astrelis-gpu/src/surface.rs`.

- [ ] **Step 2: Build the example**

Run: `cargo build -p astrelis --example render_3d_demo 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 3: Full workspace verification**

Run: `cargo test --workspace 2>&1 | tail -10 && cargo clippy --workspace 2>&1 | tail -10`
Expected: all tests pass; no new clippy warnings (pre-existing astrelis-window/astrelis-text warnings are known).

- [ ] **Step 4: Commit**

```bash
git add crates/astrelis/examples/render_3d_demo.rs
git commit -m "feat(examples): add render_3d_demo with scene-driven meshes, grid, and axes"
```

- [ ] **Step 5: Manual visual check (user)**

Run: `cargo run -p astrelis --example render_3d_demo`
Expected: dark background; 10×10 grid; RGB axes at origin; rainbow cube spinning at (0,1,0); blue and orange spheres orbiting it (dragged by the hub's rotation); camera slowly circling. Spheres pass *behind* the cube correctly (depth works); grid lines hidden behind meshes (line depth test works).

This step requires a human looking at the window — flag it for the user at the end of execution rather than blocking.
