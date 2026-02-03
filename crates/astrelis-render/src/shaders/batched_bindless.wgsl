// Batched bindless shader for Tier 3 (Bindless).
//
// Uses a binding_array of all textures (group 0) and a projection uniform (group 1).
// The texture_index instance field selects which texture to sample.
// Supports three draw types via draw_type:
//   0 = Solid quad (SDF rounded rect)
//   1 = Text glyph (R8 atlas alpha × color)
//   2 = Image (RGBA texture × tint)

// Bind group 0: bindless texture array + shared sampler
@group(0) @binding(0) var textures: binding_array<texture_2d<f32>>;
@group(0) @binding(1) var tex_sampler: sampler;

// Bind group 1: projection uniform
@group(1) @binding(0) var<uniform> projection: mat4x4<f32>;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct InstanceInput {
    @location(2) inst_position: vec2<f32>,
    @location(3) inst_size: vec2<f32>,
    @location(4) uv_min: vec2<f32>,
    @location(5) uv_max: vec2<f32>,
    @location(6) color: vec4<f32>,
    @location(7) border_radius: f32,
    @location(8) border_thickness: f32,
    @location(9) texture_index: u32,
    @location(10) draw_type: u32,
    @location(11) clip_min: vec2<f32>,
    @location(12) clip_max: vec2<f32>,
    @location(13) z_depth: f32,
    @location(14) _reserved: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) local_pos: vec2<f32>,
    @location(3) quad_size: vec2<f32>,
    @location(4) @interpolate(flat) draw_type: u32,
    @location(5) @interpolate(flat) border_radius: f32,
    @location(6) @interpolate(flat) border_thickness: f32,
    @location(7) world_pos: vec2<f32>,
    @location(8) clip_min: vec2<f32>,
    @location(9) clip_max: vec2<f32>,
    @location(10) @interpolate(flat) texture_index: u32,
}

@vertex
fn vs_main(v: VertexInput, i: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = i.inst_position + v.position * i.inst_size;
    var projected = projection * vec4<f32>(world_pos, 0.0, 1.0);
    projected.z = i.z_depth * projected.w;

    out.clip_position = projected;
    out.tex_coords = mix(i.uv_min, i.uv_max, v.tex_coords);
    out.color = i.color;
    out.local_pos = v.position * i.inst_size;
    out.quad_size = i.inst_size;
    out.draw_type = i.draw_type;
    out.border_radius = i.border_radius;
    out.border_thickness = i.border_thickness;
    out.world_pos = world_pos;
    out.clip_min = i.clip_min;
    out.clip_max = i.clip_max;
    out.texture_index = i.texture_index;

    return out;
}

fn sd_rounded_rect(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Shader-based clipping
    if in.world_pos.x < in.clip_min.x || in.world_pos.y < in.clip_min.y ||
       in.world_pos.x > in.clip_max.x || in.world_pos.y > in.clip_max.y {
        discard;
    }

    var final_color: vec4<f32>;

    if in.draw_type == 0u {
        // --- SOLID QUAD (SDF rounded rect) ---
        let half_size = in.quad_size * 0.5;
        let center_pos = in.local_pos - half_size;
        let radius = min(in.border_radius, min(half_size.x, half_size.y));

        if in.border_thickness > 0.0 {
            let dist = sd_rounded_rect(center_pos, half_size, radius);
            let outer_alpha = 1.0 - smoothstep(-0.5, 0.5, dist);
            let inner_dist = dist + in.border_thickness;
            let inner_alpha = 1.0 - smoothstep(-0.5, 0.5, inner_dist);
            let border_alpha = outer_alpha - inner_alpha;
            final_color = vec4<f32>(in.color.rgb, in.color.a * border_alpha);
        } else {
            let dist = sd_rounded_rect(center_pos, half_size, radius);
            let alpha = 1.0 - smoothstep(-0.5, 0.5, dist);
            final_color = vec4<f32>(in.color.rgb, in.color.a * alpha);
        }
    } else if in.draw_type == 1u {
        // --- TEXT GLYPH ---
        let atlas_alpha = textureSample(textures[in.texture_index], tex_sampler, in.tex_coords).r;
        if atlas_alpha < 0.01 {
            discard;
        }
        final_color = vec4<f32>(in.color.rgb, in.color.a * atlas_alpha);
    } else {
        // --- IMAGE ---
        let tex_color = textureSample(textures[in.texture_index], tex_sampler, in.tex_coords);
        final_color = tex_color * in.color;

        if in.border_radius > 0.0 {
            let half_size = in.quad_size * 0.5;
            let center_pos = in.local_pos - half_size;
            let radius = min(in.border_radius, min(half_size.x, half_size.y));
            let dist = sd_rounded_rect(center_pos, half_size, radius);
            let mask = 1.0 - smoothstep(-0.5, 0.5, dist);
            final_color = vec4<f32>(final_color.rgb, final_color.a * mask);
        }
    }

    if final_color.a < 0.001 {
        discard;
    }

    return final_color;
}
