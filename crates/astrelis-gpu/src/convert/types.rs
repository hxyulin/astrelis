//! Core type conversions between `astrelis_gpu::types` and `wgpu`.

use crate::types::*;

/// Converts an [`astrelis_gpu::types::TextureFormat`] to a [`wgpu::TextureFormat`].
pub fn texture_format(f: TextureFormat) -> wgpu::TextureFormat {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── TextureFormat round-trip ──────────────────────────────────────

    /// Every format that the engine wraps must survive a round-trip
    /// through `texture_format` -> `texture_format_from_wgpu`.
    #[test]
    fn texture_format_round_trip() {
        let formats = [
            TextureFormat::R8Unorm,
            TextureFormat::R8Snorm,
            TextureFormat::R8Uint,
            TextureFormat::R8Sint,
            TextureFormat::R16Uint,
            TextureFormat::R16Sint,
            TextureFormat::R16Float,
            TextureFormat::Rg8Unorm,
            TextureFormat::Rg8Snorm,
            TextureFormat::Rg8Uint,
            TextureFormat::Rg8Sint,
            TextureFormat::R32Uint,
            TextureFormat::R32Sint,
            TextureFormat::R32Float,
            TextureFormat::Rg16Uint,
            TextureFormat::Rg16Sint,
            TextureFormat::Rg16Float,
            TextureFormat::Rgba8Unorm,
            TextureFormat::Rgba8UnormSrgb,
            TextureFormat::Rgba8Snorm,
            TextureFormat::Rgba8Uint,
            TextureFormat::Rgba8Sint,
            TextureFormat::Bgra8Unorm,
            TextureFormat::Bgra8UnormSrgb,
            TextureFormat::Rg32Uint,
            TextureFormat::Rg32Sint,
            TextureFormat::Rg32Float,
            TextureFormat::Rgba16Uint,
            TextureFormat::Rgba16Sint,
            TextureFormat::Rgba16Float,
            TextureFormat::Rgba32Uint,
            TextureFormat::Rgba32Sint,
            TextureFormat::Rgba32Float,
            TextureFormat::Depth16Unorm,
            TextureFormat::Depth24Plus,
            TextureFormat::Depth24PlusStencil8,
            TextureFormat::Depth32Float,
            TextureFormat::Depth32FloatStencil8,
        ];

        for fmt in formats {
            let wgpu_fmt = texture_format(fmt);
            let back = texture_format_from_wgpu(wgpu_fmt);
            assert_eq!(fmt, back, "round-trip failed for {fmt:?}");
        }
    }

    /// Spot-check a few common texture formats individually.
    #[test]
    fn texture_format_common_formats() {
        assert_eq!(
            texture_format(TextureFormat::Rgba8Unorm),
            wgpu::TextureFormat::Rgba8Unorm,
        );
        assert_eq!(
            texture_format(TextureFormat::Bgra8UnormSrgb),
            wgpu::TextureFormat::Bgra8UnormSrgb,
        );
        assert_eq!(
            texture_format(TextureFormat::Depth32Float),
            wgpu::TextureFormat::Depth32Float,
        );
    }

    /// `texture_format_from_wgpu` falls back to `Rgba8Unorm` for wgpu
    /// formats the engine does not wrap.
    #[test]
    fn texture_format_from_wgpu_fallback() {
        // Bc1RgbaUnorm is a compressed format we intentionally don't wrap.
        let result = texture_format_from_wgpu(wgpu::TextureFormat::Bc1RgbaUnorm);
        assert_eq!(result, TextureFormat::Rgba8Unorm);
    }

    // ── PresentMode round-trip ───────────────────────────────────────

    #[test]
    fn present_mode_round_trip() {
        let modes = [
            PresentMode::Immediate,
            PresentMode::Fifo,
            PresentMode::Mailbox,
            PresentMode::AutoVsync,
            PresentMode::AutoNoVsync,
        ];

        for mode in modes {
            let wgpu_mode = present_mode(mode);
            let back = present_mode_from_wgpu(wgpu_mode);
            assert_eq!(mode, back, "round-trip failed for {mode:?}");
        }
    }

    #[test]
    fn present_mode_from_wgpu_fallback() {
        // FifoRelaxed is not wrapped; should fall back to Fifo.
        let result = present_mode_from_wgpu(wgpu::PresentMode::FifoRelaxed);
        assert_eq!(result, PresentMode::Fifo);
    }

    // ── PowerPreference ──────────────────────────────────────────────

    #[test]
    fn power_preference_conversion() {
        assert_eq!(
            power_preference(PowerPreference::None),
            wgpu::PowerPreference::None,
        );
        assert_eq!(
            power_preference(PowerPreference::LowPower),
            wgpu::PowerPreference::LowPower,
        );
        assert_eq!(
            power_preference(PowerPreference::HighPerformance),
            wgpu::PowerPreference::HighPerformance,
        );
    }

    // ── PrimitiveTopology ────────────────────────────────────────────

    #[test]
    fn primitive_topology_conversion() {
        assert_eq!(
            primitive_topology(PrimitiveTopology::PointList),
            wgpu::PrimitiveTopology::PointList,
        );
        assert_eq!(
            primitive_topology(PrimitiveTopology::LineList),
            wgpu::PrimitiveTopology::LineList,
        );
        assert_eq!(
            primitive_topology(PrimitiveTopology::LineStrip),
            wgpu::PrimitiveTopology::LineStrip,
        );
        assert_eq!(
            primitive_topology(PrimitiveTopology::TriangleList),
            wgpu::PrimitiveTopology::TriangleList,
        );
        assert_eq!(
            primitive_topology(PrimitiveTopology::TriangleStrip),
            wgpu::PrimitiveTopology::TriangleStrip,
        );
    }

    // ── FrontFace / CullMode / PolygonMode ───────────────────────────

    #[test]
    fn front_face_conversion() {
        assert_eq!(front_face(FrontFace::Ccw), wgpu::FrontFace::Ccw);
        assert_eq!(front_face(FrontFace::Cw), wgpu::FrontFace::Cw);
    }

    #[test]
    fn cull_mode_conversion() {
        assert_eq!(cull_mode(CullMode::None), None);
        assert_eq!(cull_mode(CullMode::Front), Some(wgpu::Face::Front));
        assert_eq!(cull_mode(CullMode::Back), Some(wgpu::Face::Back));
    }

    #[test]
    fn polygon_mode_conversion() {
        assert_eq!(polygon_mode(PolygonMode::Fill), wgpu::PolygonMode::Fill);
        assert_eq!(polygon_mode(PolygonMode::Line), wgpu::PolygonMode::Line);
        assert_eq!(polygon_mode(PolygonMode::Point), wgpu::PolygonMode::Point);
    }

    // ── CompareFunction ──────────────────────────────────────────────

    #[test]
    fn compare_function_conversion() {
        let pairs = [
            (CompareFunction::Never, wgpu::CompareFunction::Never),
            (CompareFunction::Less, wgpu::CompareFunction::Less),
            (CompareFunction::Equal, wgpu::CompareFunction::Equal),
            (CompareFunction::LessEqual, wgpu::CompareFunction::LessEqual),
            (CompareFunction::Greater, wgpu::CompareFunction::Greater),
            (CompareFunction::NotEqual, wgpu::CompareFunction::NotEqual),
            (
                CompareFunction::GreaterEqual,
                wgpu::CompareFunction::GreaterEqual,
            ),
            (CompareFunction::Always, wgpu::CompareFunction::Always),
        ];
        for (engine, expected) in pairs {
            assert_eq!(
                compare_function(engine),
                expected,
                "mismatch for {engine:?}"
            );
        }
    }

    // ── BlendFactor / BlendOperation ─────────────────────────────────

    #[test]
    fn blend_factor_conversion() {
        let pairs = [
            (BlendFactor::Zero, wgpu::BlendFactor::Zero),
            (BlendFactor::One, wgpu::BlendFactor::One),
            (BlendFactor::Src, wgpu::BlendFactor::Src),
            (BlendFactor::OneMinusSrc, wgpu::BlendFactor::OneMinusSrc),
            (BlendFactor::SrcAlpha, wgpu::BlendFactor::SrcAlpha),
            (
                BlendFactor::OneMinusSrcAlpha,
                wgpu::BlendFactor::OneMinusSrcAlpha,
            ),
            (BlendFactor::Dst, wgpu::BlendFactor::Dst),
            (BlendFactor::OneMinusDst, wgpu::BlendFactor::OneMinusDst),
            (BlendFactor::DstAlpha, wgpu::BlendFactor::DstAlpha),
            (
                BlendFactor::OneMinusDstAlpha,
                wgpu::BlendFactor::OneMinusDstAlpha,
            ),
            (
                BlendFactor::SrcAlphaSaturated,
                wgpu::BlendFactor::SrcAlphaSaturated,
            ),
            (BlendFactor::Constant, wgpu::BlendFactor::Constant),
            (
                BlendFactor::OneMinusConstant,
                wgpu::BlendFactor::OneMinusConstant,
            ),
        ];
        for (engine, expected) in pairs {
            assert_eq!(
                blend_factor(engine),
                expected,
                "mismatch for {engine:?}"
            );
        }
    }

    #[test]
    fn blend_operation_conversion() {
        let pairs = [
            (BlendOperation::Add, wgpu::BlendOperation::Add),
            (BlendOperation::Subtract, wgpu::BlendOperation::Subtract),
            (
                BlendOperation::ReverseSubtract,
                wgpu::BlendOperation::ReverseSubtract,
            ),
            (BlendOperation::Min, wgpu::BlendOperation::Min),
            (BlendOperation::Max, wgpu::BlendOperation::Max),
        ];
        for (engine, expected) in pairs {
            assert_eq!(
                blend_operation(engine),
                expected,
                "mismatch for {engine:?}"
            );
        }
    }

    // ── BlendComponent / BlendState ──────────────────────────────────

    #[test]
    fn blend_component_conversion() {
        let comp = BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        };
        let wgpu_comp = blend_component(&comp);
        assert_eq!(wgpu_comp.src_factor, wgpu::BlendFactor::SrcAlpha);
        assert_eq!(wgpu_comp.dst_factor, wgpu::BlendFactor::OneMinusSrcAlpha);
        assert_eq!(wgpu_comp.operation, wgpu::BlendOperation::Add);
    }

    #[test]
    fn blend_state_conversion() {
        let state = BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::Zero,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
        };
        let wgpu_state = blend_state(&state);
        assert_eq!(wgpu_state.color.src_factor, wgpu::BlendFactor::One);
        assert_eq!(wgpu_state.color.dst_factor, wgpu::BlendFactor::Zero);
        assert_eq!(wgpu_state.alpha.src_factor, wgpu::BlendFactor::SrcAlpha);
        assert_eq!(
            wgpu_state.alpha.dst_factor,
            wgpu::BlendFactor::OneMinusSrcAlpha
        );
    }

    // ── IndexFormat / VertexStepMode ─────────────────────────────────

    #[test]
    fn index_format_conversion() {
        assert_eq!(
            index_format(IndexFormat::Uint16),
            wgpu::IndexFormat::Uint16
        );
        assert_eq!(
            index_format(IndexFormat::Uint32),
            wgpu::IndexFormat::Uint32
        );
    }

    #[test]
    fn vertex_step_mode_conversion() {
        assert_eq!(
            vertex_step_mode(VertexStepMode::Vertex),
            wgpu::VertexStepMode::Vertex,
        );
        assert_eq!(
            vertex_step_mode(VertexStepMode::Instance),
            wgpu::VertexStepMode::Instance,
        );
    }

    // ── VertexFormat (spot-check representative formats) ─────────────

    #[test]
    fn vertex_format_conversion() {
        let pairs = [
            (VertexFormat::Float32, wgpu::VertexFormat::Float32),
            (VertexFormat::Float32x2, wgpu::VertexFormat::Float32x2),
            (VertexFormat::Float32x3, wgpu::VertexFormat::Float32x3),
            (VertexFormat::Float32x4, wgpu::VertexFormat::Float32x4),
            (VertexFormat::Uint32, wgpu::VertexFormat::Uint32),
            (VertexFormat::Uint8x4, wgpu::VertexFormat::Uint8x4),
            (VertexFormat::Sint32x4, wgpu::VertexFormat::Sint32x4),
            (VertexFormat::Unorm8x4, wgpu::VertexFormat::Unorm8x4),
            (VertexFormat::Float16x4, wgpu::VertexFormat::Float16x4),
        ];
        for (engine, expected) in pairs {
            assert_eq!(
                vertex_format(engine),
                expected,
                "mismatch for {engine:?}"
            );
        }
    }

    // ── TextureDimension / TextureViewDimension ──────────────────────

    #[test]
    fn texture_dimension_conversion() {
        assert_eq!(
            texture_dimension(TextureDimension::D1),
            wgpu::TextureDimension::D1,
        );
        assert_eq!(
            texture_dimension(TextureDimension::D2),
            wgpu::TextureDimension::D2,
        );
        assert_eq!(
            texture_dimension(TextureDimension::D3),
            wgpu::TextureDimension::D3,
        );
    }

    #[test]
    fn texture_view_dimension_conversion() {
        assert_eq!(
            texture_view_dimension(TextureViewDimension::D1),
            wgpu::TextureViewDimension::D1,
        );
        assert_eq!(
            texture_view_dimension(TextureViewDimension::D2),
            wgpu::TextureViewDimension::D2,
        );
        assert_eq!(
            texture_view_dimension(TextureViewDimension::D2Array),
            wgpu::TextureViewDimension::D2Array,
        );
        assert_eq!(
            texture_view_dimension(TextureViewDimension::Cube),
            wgpu::TextureViewDimension::Cube,
        );
        assert_eq!(
            texture_view_dimension(TextureViewDimension::CubeArray),
            wgpu::TextureViewDimension::CubeArray,
        );
    }

    // ── FilterMode / AddressMode ─────────────────────────────────────

    #[test]
    fn filter_mode_conversion() {
        assert_eq!(
            filter_mode(FilterMode::Nearest),
            wgpu::FilterMode::Nearest
        );
        assert_eq!(filter_mode(FilterMode::Linear), wgpu::FilterMode::Linear);
    }

    #[test]
    fn address_mode_conversion() {
        assert_eq!(
            address_mode(AddressMode::ClampToEdge),
            wgpu::AddressMode::ClampToEdge,
        );
        assert_eq!(
            address_mode(AddressMode::Repeat),
            wgpu::AddressMode::Repeat,
        );
        assert_eq!(
            address_mode(AddressMode::MirrorRepeat),
            wgpu::AddressMode::MirrorRepeat,
        );
    }

    // ── ColorWrites ──────────────────────────────────────────────────

    #[test]
    fn color_writes_all() {
        let all = ColorWrites::ALL;
        let result = color_writes(all);
        assert_eq!(result, wgpu::ColorWrites::ALL);
    }

    #[test]
    fn color_writes_individual_channels() {
        assert_eq!(color_writes(ColorWrites::RED), wgpu::ColorWrites::RED);
        assert_eq!(color_writes(ColorWrites::GREEN), wgpu::ColorWrites::GREEN);
        assert_eq!(color_writes(ColorWrites::BLUE), wgpu::ColorWrites::BLUE);
        assert_eq!(color_writes(ColorWrites::ALPHA), wgpu::ColorWrites::ALPHA);
    }

    #[test]
    fn color_writes_combination() {
        let rg = ColorWrites::RED | ColorWrites::GREEN;
        let result = color_writes(rg);
        assert!(result.contains(wgpu::ColorWrites::RED));
        assert!(result.contains(wgpu::ColorWrites::GREEN));
        assert!(!result.contains(wgpu::ColorWrites::BLUE));
        assert!(!result.contains(wgpu::ColorWrites::ALPHA));
    }

    // ── StoreOp ──────────────────────────────────────────────────────

    #[test]
    fn store_op_conversion() {
        assert_eq!(store_op(StoreOp::Store), wgpu::StoreOp::Store);
        assert_eq!(store_op(StoreOp::Discard), wgpu::StoreOp::Discard);
    }
}
