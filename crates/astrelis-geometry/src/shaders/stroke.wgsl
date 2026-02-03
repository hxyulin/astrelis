// Stroke shader for geometry rendering
// Renders stroked paths with instancing support

struct ProjectionUniform {
    matrix: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> projection: ProjectionUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) normal: vec2<f32>,
    @location(2) distance: f32,
    @location(3) side: f32,
}

struct InstanceInput {
    @location(4) transform_row0: vec4<f32>,
    @location(5) transform_row1: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) width: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) distance: f32,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // Apply 2D transform to the position
    let a = instance.transform_row0.x;
    let b = instance.transform_row0.y;
    let c = instance.transform_row1.x;
    let d = instance.transform_row1.y;
    let tx = instance.transform_row1.z;
    let ty = instance.transform_row1.w;

    // The stroke vertices are already expanded by the tessellator
    // We just need to apply the transform
    let transformed = vec2<f32>(
        a * vertex.position.x + c * vertex.position.y + tx,
        b * vertex.position.x + d * vertex.position.y + ty
    );

    out.position = projection.matrix * vec4<f32>(transformed, 0.0, 1.0);
    out.color = instance.color;
    out.distance = vertex.distance;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
