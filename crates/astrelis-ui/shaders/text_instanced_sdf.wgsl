// UI Text Instanced SDF Shader - renders SDF text glyphs with effects using instanced rendering
// This extends the basic instanced text shader with SDF-based effects support

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
var atlas_texture: texture_2d<f32>;

@group(0) @binding(1)
var atlas_sampler: sampler;

@group(1) @binding(0)
var<uniform> uniforms: Uniforms;

@group(2) @binding(0)
var<uniform> sdf_params: SdfParams;

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

// Sample the SDF value at given UV coordinates
fn sample_sdf(uv: vec2<f32>) -> f32 {
    return textureSample(atlas_texture, atlas_sampler, uv).r;
}

// Convert SDF distance to alpha using smoothstep for anti-aliasing
fn sdf_to_alpha(distance: f32, edge_width: f32) -> f32 {
    // SDF convention: 0.5 = edge, >0.5 = inside, <0.5 = outside
    return smoothstep(0.5 - edge_width, 0.5 + edge_width, distance);
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

    // Discard fully transparent fragments for better performance
    if (result_color.a < 0.01) {
        discard;
    }

    return result_color;
}
