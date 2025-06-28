struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct InstanceData {
    @location(1) translation: vec2<f32>,
    @location(2) rotation: f32,
    @location(3) scale: vec2<f32>,
    @location(4) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

fn get_rotation_matrix(rotation: f32) -> mat2x2<f32> {
    let c = cos(rotation);
    let s = sin(rotation);
    return mat2x2<f32>(vec2<f32>(c, -s), vec2<f32>(s, c));
}

@vertex
fn vs_main(input: VertexInput, instance: InstanceData) -> VertexOutput {
    // Model matrix from instance data (scale, rotate, translate)
    let scale_matrix = mat2x2<f32>(vec2<f32>(instance.scale.x, 0.0), vec2<f32>(0.0, instance.scale.y));
    let rotation_matrix = get_rotation_matrix(instance.rotation);
    let model_position = instance.translation + rotation_matrix * scale_matrix * input.position;

    // Convert model position to 4D vector for matrix multiplication
    let model_position_4d = vec4<f32>(model_position, 0.0, 1.0);

    // Apply view and projection transformations
    var output: VertexOutput;
    output.position = model_position_4d;
    output.color = instance.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
