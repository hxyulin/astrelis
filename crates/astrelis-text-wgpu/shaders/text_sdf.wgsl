// SDF Text Rendering Shader
// Basic Signed Distance Field text rendering.
// Effects (shadow, outline, glow) are not yet implemented.

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct Uniforms {
    projection: mat4x4<f32>,
}

struct SdfParams {
    edge_softness: f32,
    outline_width: f32,
    outline_color: vec4<f32>,
    shadow_offset: vec2<f32>,
    shadow_blur: f32,
    shadow_color: vec4<f32>,
    glow_radius: f32,
    glow_color: vec4<f32>,
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var t_atlas: texture_2d<f32>;
@group(0) @binding(1)
var s_atlas: sampler;

@group(1) @binding(0)
var<uniform> uniforms: Uniforms;

@group(2) @binding(0)
var<uniform> sdf_params: SdfParams;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.projection * vec4<f32>(in.position, 0.0, 1.0);
    out.tex_coords = in.tex_coords;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let distance = textureSample(t_atlas, s_atlas, in.tex_coords).r;

    // Adaptive edge width for anti-aliasing
    let fw = fwidth(distance);
    let edge_width = sdf_params.edge_softness + fw * 0.5;

    // SDF alpha: 0.5 = edge, >0.5 = inside, <0.5 = outside
    let alpha = smoothstep(0.5 - edge_width, 0.5 + edge_width, distance);

    if (alpha < 0.01) {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
