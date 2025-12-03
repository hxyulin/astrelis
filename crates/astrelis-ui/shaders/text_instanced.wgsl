// UI Text Instanced Shader - renders text glyphs from atlas using instanced rendering
// This is part of Phase 5: Instance-Based Retained Rendering

// Per-vertex data (shared unit quad geometry)
struct VertexInput {
    @location(0) position: vec2<f32>,  // Unit quad position (0-1)
    @location(1) uv: vec2<f32>,         // UV coordinates (0-1)
}

// Per-instance data (unique for each glyph)
struct InstanceInput {
    @location(2) instance_position: vec2<f32>,     // Screen position
    @location(3) instance_size: vec2<f32>,         // Glyph size in pixels
    @location(4) instance_atlas_uv_min: vec2<f32>, // Atlas UV top-left
    @location(5) instance_atlas_uv_max: vec2<f32>, // Atlas UV bottom-right
    @location(6) instance_color: vec4<f32>,        // Text color
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct Uniforms {
    projection: mat4x4<f32>,
}

@group(0) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(0) @binding(1)
var atlas_sampler: sampler;

@group(1) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // Scale and position the unit quad based on instance data
    let world_pos = instance.instance_position + vertex.position * instance.instance_size;
    out.clip_position = uniforms.projection * vec4<f32>(world_pos, 0.0, 1.0);

    // Interpolate atlas UV coordinates based on vertex position in unit quad
    out.tex_coords = mix(
        instance.instance_atlas_uv_min,
        instance.instance_atlas_uv_max,
        vertex.uv
    );

    out.color = instance.instance_color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the glyph from the atlas (R8 format, alpha channel)
    let alpha = textureSample(atlas_texture, atlas_sampler, in.tex_coords).r;

    // Apply alpha to the text color
    var color = in.color;
    color.a *= alpha;

    // Discard fully transparent fragments for better performance
    if (color.a < 0.01) {
        discard;
    }

    return color;
}
