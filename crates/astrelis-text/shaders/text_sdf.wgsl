// SDF Text Rendering Shader
// Signed Distance Field text rendering with support for effects (shadows, outlines, glows)

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

// SDF-specific parameters for effects
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

// Sample the SDF value at given UV coordinates
fn sample_sdf(uv: vec2<f32>) -> f32 {
    return textureSample(t_atlas, s_atlas, uv).r;
}

// Convert SDF distance to alpha using smoothstep for anti-aliasing
fn sdf_to_alpha(distance: f32, edge_width: f32) -> f32 {
    // SDF convention: 0.5 = edge, >0.5 = inside, <0.5 = outside
    // smoothstep creates a smooth transition at the edge
    return smoothstep(0.5 - edge_width, 0.5 + edge_width, distance);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let distance = sample_sdf(in.tex_coords);

    // Calculate adaptive edge width using screen-space derivatives
    // This ensures consistent anti-aliasing regardless of scale
    let fw = fwidth(distance);
    let edge_width = sdf_params.edge_softness + fw * 0.5;

    // Core text alpha
    let text_alpha = sdf_to_alpha(distance, edge_width);
    var result_color = in.color;
    result_color.a *= text_alpha;

    // Apply outline effect (renders at a threshold < 0.5)
    if (sdf_params.outline_width > 0.0) {
        // Outline threshold is lower than text threshold
        let outline_threshold = 0.5 - sdf_params.outline_width * 0.1;
        let outline_alpha = smoothstep(
            outline_threshold - edge_width,
            outline_threshold + edge_width,
            distance
        );

        // Blend outline under text
        let outline_color = vec4<f32>(
            sdf_params.outline_color.rgb,
            sdf_params.outline_color.a * outline_alpha * (1.0 - text_alpha)
        );
        result_color = blend_colors(outline_color, result_color);
    }

    // Apply shadow effect (sample at offset UV)
    if (sdf_params.shadow_color.a > 0.0 && (sdf_params.shadow_offset.x != 0.0 || sdf_params.shadow_offset.y != 0.0 || sdf_params.shadow_blur > 0.0)) {
        // Calculate shadow UV offset (normalized to texture space)
        let shadow_uv = in.tex_coords - sdf_params.shadow_offset * 0.01;
        let shadow_distance = sample_sdf(shadow_uv);

        // Apply blur to shadow edges
        let shadow_edge_width = edge_width + sdf_params.shadow_blur * 0.05;
        let shadow_alpha = sdf_to_alpha(shadow_distance, shadow_edge_width);

        let shadow_color = vec4<f32>(
            sdf_params.shadow_color.rgb,
            sdf_params.shadow_color.a * shadow_alpha
        );

        // Shadow renders behind everything
        result_color = blend_colors(shadow_color, result_color);
    }

    // Apply glow effect (renders at a larger threshold)
    if (sdf_params.glow_radius > 0.0 && sdf_params.glow_color.a > 0.0) {
        // Glow extends beyond the text edge
        let glow_threshold = 0.5 - sdf_params.glow_radius * 0.05;
        let glow_edge_width = edge_width + sdf_params.glow_radius * 0.02;
        let glow_alpha = smoothstep(
            glow_threshold - glow_edge_width,
            glow_threshold + glow_edge_width,
            distance
        );

        // Soft falloff for glow
        let glow_falloff = 1.0 - smoothstep(glow_threshold, 0.5, distance);
        let final_glow_alpha = glow_alpha * glow_falloff * sdf_params.glow_color.a;

        let glow_color = vec4<f32>(
            sdf_params.glow_color.rgb,
            final_glow_alpha * (1.0 - result_color.a)
        );
        result_color = blend_colors(glow_color, result_color);
    }

    // Discard fully transparent fragments
    if (result_color.a < 0.01) {
        discard;
    }

    return result_color;
}

// Alpha blending: blend src over dst
fn blend_colors(src: vec4<f32>, dst: vec4<f32>) -> vec4<f32> {
    let out_alpha = src.a + dst.a * (1.0 - src.a);
    if (out_alpha < 0.001) {
        return vec4<f32>(0.0);
    }
    let out_rgb = (src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / out_alpha;
    return vec4<f32>(out_rgb, out_alpha);
}
