struct Camera {
    view_projection: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var sprite_texture: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) basis_x: vec2<f32>,
    @location(1) basis_y: vec2<f32>,
    @location(2) translation: vec2<f32>,
    @location(3) size: vec2<f32>,
    @location(4) pivot: vec2<f32>,
    @location(5) uv_rect: vec4<f32>,
    @location(6) color: vec4<f32>,
) -> VertexOut {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex_index];
    let local = (corner - pivot) * size;
    let world = translation + basis_x * local.x + basis_y * local.y;
    var out: VertexOut;
    out.clip_position = camera.view_projection * vec4<f32>(world, 0.0, 1.0);
    out.uv = mix(uv_rect.xy, uv_rect.zw, corner);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let sampled = textureSample(sprite_texture, sprite_sampler, in.uv) * in.color;
    return vec4<f32>(sampled.rgb * sampled.a, sampled.a);
}
