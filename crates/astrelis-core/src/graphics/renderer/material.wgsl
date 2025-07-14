//! A Material Shader for a Instance Renderer
struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) texcoord: vec2<f32>,
};

struct InstanceData {
    @location(2) transform_c1: vec4<f32>,
    @location(3) transform_c2: vec4<f32>,
    @location(4) transform_c3: vec4<f32>,
    @location(5) transform_c4: vec4<f32>,
};

struct Camera {
    projection: mat4x4<f32>,
    view: mat4x4<f32>,
};

struct Material {
    diffuse_color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> mat: Material;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) texcoord: vec2<f32>,
};

@vertex
fn vs_main(input: Vertex, instance: InstanceData) -> VertexOutput {
    var out: VertexOutput;

    let model: mat4x4<f32> = mat4x4<f32>(
        instance.transform_c1,
        instance.transform_c2,
        instance.transform_c3,
        instance.transform_c4,
    );

    out.position = model * vec4<f32>(input.position, 1.0);
    out.texcoord = input.texcoord;
    out.color = mat.diffuse_color;

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
