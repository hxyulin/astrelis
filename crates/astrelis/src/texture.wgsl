@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
};
@vertex fn vertex_main(@builtin(vertex_index) index: u32,
    @location(0) origin_axis_x: vec4<f32>, @location(1) axis_y: vec4<f32>,
    @location(2) source: vec4<f32>, @location(3) tint: vec4<f32>) -> Output {
    let corners = array(vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 0.0),
                        vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0));
    let corner = corners[index];
    let destination = origin_axis_x.xy + corner.x * origin_axis_x.zw + corner.y * axis_y.xy;
    var output: Output;
    output.position = vec4(destination * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    output.uv = source.xy + corner * source.zw;
    output.tint = tint;
    return output;
}
// Retained instance positions stay immutable; the extra transform is per draw.
@vertex fn vertex_transformed(@builtin(vertex_index) index: u32,
    @location(0) origin_axis_x: vec4<f32>, @location(1) axis_y: vec4<f32>,
    @location(2) source: vec4<f32>, @location(3) tint: vec4<f32>,
    @location(4) pixels: vec4<f32>, @location(5) normalized: vec4<f32>,
    @location(6) translations: vec4<f32>) -> Output {
    let corners = array(vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 0.0),
                        vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0));
    let corner = corners[index];
    let p = origin_axis_x.xy + corner.x * origin_axis_x.zw + corner.y * axis_y.xy;
    let m = select(pixels, normalized, axis_y.z != 0.0);
    let offset = select(translations.xy, translations.zw, axis_y.z != 0.0);
    let destination = vec2(m.x*p.x+m.z*p.y,m.y*p.x+m.w*p.y)+offset;
    var output: Output;
    output.position = vec4(destination * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    output.uv = source.xy + corner * source.zw;
    output.tint = tint;
    return output;
}
fn tinted(color: vec4<f32>, tint: vec4<f32>) -> vec4<f32> {
    return vec4(color.rgb * tint.rgb * tint.a, color.a * tint.a);
}
fn shade_fragment_straight(input: Output) -> vec4<f32> {
    let color = textureSample(image, image_sampler, input.uv);
    return tinted(vec4(color.rgb * color.a, color.a), input.tint);
}
// Linear filtering must interpolate premultiplied texels. Filtering straight RGB first
// blends the colour of transparent texels into edges as dark fringes, so gather the
// bilinear footprint of the first level, premultiply each texel, then weight it.
fn shade_fragment_straight_linear(input: Output) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(image));
    let texel = input.uv * size - 0.5;
    let base = floor(texel);
    let f = texel - base;
    // Gathering at the footprint centre keeps the footprint consistent with `base`.
    let centre = (base + 1.0) / size;
    let r = textureGather(0, image, image_sampler, centre);
    let g = textureGather(1, image, image_sampler, centre);
    let b = textureGather(2, image, image_sampler, centre);
    let a = textureGather(3, image, image_sampler, centre);
    // Gathered components are ordered (u0, v1), (u1, v1), (u1, v0), (u0, v0).
    let weights = vec4((1.0 - f.x) * f.y, f.x * f.y, f.x * (1.0 - f.y), (1.0 - f.x) * (1.0 - f.y));
    let coverage = weights * a;
    let color = vec4(dot(r, coverage), dot(g, coverage), dot(b, coverage), dot(a, weights));
    return tinted(color, input.tint);
}
fn shade_fragment_premultiplied(input: Output) -> vec4<f32> {
    return tinted(textureSample(image, image_sampler, input.uv), input.tint);
}

@fragment fn fragment_straight(input: Output) -> @location(0) vec4<f32> {
    return shade_fragment_straight(input);
}
@fragment fn fragment_straight_covered(input: Output) -> @location(0) vec4<f32> {
    let color = shade_fragment_straight(input);
    if color.a <= 0.0 { discard; }
    return color;
}

@fragment fn fragment_straight_linear(input: Output) -> @location(0) vec4<f32> {
    return shade_fragment_straight_linear(input);
}
@fragment fn fragment_straight_linear_covered(input: Output) -> @location(0) vec4<f32> {
    let color = shade_fragment_straight_linear(input);
    if color.a <= 0.0 { discard; }
    return color;
}

@fragment fn fragment_premultiplied(input: Output) -> @location(0) vec4<f32> {
    return shade_fragment_premultiplied(input);
}
@fragment fn fragment_premultiplied_covered(input: Output) -> @location(0) vec4<f32> {
    let color = shade_fragment_premultiplied(input);
    if color.a <= 0.0 { discard; }
    return color;
}
