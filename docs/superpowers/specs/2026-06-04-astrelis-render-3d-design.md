# astrelis-render-3d — Unlit 3D Rendering (Design)

Date: 2026-06-04
Status: Approved
Roadmap position: sub-project 2 of 4 (after `astrelis-scene`, before
`astrelis-ui`), per `2026-06-04-astrelis-scene-design.md`.

## Goal

Unlit/debug 3D rendering: enough to render procedurally generated,
vertex-colored meshes plus debug lines through the scene tree, with a
perspective camera and a depth buffer. First real 3D consumer of the
scene's world transforms. Lighting, model loading, and textures are
explicitly out of scope (see Deferred).

v1 success criterion: a `render_3d_demo` example showing a spinning
vertex-rainbow cube at the origin, two spheres orbiting as scene-tree
children, a ground grid at y=0, and axis lines — driven by scene
world transforms, with correct depth occlusion.

## Decisions (with rationale)

| Decision | Choice | Why |
|---|---|---|
| v1 target | Procedural primitives + debug grid/axes | Validates transforms, camera, depth with zero asset-pipeline work; model loading is a follow-up |
| World convention | Right-handed, +Y up, −Z forward | glTF convention — future model loading needs no axis fix-ups; matches most references. 2D stays y-down screen space |
| Depth strategy | Reverse-Z, infinite far, `Depth32Float` | Near-uniform float precision over the whole range; one glam call (`perspective_infinite_reverse_rh`) + `GreaterEqual` compare + clear-to-0.0. Retrofitting later breaks every custom shader, so day one is the cheap moment |
| Depth ownership | `Renderer3D` owns its depth texture | Self-contained like `Renderer2D`; lazily recreated when the target size changes. Shared frame-targets helper deferred until a second pass needs it |
| Draw recording | Retained meshes + immediate draw list | `create_mesh` uploads once; per-frame `begin/draw_mesh/end` mirrors `Renderer2D`. Sort-by-mesh in `end()` makes instancing an internal detail, not an API change |
| Scene glue | Lives in example code for v1 | See the glue pattern twice (2D, 3D) before freezing a `astrelis-scene-render` API; extraction decided during UI work |
| Camera aspect | `aspect: f32`, not a viewport size | Projection only needs the ratio; storing a size would re-import the Physical/Logical ambiguity that bit `Camera2D` |

## Crate

`astrelis-render-3d`, layer 2.

Dependencies: `astrelis-core` (math, color, geometry), `astrelis-gpu`,
`astrelis-profiling`. No scene, no app, no winit — same dependency
profile as `astrelis-render-2d`. Re-exported through the `astrelis`
facade as `render_3d` plus prelude entries.

Modules:

- `camera` — `Camera3D`
- `mesh` — `Vertex`, `MeshData`, `MeshHandle`
- `primitives` — `cube`, `uv_sphere`, `plane` generators
- `renderer` — `Renderer3D`, draw list, depth texture, debug lines
- `pipeline` — mesh + line pipeline construction
- `shader.wgsl` — unlit mesh and line shaders

## Camera3D

```rust
pub struct Camera3D {
    pub position: Vec3,
    pub rotation: Quat,   // identity = looking down −Z, +Y up
    pub fov_y: f32,       // vertical FOV, radians
    pub aspect: f32,      // width / height — set on resize
    pub near: f32,        // default 0.1
}

impl Camera3D {
    pub fn new(aspect: f32) -> Self;                  // sane defaults
    pub fn look_at(&mut self, target: Vec3, up: Vec3); // sets rotation
    pub fn view_projection(&self) -> Mat4;
}
```

- `view_projection` = `Mat4::perspective_infinite_reverse_rh(fov_y,
  aspect, near) * view`, where `view` is the inverse of the camera's
  TR transform. Near maps to depth 1, infinity to 0. There is no
  `far` field by design.
- View convention: identity rotation looks down −Z with +Y up
  (right-handed). `look_at` is a convenience over `rotation`.

## Mesh

Fixed vertex format for v1 (48 bytes, `bytemuck::Pod`):

```rust
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal:   [f32; 3],
    pub uv:       [f32; 2],
    pub color:    [f32; 4],
}

pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices:  Vec<u32>,
}
```

Normals and UVs ride along even though v1 is unlit and untextured:
they are free in the generators, and adding them later would force
regenerating and re-uploading every mesh the moment lighting lands.
`MeshData` is plain CPU data — callers may paint per-vertex colors
before upload.

`Renderer3D::create_mesh(&mut self, gpu: &Gpu, data: &MeshData) ->
MeshHandle` uploads vertex/index buffers once and returns a
`u32`-backed handle (the `TextureHandle` pattern). No `destroy_mesh`
in v1 (deferred, together with generational handle safety).

## Primitives

Free functions returning `MeshData`, all centered at the origin,
CCW winding viewed from outside, vertex colors default white:

