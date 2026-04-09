//! Core type conversions between `astrelis_gpu::types` and `wgpu`.

use astrelis_gpu::types::*;

pub(crate) fn texture_format(f: TextureFormat) -> wgpu::TextureFormat {
    match f {
        TextureFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
        TextureFormat::R8Snorm => wgpu::TextureFormat::R8Snorm,
        TextureFormat::R8Uint => wgpu::TextureFormat::R8Uint,
        TextureFormat::R8Sint => wgpu::TextureFormat::R8Sint,
        TextureFormat::R16Uint => wgpu::TextureFormat::R16Uint,
        TextureFormat::R16Sint => wgpu::TextureFormat::R16Sint,
        TextureFormat::R16Float => wgpu::TextureFormat::R16Float,
        TextureFormat::Rg8Unorm => wgpu::TextureFormat::Rg8Unorm,
        TextureFormat::Rg8Snorm => wgpu::TextureFormat::Rg8Snorm,
        TextureFormat::Rg8Uint => wgpu::TextureFormat::Rg8Uint,
        TextureFormat::Rg8Sint => wgpu::TextureFormat::Rg8Sint,
        TextureFormat::R32Uint => wgpu::TextureFormat::R32Uint,
        TextureFormat::R32Sint => wgpu::TextureFormat::R32Sint,
        TextureFormat::R32Float => wgpu::TextureFormat::R32Float,
        TextureFormat::Rg16Uint => wgpu::TextureFormat::Rg16Uint,
        TextureFormat::Rg16Sint => wgpu::TextureFormat::Rg16Sint,
        TextureFormat::Rg16Float => wgpu::TextureFormat::Rg16Float,
        TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Rgba8Snorm => wgpu::TextureFormat::Rgba8Snorm,
        TextureFormat::Rgba8Uint => wgpu::TextureFormat::Rgba8Uint,
        TextureFormat::Rgba8Sint => wgpu::TextureFormat::Rgba8Sint,
        TextureFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Rg32Uint => wgpu::TextureFormat::Rg32Uint,
        TextureFormat::Rg32Sint => wgpu::TextureFormat::Rg32Sint,
        TextureFormat::Rg32Float => wgpu::TextureFormat::Rg32Float,
        TextureFormat::Rgba16Uint => wgpu::TextureFormat::Rgba16Uint,
        TextureFormat::Rgba16Sint => wgpu::TextureFormat::Rgba16Sint,
        TextureFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
        TextureFormat::Rgba32Uint => wgpu::TextureFormat::Rgba32Uint,
        TextureFormat::Rgba32Sint => wgpu::TextureFormat::Rgba32Sint,
        TextureFormat::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
        TextureFormat::Depth16Unorm => wgpu::TextureFormat::Depth16Unorm,
        TextureFormat::Depth24Plus => wgpu::TextureFormat::Depth24Plus,
        TextureFormat::Depth24PlusStencil8 => wgpu::TextureFormat::Depth24PlusStencil8,
        TextureFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
        TextureFormat::Depth32FloatStencil8 => wgpu::TextureFormat::Depth32FloatStencil8,
        _ => unreachable!("unknown texture format"),
    }
}

