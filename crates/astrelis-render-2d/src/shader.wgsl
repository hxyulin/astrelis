// Unified 2D shader for sprites, rectangles, circles, and lines.
//
// All draw types share the same vertex/instance layout. The fragment
// shader branches on `draw_type` to handle textured sprites vs SDF
// circles vs solid shapes.

// Camera uniform buffer.
struct Camera {
    view_projection: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

// Texture + sampler (group 1 — rebound per texture batch).
@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

// Per-instance data.
struct InstanceInput {
    @location(0) position: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv_min: vec2<f32>,
    @location(3) uv_max: vec2<f32>,
    @location(4) color: vec4<f32>,
    @location(5) rotation: f32,
    @location(6) z_depth: f32,
    @location(7) texture_index: u32,
    @location(8) draw_type: u32,
}

// Vertex output.
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) draw_type: u32,
}

// Fullscreen-quad vertices: 6 vertices forming 2 triangles.
// Vertex index → local position [0,1] within the quad.
fn quad_uv(vertex_index: u32) -> vec2<f32> {
    // Triangle 1: 0,1,2  Triangle 2: 2,1,3
    // Arranged as: TL(0,0) TR(1,0) BL(0,1) BR(1,1)
    let idx = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), // TL
        vec2(1.0, 0.0), // TR
        vec2(0.0, 1.0), // BL
        vec2(0.0, 1.0), // BL
        vec2(1.0, 0.0), // TR
        vec2(1.0, 1.0), // BR
    );
    return idx[vertex_index];
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Local quad position [0,1].
    let local = quad_uv(vertex_index);

    // Scale to instance size.
    var world_offset = local * instance.size;

    // Apply rotation around origin (0,0 of the quad).
    let cos_r = cos(instance.rotation);
    let sin_r = sin(instance.rotation);
    let rotated = vec2(
        world_offset.x * cos_r - world_offset.y * sin_r,
        world_offset.x * sin_r + world_offset.y * cos_r,
    );

    // Translate to world position.
    let world_pos = instance.position + rotated;

    // Project.
    out.clip_position = camera.view_projection * vec4(world_pos, instance.z_depth, 1.0);

    // Interpolate UVs between uv_min and uv_max.
    out.uv = mix(instance.uv_min, instance.uv_max, local);

    out.color = instance.color;
    out.draw_type = instance.draw_type;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let draw_type = in.draw_type;

    // Sprite (draw_type == 0): sample texture and multiply by tint.
    if draw_type == 0u {
        let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
        return tex_color * in.color;
    }

    // Rectangle (draw_type == 1): solid color, sample white texture.
    if draw_type == 1u {
        let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
        return tex_color * in.color;
    }

    // Circle (draw_type == 2): SDF circle.
    if draw_type == 2u {
        let center = vec2(0.5, 0.5);
        let dist = distance(in.uv, center);
        // Smooth edge with ~1px anti-aliasing.
        let alpha = 1.0 - smoothstep(0.48, 0.50, dist);
        return vec4(in.color.rgb, in.color.a * alpha);
    }

    // Line (draw_type == 3): solid color.
    return in.color;
}