- `cube(size: f32)` — `size` is the full edge length; per-face
  normals/UVs, 24 vertices, 36 indices.
- `uv_sphere(radius: f32, sectors: u32, stacks: u32)` — smooth
  normals (= normalized position), equirectangular UVs.
- `plane(width: f32, depth: f32)` — XZ plane facing +Y, 4 vertices.

## Renderer3D frame flow

```rust
renderer.begin(&camera);                        // captures view_proj, clears lists
renderer.draw_mesh(handle, world: Mat4, tint: Color);
renderer.draw_line(a: Vec3, b: Vec3, color: Color);
renderer.draw_grid(half_extent: f32, spacing: f32, color: Color); // XZ at y=0
renderer.draw_axes(transform: Mat4, length: f32); // RGB = XYZ, drawn in that frame
renderer.end(&gpu, &mut encoder, target: &TextureView, target_size: Size<Physical>);
```

`end()`:

1. Sorts the draw list by `MeshHandle`.
2. Writes one storage buffer of per-draw data
   (`world: Mat4`, `tint: [f32; 4]`).
3. Records a single render pass: color `Load` (clearing the target
   stays the user's job — same contract as `Renderer2D`), depth
   `Clear(0.0)`.
4. Each run of identical handles issues **one instanced draw call**
   (`draw_indexed`, instance range `i..i+n`); the vertex shader
   indexes the storage buffer by `instance_index`. Instancing falls
   out of the sort — perf headroom without an API change.
5. Debug lines accumulate in a dynamic vertex buffer and draw after
   the meshes in the same pass.

Depth texture: `Depth32Float`, owned by the renderer, lazily
recreated inside `end()` when `target_size` differs from the cached
size. Passing the size explicitly avoids resize-event-ordering bugs.

Profiling: `profile_function!()` in `end()`, counters for draw calls
and instances, matching `Renderer2D`'s instrumentation style.

## Pipelines and shaders

Two pipelines sharing bind group 0 (camera uniform: `view_proj`) and
bind group 1 (per-draw storage buffer):

- **Mesh**: triangle list, back-face cull, depth test `GreaterEqual`
  + depth write.
- **Line**: line list, depth test `GreaterEqual`, **no** depth write —
  debug lines occlude correctly behind geometry but never punch
  holes in it.

Unlit fragment: `tint × vertex_color`. Normals/UVs pass through
unused (texture sampling is the first follow-up and slots into bind
group 2).

## 2D-over-3D composition

Pure pass ordering in user code: clear pass → `renderer3d.end()`
(own depth) → `renderer2d.end()` (no depth attachment) → 2D HUD lands
on top. No renderer changes; documented in crate docs, demonstrated
when the UI work arrives.

## Error handling

Same contract as `Renderer2D`:

- Stale/invalid `MeshHandle` in `draw_mesh` → panic with a clear
  message (programmer error, not recoverable).
- Zero-sized target in `end()` → skip the pass.
- Draws issued outside `begin`/`end` accumulate into the next frame;
  documented, not policed.

## Testing

- **Camera math** (mirrors the `Camera2D` NDC tests that caught the
  center-origin bug): a point straight ahead maps to NDC center;
  near-plane depth = 1 and distant depth → 0 (reverse-Z); aspect
  stretches X only; `look_at` points −Z at the target.
- **Primitive generators**: index ranges valid; normals unit-length
  and outward (`dot(n, v − center) > 0`); CCW winding from outside;
  exact vertex/index counts.
- **Draw-list batching**: sorted runs produce expected instance
  ranges (pure CPU test).
- No headless-GPU tests in v1; visual validation via the example.

## Demo (`render_3d_demo`)

Scene-tree-driven, mirroring `scene_demo` in 3D: spinning
vertex-rainbow cube at the origin, two spheres orbiting as children,
grid at y=0 + axes at the origin, slowly orbiting camera (example
code, not an engine controller). `MeshInstance { handle, tint }`
component and the `Render`-phase system live in the example, reading
cached `world` matrices from the scene — the glue pattern is
observed here, frozen later.

## Deferred (explicitly out of scope for v1)

- Lighting and materials beyond `tint × vertex_color`
- Texture sampling (UVs already in the vertex format; bind group 2)
- Model loading (glTF first, given the RH +Y-up convention choice)
- `destroy_mesh` / generational mesh handles
- Shared frame-targets helper in `astrelis-gpu` (extract when a
  second depth-sharing pass exists)
- `astrelis-scene-render` glue crate (decide during UI work)
- Camera controllers (orbit/fly) as engine code

## Anti-debt rules (carried from the scene spec)

- One concern per crate; `astrelis-render-3d` (layer 2) never knows
  about the scene (layer 3). Glue is one-directional, downstream.
- No trait-object render abstractions; concrete types.
- No speculative pass/render-graph machinery — extract from real
  multi-pass needs (shadows, post), don't predict.
