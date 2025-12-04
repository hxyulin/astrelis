// Image instanced rendering shader
// Renders textured quads with UV coordinates, tint color, and optional rounded corners

struct Projection {
    matrix: mat4x4<f32>,
}

@group(0) @binding(0) var image_texture: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@group(1) @binding(0) var<uniform> projection: Projection;

// Per-vertex data (unit quad)
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

// Per-instance data
struct InstanceInput {
    @location(2) inst_position: vec2<f32>,
    @location(3) inst_size: vec2<f32>,
    @location(4) inst_uv_min: vec2<f32>,
    @location(5) inst_uv_max: vec2<f32>,
    @location(6) inst_tint: vec4<f32>,
    @location(7) inst_border_radius: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
    @location(2) local_pos: vec2<f32>,
    @location(3) size: vec2<f32>,
    @location(4) border_radius: f32,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform unit quad to world position
    let world_pos = instance.inst_position + vertex.position * instance.inst_size;
    out.clip_position = projection.matrix * vec4<f32>(world_pos, 0.0, 1.0);
    
    // Interpolate UV coordinates based on instance UV bounds
    out.uv = mix(instance.inst_uv_min, instance.inst_uv_max, vertex.uv);
    
    out.tint = instance.inst_tint;
    out.local_pos = vertex.position * instance.inst_size;
    out.size = instance.inst_size;
    out.border_radius = instance.inst_border_radius;
    
    return out;
}

// Signed distance function for rounded rectangle
fn rounded_rect_sdf(pos: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let half_size = size * 0.5;
    let center_pos = pos - half_size;
    let q = abs(center_pos) - half_size + vec2<f32>(radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample texture
    let tex_color = textureSample(image_texture, image_sampler, in.uv);
    
    // Apply tint
    var color = tex_color * in.tint;
    
    // Apply rounded corners if border_radius > 0
    if in.border_radius > 0.0 {
        let dist = rounded_rect_sdf(in.local_pos, in.size, in.border_radius);
        // Anti-aliased edge
        let alpha = 1.0 - smoothstep(-1.0, 1.0, dist);
        color.a *= alpha;
    }
    
    return color;
}