pub(crate) fn texture_format_from_wgpu(f: wgpu::TextureFormat) -> TextureFormat {
    match f {
        wgpu::TextureFormat::R8Unorm => TextureFormat::R8Unorm,
        wgpu::TextureFormat::R8Snorm => TextureFormat::R8Snorm,
        wgpu::TextureFormat::R8Uint => TextureFormat::R8Uint,
        wgpu::TextureFormat::R8Sint => TextureFormat::R8Sint,
        wgpu::TextureFormat::R16Uint => TextureFormat::R16Uint,
        wgpu::TextureFormat::R16Sint => TextureFormat::R16Sint,
        wgpu::TextureFormat::R16Float => TextureFormat::R16Float,
        wgpu::TextureFormat::Rg8Unorm => TextureFormat::Rg8Unorm,
        wgpu::TextureFormat::Rg8Snorm => TextureFormat::Rg8Snorm,
        wgpu::TextureFormat::Rg8Uint => TextureFormat::Rg8Uint,
        wgpu::TextureFormat::Rg8Sint => TextureFormat::Rg8Sint,
        wgpu::TextureFormat::R32Uint => TextureFormat::R32Uint,
        wgpu::TextureFormat::R32Sint => TextureFormat::R32Sint,
        wgpu::TextureFormat::R32Float => TextureFormat::R32Float,
        wgpu::TextureFormat::Rg16Uint => TextureFormat::Rg16Uint,
        wgpu::TextureFormat::Rg16Sint => TextureFormat::Rg16Sint,
        wgpu::TextureFormat::Rg16Float => TextureFormat::Rg16Float,
        wgpu::TextureFormat::Rgba8Unorm => TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb => TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Rgba8Snorm => TextureFormat::Rgba8Snorm,
        wgpu::TextureFormat::Rgba8Uint => TextureFormat::Rgba8Uint,
        wgpu::TextureFormat::Rgba8Sint => TextureFormat::Rgba8Sint,
        wgpu::TextureFormat::Bgra8Unorm => TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb => TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rg32Uint => TextureFormat::Rg32Uint,
        wgpu::TextureFormat::Rg32Sint => TextureFormat::Rg32Sint,
        wgpu::TextureFormat::Rg32Float => TextureFormat::Rg32Float,
        wgpu::TextureFormat::Rgba16Uint => TextureFormat::Rgba16Uint,
        wgpu::TextureFormat::Rgba16Sint => TextureFormat::Rgba16Sint,
        wgpu::TextureFormat::Rgba16Float => TextureFormat::Rgba16Float,
        wgpu::TextureFormat::Rgba32Uint => TextureFormat::Rgba32Uint,
        wgpu::TextureFormat::Rgba32Sint => TextureFormat::Rgba32Sint,
        wgpu::TextureFormat::Rgba32Float => TextureFormat::Rgba32Float,
        wgpu::TextureFormat::Depth16Unorm => TextureFormat::Depth16Unorm,
        wgpu::TextureFormat::Depth24Plus => TextureFormat::Depth24Plus,
        wgpu::TextureFormat::Depth24PlusStencil8 => TextureFormat::Depth24PlusStencil8,
        wgpu::TextureFormat::Depth32Float => TextureFormat::Depth32Float,
        wgpu::TextureFormat::Depth32FloatStencil8 => TextureFormat::Depth32FloatStencil8,
        _ => TextureFormat::Rgba8Unorm, // Fallback for formats we don't wrap
    }
}

pub(crate) fn present_mode(m: PresentMode) -> wgpu::PresentMode {
    match m {
        PresentMode::Immediate => wgpu::PresentMode::Immediate,
        PresentMode::Fifo => wgpu::PresentMode::Fifo,
        PresentMode::Mailbox => wgpu::PresentMode::Mailbox,
        PresentMode::AutoVsync => wgpu::PresentMode::AutoVsync,
        PresentMode::AutoNoVsync => wgpu::PresentMode::AutoNoVsync,
    }
}

pub(crate) fn present_mode_from_wgpu(m: wgpu::PresentMode) -> PresentMode {
    match m {
        wgpu::PresentMode::Immediate => PresentMode::Immediate,
        wgpu::PresentMode::Fifo => PresentMode::Fifo,
        wgpu::PresentMode::Mailbox => PresentMode::Mailbox,
        wgpu::PresentMode::AutoVsync => PresentMode::AutoVsync,
        wgpu::PresentMode::AutoNoVsync => PresentMode::AutoNoVsync,
        _ => PresentMode::Fifo,
    }
}

pub(crate) fn power_preference(p: PowerPreference) -> wgpu::PowerPreference {
    match p {
        PowerPreference::None => wgpu::PowerPreference::None,
        PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
        PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
    }
}

pub(crate) fn primitive_topology(t: PrimitiveTopology) -> wgpu::PrimitiveTopology {
    match t {
        PrimitiveTopology::PointList => wgpu::PrimitiveTopology::PointList,
        PrimitiveTopology::LineList => wgpu::PrimitiveTopology::LineList,
        PrimitiveTopology::LineStrip => wgpu::PrimitiveTopology::LineStrip,
        PrimitiveTopology::TriangleList => wgpu::PrimitiveTopology::TriangleList,
        PrimitiveTopology::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
    }
}

pub(crate) fn front_face(f: FrontFace) -> wgpu::FrontFace {
    match f {
        FrontFace::Ccw => wgpu::FrontFace::Ccw,
        FrontFace::Cw => wgpu::FrontFace::Cw,
    }
}

pub(crate) fn cull_mode(c: CullMode) -> Option<wgpu::Face> {
    match c {
        CullMode::None => None,
        CullMode::Front => Some(wgpu::Face::Front),
        CullMode::Back => Some(wgpu::Face::Back),
    }
}

