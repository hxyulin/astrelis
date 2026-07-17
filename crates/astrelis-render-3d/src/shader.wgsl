struct Frame {
    view_projection: mat4x4<f32>,
    camera_position: vec4<f32>,
    ambient: vec4<f32>,
    light_direction_intensity: vec4<f32>,
    light_color: vec4<f32>,
};

struct Material {
    base_color: vec4<f32>,
    alpha: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var albedo_texture: texture_2d<f32>;
@group(1) @binding(2) var albedo_sampler: sampler;

struct MeshOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_mesh(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) vertex_color: vec4<f32>,
    @location(4) model_0: vec4<f32>,
    @location(5) model_1: vec4<f32>,
    @location(6) model_2: vec4<f32>,
    @location(7) model_3: vec4<f32>,
    @location(8) normal_0: vec4<f32>,
    @location(9) normal_1: vec4<f32>,
    @location(10) normal_2: vec4<f32>,
    @location(11) tint: vec4<f32>,
) -> MeshOut {
    let model = mat4x4<f32>(model_0, model_1, model_2, model_3);
    let normal_matrix = mat3x3<f32>(normal_0.xyz, normal_1.xyz, normal_2.xyz);
    let world = model * vec4<f32>(position, 1.0);
    var out: MeshOut;
    out.clip_position = frame.view_projection * world;
    out.world_normal = normalize(normal_matrix * normal);
    out.uv = uv;
    out.color = vertex_color * tint;
    return out;
}

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    let sampled = textureSample(albedo_texture, albedo_sampler, in.uv);
    let surface = sampled * material.base_color * in.color;
    if material.alpha.y > 0.5 && surface.a < material.alpha.x {
        discard;
    }
    let diffuse = max(dot(normalize(in.world_normal), normalize(frame.light_direction_intensity.xyz)), 0.0);
    let illumination = frame.ambient.rgb + frame.light_color.rgb * frame.light_direction_intensity.w * diffuse;
    var alpha = surface.a;
    if material.alpha.y < 1.5 {
        alpha = 1.0;
    }
    let rgb = surface.rgb * illumination;
    return vec4<f32>(rgb * alpha, alpha);
}

struct LineOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_line(@location(0) position: vec3<f32>, @location(1) color: vec4<f32>) -> LineOut {
    var out: LineOut;
    out.clip_position = frame.view_projection * vec4<f32>(position, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_line(in: LineOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color.rgb * in.color.a, in.color.a);
}
