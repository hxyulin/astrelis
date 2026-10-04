struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) geometry: vec4<f32>,
    @location(2) @interpolate(flat) style: vec2<f32>,
    @location(3) @interpolate(flat) color: vec4<f32>,
};
@vertex fn vertex_main(@builtin(vertex_index) index: u32,
    @location(0) origin_axis_x: vec4<f32>, @location(1) axis_y_min: vec4<f32>,
    @location(2) span_style: vec4<f32>, @location(3) geometry: vec4<f32>,
    @location(4) color: vec4<f32>) -> Output {
    let corners = array(vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 0.0),
                        vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0));
    let local = axis_y_min.zw + corners[index] * span_style.xy;
    let position = origin_axis_x.xy + local.x * origin_axis_x.zw + local.y * axis_y_min.xy;
    var output: Output;
    output.position = vec4(position * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    output.local = local;
    output.geometry = geometry;
    output.style = span_style.zw;
    output.color = color;
    return output;
}
fn box_distance(p: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let q = abs(p) - half_size;
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0);
}
@fragment fn fragment_main(input: Output) -> @location(0) vec4<f32> {
    let half_size = input.geometry.xy;
    let kind = u32(input.style.x);
    let radius = input.style.y;
    var distance = box_distance(input.local, half_size);
    if kind == 1u {
        distance = box_distance(input.local, half_size - vec2(radius)) - radius;
    } else if kind == 2u {
        distance = length(input.local / max(half_size, vec2(0.000001))) - 1.0;
    } else if kind == 4u {
        distance = box_distance(input.local, vec2(half_size.x + half_size.y, half_size.y));
    } else if kind == 5u {
        distance = length(vec2(max(abs(input.local.x) - half_size.x, 0.0), input.local.y)) - half_size.y;
    }
    // Derivatives run outside primitive-dependent control flow. Geometry's final
    // component selects analytic coverage independently of attachment MSAA.
    let edge_width = max(fwidth(distance), 0.000001);
    // Bound coverage by the filtered local box. Opposing fringes overlap for
    // subpixel geometry; a single distance threshold otherwise makes thin lines
    // too opaque. The box filter is approximate under rotation/shear, like fwidth.
    let footprint = max(fwidth(input.local), vec2(0.000001));
    var bounds = half_size;
    if kind == 4u || kind == 5u { bounds.x += half_size.y; }
    let positive = clamp((bounds - input.local) / footprint + vec2(0.5), vec2(0.0), vec2(1.0));
    let negative = clamp((-bounds - input.local) / footprint + vec2(0.5), vec2(0.0), vec2(1.0));
    let box_coverage = positive - negative;
    let analytic = min(clamp(0.5 - distance / edge_width, 0.0, 1.0), box_coverage.x * box_coverage.y);
    let coverage = select(select(0.0, 1.0, distance <= 0.0), analytic, input.geometry.w > 0.0);
    let alpha = input.color.a * coverage;
    return vec4(input.color.rgb * alpha, alpha);
}