pub(crate) fn polygon_mode(m: PolygonMode) -> wgpu::PolygonMode {
    match m {
        PolygonMode::Fill => wgpu::PolygonMode::Fill,
        PolygonMode::Line => wgpu::PolygonMode::Line,
        PolygonMode::Point => wgpu::PolygonMode::Point,
    }
}

pub(crate) fn compare_function(c: CompareFunction) -> wgpu::CompareFunction {
    match c {
        CompareFunction::Never => wgpu::CompareFunction::Never,
        CompareFunction::Less => wgpu::CompareFunction::Less,
        CompareFunction::Equal => wgpu::CompareFunction::Equal,
        CompareFunction::LessEqual => wgpu::CompareFunction::LessEqual,
        CompareFunction::Greater => wgpu::CompareFunction::Greater,
        CompareFunction::NotEqual => wgpu::CompareFunction::NotEqual,
        CompareFunction::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
        CompareFunction::Always => wgpu::CompareFunction::Always,
    }
}

pub(crate) fn blend_factor(f: BlendFactor) -> wgpu::BlendFactor {
    match f {
        BlendFactor::Zero => wgpu::BlendFactor::Zero,
        BlendFactor::One => wgpu::BlendFactor::One,
        BlendFactor::Src => wgpu::BlendFactor::Src,
        BlendFactor::OneMinusSrc => wgpu::BlendFactor::OneMinusSrc,
        BlendFactor::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
        BlendFactor::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
        BlendFactor::Dst => wgpu::BlendFactor::Dst,
        BlendFactor::OneMinusDst => wgpu::BlendFactor::OneMinusDst,
        BlendFactor::DstAlpha => wgpu::BlendFactor::DstAlpha,
        BlendFactor::OneMinusDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
        BlendFactor::SrcAlphaSaturated => wgpu::BlendFactor::SrcAlphaSaturated,
        BlendFactor::Constant => wgpu::BlendFactor::Constant,
        BlendFactor::OneMinusConstant => wgpu::BlendFactor::OneMinusConstant,
    }
}

pub(crate) fn blend_operation(o: BlendOperation) -> wgpu::BlendOperation {
    match o {
        BlendOperation::Add => wgpu::BlendOperation::Add,
        BlendOperation::Subtract => wgpu::BlendOperation::Subtract,
        BlendOperation::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
        BlendOperation::Min => wgpu::BlendOperation::Min,
        BlendOperation::Max => wgpu::BlendOperation::Max,
    }
}

pub(crate) fn blend_component(c: &BlendComponent) -> wgpu::BlendComponent {
    wgpu::BlendComponent {
        src_factor: blend_factor(c.src_factor),
        dst_factor: blend_factor(c.dst_factor),
        operation: blend_operation(c.operation),
    }
}

pub(crate) fn blend_state(s: &BlendState) -> wgpu::BlendState {
    wgpu::BlendState {
        color: blend_component(&s.color),
        alpha: blend_component(&s.alpha),
    }
}

pub(crate) fn index_format(f: IndexFormat) -> wgpu::IndexFormat {
    match f {
        IndexFormat::Uint16 => wgpu::IndexFormat::Uint16,
        IndexFormat::Uint32 => wgpu::IndexFormat::Uint32,
    }
}

