struct Parameters {
    destination: vec4<f32>,
    source: vec4<f32>,
    tint: vec4<f32>,
};
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@group(0) @binding(2) var<uniform> parameters: Parameters;

struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex fn vertex_main(@builtin(vertex_index) index: u32) -> Output {
    let corners = array(vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 0.0),
                        vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0));
    let corner = corners[index];
    let destination = parameters.destination.xy + corner * parameters.destination.zw;
    var output: Output;
    output.position = vec4(destination * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    output.uv = parameters.source.xy + corner * parameters.source.zw;
    return output;
}

fn tinted(color: vec4<f32>) -> vec4<f32> {
    return vec4(color.rgb * parameters.tint.rgb * parameters.tint.a, color.a * parameters.tint.a);
}

@fragment fn fragment_straight(input: Output) -> @location(0) vec4<f32> {
    let color = textureSample(image, image_sampler, input.uv);
    return tinted(vec4(color.rgb * color.a, color.a));
}

@fragment fn fragment_premultiplied(input: Output) -> @location(0) vec4<f32> {
    return tinted(textureSample(image, image_sampler, input.uv));
}
