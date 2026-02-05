// UI Quad Instanced Shader - renders colored rectangles with rounded corners using instanced rendering
// This is part of Phase 5: Instance-Based Retained Rendering

// Per-vertex data (shared unit quad geometry)
struct VertexInput {
    @location(0) position: vec2<f32>,  // Unit quad position (0-1)
    @location(1) uv: vec2<f32>,         // UV coordinates
}

// Per-instance data (unique for each quad)
struct InstanceInput {
    @location(2) instance_position: vec2<f32>,
    @location(3) instance_size: vec2<f32>,
    @location(4) instance_color: vec4<f32>,
    @location(5) instance_border_radius: f32,
    @location(6) instance_border_thickness: f32,
    @location(7) instance_z_depth: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) border_radius: f32,
    @location(3) rect_size: vec2<f32>,
    @location(4) border_thickness: f32,
}

struct Uniforms {
    projection: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // Scale and position the unit quad based on instance data
    let world_pos = instance.instance_position + vertex.position * instance.instance_size;
    out.clip_position = uniforms.projection * vec4<f32>(world_pos, instance.instance_z_depth, 1.0);

    out.color = instance.instance_color;
    out.uv = vertex.uv;
    out.border_radius = instance.instance_border_radius;
    out.rect_size = instance.instance_size;
    out.border_thickness = instance.instance_border_thickness;

    return out;
}

// Signed distance function for a rounded rectangle
fn sdf_rounded_rect(pos: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(pos) - size * 0.5 + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Convert UV (0-1) to pixel coordinates centered at rect center
    let pixel_pos = (in.uv - 0.5) * in.rect_size;

    // Calculate signed distance to rounded rectangle
    let dist = sdf_rounded_rect(pixel_pos, in.rect_size, in.border_radius);

    var alpha: f32;

    if (in.border_thickness > 0.0) {
        // Border mode: render only the outline
        let inner_dist = sdf_rounded_rect(
            pixel_pos,
            in.rect_size - vec2<f32>(in.border_thickness * 2.0),
            max(in.border_radius - in.border_thickness, 0.0)
        );

        // Alpha is 1 where we're inside the outer edge but outside the inner edge
        let outer_alpha = 1.0 - smoothstep(-1.0, 1.0, dist);
        let inner_alpha = 1.0 - smoothstep(-1.0, 1.0, inner_dist);
        alpha = outer_alpha * (1.0 - inner_alpha);
    } else {
        // Filled mode: render the entire shape
        alpha = 1.0 - smoothstep(-1.0, 1.0, dist);
    }

    // Apply alpha to color
    var color = in.color;
    color.a *= alpha;

    return color;
}