pub(crate) fn vertex_format(f: VertexFormat) -> wgpu::VertexFormat {
    match f {
        VertexFormat::Uint8x2 => wgpu::VertexFormat::Uint8x2,
        VertexFormat::Uint8x4 => wgpu::VertexFormat::Uint8x4,
        VertexFormat::Sint8x2 => wgpu::VertexFormat::Sint8x2,
        VertexFormat::Sint8x4 => wgpu::VertexFormat::Sint8x4,
        VertexFormat::Unorm8x2 => wgpu::VertexFormat::Unorm8x2,
        VertexFormat::Unorm8x4 => wgpu::VertexFormat::Unorm8x4,
        VertexFormat::Snorm8x2 => wgpu::VertexFormat::Snorm8x2,
        VertexFormat::Snorm8x4 => wgpu::VertexFormat::Snorm8x4,
        VertexFormat::Uint16x2 => wgpu::VertexFormat::Uint16x2,
        VertexFormat::Uint16x4 => wgpu::VertexFormat::Uint16x4,
        VertexFormat::Sint16x2 => wgpu::VertexFormat::Sint16x2,
        VertexFormat::Sint16x4 => wgpu::VertexFormat::Sint16x4,
        VertexFormat::Unorm16x2 => wgpu::VertexFormat::Unorm16x2,
        VertexFormat::Unorm16x4 => wgpu::VertexFormat::Unorm16x4,
        VertexFormat::Snorm16x2 => wgpu::VertexFormat::Snorm16x2,
        VertexFormat::Snorm16x4 => wgpu::VertexFormat::Snorm16x4,
        VertexFormat::Float16x2 => wgpu::VertexFormat::Float16x2,
        VertexFormat::Float16x4 => wgpu::VertexFormat::Float16x4,
        VertexFormat::Float32 => wgpu::VertexFormat::Float32,
        VertexFormat::Float32x2 => wgpu::VertexFormat::Float32x2,
        VertexFormat::Float32x3 => wgpu::VertexFormat::Float32x3,
        VertexFormat::Float32x4 => wgpu::VertexFormat::Float32x4,
        VertexFormat::Uint32 => wgpu::VertexFormat::Uint32,
        VertexFormat::Uint32x2 => wgpu::VertexFormat::Uint32x2,
        VertexFormat::Uint32x3 => wgpu::VertexFormat::Uint32x3,
        VertexFormat::Uint32x4 => wgpu::VertexFormat::Uint32x4,
        VertexFormat::Sint32 => wgpu::VertexFormat::Sint32,
        VertexFormat::Sint32x2 => wgpu::VertexFormat::Sint32x2,
        VertexFormat::Sint32x3 => wgpu::VertexFormat::Sint32x3,
        VertexFormat::Sint32x4 => wgpu::VertexFormat::Sint32x4,
        _ => unreachable!("unknown vertex format"),
    }
}

pub(crate) fn vertex_step_mode(m: VertexStepMode) -> wgpu::VertexStepMode {
    match m {
        VertexStepMode::Vertex => wgpu::VertexStepMode::Vertex,
        VertexStepMode::Instance => wgpu::VertexStepMode::Instance,
    }
}

pub(crate) fn texture_dimension(d: TextureDimension) -> wgpu::TextureDimension {
    match d {
        TextureDimension::D1 => wgpu::TextureDimension::D1,
        TextureDimension::D2 => wgpu::TextureDimension::D2,
        TextureDimension::D3 => wgpu::TextureDimension::D3,
    }
}

pub(crate) fn texture_view_dimension(d: TextureViewDimension) -> wgpu::TextureViewDimension {
    match d {
        TextureViewDimension::D1 => wgpu::TextureViewDimension::D1,
        TextureViewDimension::D2 => wgpu::TextureViewDimension::D2,
        TextureViewDimension::D2Array => wgpu::TextureViewDimension::D2Array,
        TextureViewDimension::D3 => wgpu::TextureViewDimension::D3,
        TextureViewDimension::Cube => wgpu::TextureViewDimension::Cube,
        TextureViewDimension::CubeArray => wgpu::TextureViewDimension::CubeArray,
    }
}

pub(crate) fn filter_mode(m: FilterMode) -> wgpu::FilterMode {
    match m {
        FilterMode::Nearest => wgpu::FilterMode::Nearest,
        FilterMode::Linear => wgpu::FilterMode::Linear,
    }
}

pub(crate) fn address_mode(m: AddressMode) -> wgpu::AddressMode {
    match m {
        AddressMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        AddressMode::Repeat => wgpu::AddressMode::Repeat,
        AddressMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
    }
}

pub(crate) fn color_writes(w: ColorWrites) -> wgpu::ColorWrites {
    let mut result = wgpu::ColorWrites::empty();
    if w.contains(ColorWrites::RED) {
        result |= wgpu::ColorWrites::RED;
    }
    if w.contains(ColorWrites::GREEN) {
        result |= wgpu::ColorWrites::GREEN;
    }
    if w.contains(ColorWrites::BLUE) {
        result |= wgpu::ColorWrites::BLUE;
    }
    if w.contains(ColorWrites::ALPHA) {
        result |= wgpu::ColorWrites::ALPHA;
    }
    result
}

pub(crate) fn store_op(op: StoreOp) -> wgpu::StoreOp {
    match op {
        StoreOp::Store => wgpu::StoreOp::Store,
        StoreOp::Discard => wgpu::StoreOp::Discard,
    }
}
