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
