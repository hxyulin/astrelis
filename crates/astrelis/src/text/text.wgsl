@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) kind: f32,
    @location(3) clip: vec4<f32>,
    @location(4) @interpolate(flat) clip_radii: vec4<f32>,
};
@vertex fn vertex_main(@builtin(vertex_index) index: u32,
    @location(0) rect: vec4<f32>, @location(1) uv: vec4<f32>, @location(2) kind: vec4<f32>,
    @location(3) origin_axis_x: vec4<f32>, @location(4) axis_y: vec4<f32>, @location(5) color: vec4<f32>,
    @location(6) clip_axes: vec4<f32>, @location(7) clip_offset_half: vec4<f32>,
    @location(8) clip_radii: vec4<f32>) -> Output {
    let corners = array(vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 0.0),
                        vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0));
    let corner = corners[index];
    let local = rect.xy + corner * rect.zw;
    let destination = origin_axis_x.xy + local.x * origin_axis_x.zw + local.y * axis_y.xy;
    var output: Output;
    output.position = vec4(destination * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    output.uv = uv.xy + corner * uv.zw;
    output.color = color;
    output.kind = kind.x;
    output.clip = clip_varying(destination, clip_axes, clip_offset_half);
    output.clip_radii = clip_radii;
    return output;
}
fn shade_fragment_main(input: Output) -> vec4<f32> {
    let texel = textureSample(atlas, atlas_sampler, input.uv);
    let clip = clip_coverage(input.clip, input.clip_radii);
    if input.kind == 0.0 {
        let alpha = texel.r * input.color.a * clip;
        return vec4(input.color.rgb * alpha, alpha);
    }
    // Color atlas stores linear, premultiplied RGBA. Draw RGB colors masks only.
    return texel * input.color.a * clip;
}

fn shade_fragment_mtsdf(input: Output) -> vec4<f32> {
    let texel = textureSample(atlas, atlas_sampler, input.uv);
    let distance = max(min(texel.r, texel.g), min(max(texel.r, texel.g), texel.b)) - 0.5;
    // The gradient of reconstructed distance follows the screen-space edge normal,
    // accounting for rotation, reflection, and anisotropic affine scaling.
    let distance_width = max(length(vec2(dpdx(distance), dpdy(distance))), 0.000001);
    let coverage = clamp(distance / distance_width + 0.5, 0.0, 1.0);
    let alpha = coverage * input.color.a * clip_coverage(input.clip, input.clip_radii);
    return vec4(input.color.rgb * alpha, alpha);
}

@fragment fn fragment_main(input: Output) -> @location(0) vec4<f32> {
    return shade_fragment_main(input);
}
@fragment fn fragment_covered(input: Output) -> @location(0) vec4<f32> {
    let color = shade_fragment_main(input);
    if color.a <= 0.0 { discard; }
    return color;
}

@fragment fn fragment_mtsdf(input: Output) -> @location(0) vec4<f32> {
    return shade_fragment_mtsdf(input);
}
@fragment fn fragment_mtsdf_covered(input: Output) -> @location(0) vec4<f32> {
    let color = shade_fragment_mtsdf(input);
    if color.a <= 0.0 { discard; }
    return color;
}
