// Text rendering shader using glyph atlas

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@group(0) @binding(0)
var glyph_atlas: texture_2d<f32>;

@group(0) @binding(1)
var atlas_sampler: sampler;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Simple orthographic projection (screen space -1 to 1)
    // Assuming normalized device coordinates are passed in
    out.clip_position = vec4<f32>(in.position, 1.0);
    out.tex_coords = in.tex_coords;
    out.color = in.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the glyph atlas (single channel - coverage/alpha)
    let alpha = textureSample(glyph_atlas, atlas_sampler, in.tex_coords).r;

    // Apply text color with sampled alpha
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
