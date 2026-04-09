//! Shared GPU enums and small types.
//!
//! These types mirror the underlying graphics API concepts but are
//! engine-owned, keeping the trait crate free of backend dependencies.

/// Texture format.
///
/// Covers the most common formats. Marked `#[non_exhaustive]` so new
/// formats can be added without a breaking change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextureFormat {
    // 8-bit per channel
    /// Single channel, 8-bit unsigned normalized.
    R8Unorm,
    /// Single channel, 8-bit signed normalized.
    R8Snorm,
    /// Single channel, 8-bit unsigned integer.
    R8Uint,
    /// Single channel, 8-bit signed integer.
    R8Sint,

    // 16-bit per channel
    /// Single channel, 16-bit unsigned integer.
    R16Uint,
    /// Single channel, 16-bit signed integer.
    R16Sint,
    /// Single channel, 16-bit float.
    R16Float,
    /// Two channels, 8-bit unsigned normalized.
    Rg8Unorm,
    /// Two channels, 8-bit signed normalized.
    Rg8Snorm,
    /// Two channels, 8-bit unsigned integer.
    Rg8Uint,
    /// Two channels, 8-bit signed integer.
    Rg8Sint,

    // 32-bit
    /// Single channel, 32-bit unsigned integer.
    R32Uint,
    /// Single channel, 32-bit signed integer.
    R32Sint,
    /// Single channel, 32-bit float.
    R32Float,
    /// Two channels, 16-bit unsigned integer.
    Rg16Uint,
    /// Two channels, 16-bit signed integer.
    Rg16Sint,
    /// Two channels, 16-bit float.
    Rg16Float,
    /// Four channels, 8-bit unsigned normalized (linear).
    Rgba8Unorm,
    /// Four channels, 8-bit unsigned normalized (sRGB).
    Rgba8UnormSrgb,
    /// Four channels, 8-bit signed normalized.
    Rgba8Snorm,
    /// Four channels, 8-bit unsigned integer.
    Rgba8Uint,
    /// Four channels, 8-bit signed integer.
    Rgba8Sint,
    /// Four channels BGRA, 8-bit unsigned normalized (linear).
    Bgra8Unorm,
    /// Four channels BGRA, 8-bit unsigned normalized (sRGB).
    Bgra8UnormSrgb,

    // 64-bit
    /// Two channels, 32-bit unsigned integer.
    Rg32Uint,
    /// Two channels, 32-bit signed integer.
    Rg32Sint,
    /// Two channels, 32-bit float.
    Rg32Float,
    /// Four channels, 16-bit unsigned integer.
    Rgba16Uint,
    /// Four channels, 16-bit signed integer.
    Rgba16Sint,
    /// Four channels, 16-bit float.
    Rgba16Float,

    // 128-bit
    /// Four channels, 32-bit unsigned integer.
    Rgba32Uint,
    /// Four channels, 32-bit signed integer.
    Rgba32Sint,
    /// Four channels, 32-bit float.
    Rgba32Float,

    // Depth / stencil
    /// 16-bit unsigned normalized depth.
    Depth16Unorm,
    /// 24-bit depth (plus padding).
    Depth24Plus,
    /// 24-bit depth + 8-bit stencil.
    Depth24PlusStencil8,
    /// 32-bit float depth.
    Depth32Float,
    /// 32-bit float depth + 8-bit stencil.
    Depth32FloatStencil8,
}

/// How primitives are assembled from vertices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum PrimitiveTopology {
    /// Each vertex is a separate point.
    PointList,
    /// Every two vertices form a line segment.
    LineList,
    /// Vertices form a connected line strip.
    LineStrip,
    /// Every three vertices form a triangle.
    #[default]
    TriangleList,
    /// Vertices form a connected triangle strip.
    TriangleStrip,
}

/// Winding order for front-face determination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum FrontFace {
    /// Counter-clockwise winding.
    #[default]
    Ccw,
    /// Clockwise winding.
    Cw,
}

/// Which triangle faces to cull.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum CullMode {
    /// No culling.
    #[default]
    None,
    /// Cull front-facing triangles.
    Front,
    /// Cull back-facing triangles.
    Back,
}

/// Polygon rasterization mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum PolygonMode {
    /// Fill the polygon interior.
    #[default]
    Fill,
    /// Draw only edges (wireframe).
    Line,
    /// Draw only vertices.
    Point,
}

/// Comparison function for depth/stencil tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompareFunction {
    /// Never passes.
    Never,
    /// Passes if new < existing.
    Less,
    /// Passes if new == existing.
    Equal,
    /// Passes if new <= existing.
    LessEqual,
    /// Passes if new > existing.
    Greater,
    /// Passes if new != existing.
    NotEqual,
    /// Passes if new >= existing.
    GreaterEqual,
    /// Always passes.
    Always,
}

/// Blend factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendFactor {
    /// 0
    Zero,
    /// 1
    One,
    /// Source color/alpha.
    Src,
    /// 1 - source color/alpha.
    OneMinusSrc,
    /// Source alpha.
    SrcAlpha,
    /// 1 - source alpha.
    OneMinusSrcAlpha,
    /// Destination color/alpha.
    Dst,
    /// 1 - destination color/alpha.
    OneMinusDst,
    /// Destination alpha.
    DstAlpha,
    /// 1 - destination alpha.
    OneMinusDstAlpha,
    /// min(source alpha, 1 - destination alpha).
    SrcAlphaSaturated,
    /// Constant blend color.
    Constant,
    /// 1 - constant blend color.
    OneMinusConstant,
}

/// Blend operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendOperation {
    /// src + dst
    Add,
    /// src - dst
    Subtract,
    /// dst - src
    ReverseSubtract,
    /// min(src, dst)
    Min,
    /// max(src, dst)
    Max,
}

/// A single blend component (color or alpha).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlendComponent {
    /// Source factor.
    pub src_factor: BlendFactor,
    /// Destination factor.
    pub dst_factor: BlendFactor,
    /// Blend operation.
    pub operation: BlendOperation,
}

/// Complete blend state for a color target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlendState {
    /// Color blend component.
    pub color: BlendComponent,
    /// Alpha blend component.
    pub alpha: BlendComponent,
}

impl BlendState {
    /// Standard alpha blending: `src.rgb * src.a + dst.rgb * (1 - src.a)`.
    pub const ALPHA_BLENDING: Self = Self {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    };

    /// Premultiplied alpha blending: `src.rgb + dst.rgb * (1 - src.a)`.
    pub const PREMULTIPLIED_ALPHA_BLENDING: Self = Self {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    };

    /// Replace destination with source (no blending).
    pub const REPLACE: Self = Self {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
    };
}

/// Index buffer element format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IndexFormat {
    /// 16-bit unsigned indices (max 65535 vertices).
    Uint16,
    /// 32-bit unsigned indices.
    Uint32,
}

/// Vertex attribute format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VertexFormat {
    /// Two 8-bit unsigned integers.
    Uint8x2,
    /// Four 8-bit unsigned integers.
    Uint8x4,
    /// Two 8-bit signed integers.
    Sint8x2,
    /// Four 8-bit signed integers.
    Sint8x4,
    /// Two 8-bit unsigned normalized values.
    Unorm8x2,
    /// Four 8-bit unsigned normalized values.
    Unorm8x4,
    /// Two 8-bit signed normalized values.
    Snorm8x2,
    /// Four 8-bit signed normalized values.
    Snorm8x4,
    /// Two 16-bit unsigned integers.
    Uint16x2,
    /// Four 16-bit unsigned integers.
    Uint16x4,
    /// Two 16-bit signed integers.
    Sint16x2,
    /// Four 16-bit signed integers.
    Sint16x4,
    /// Two 16-bit unsigned normalized values.
    Unorm16x2,
    /// Four 16-bit unsigned normalized values.
    Unorm16x4,
    /// Two 16-bit signed normalized values.
    Snorm16x2,
    /// Four 16-bit signed normalized values.
    Snorm16x4,
    /// Two 16-bit floats.
    Float16x2,
    /// Four 16-bit floats.
    Float16x4,
    /// One 32-bit float.
    Float32,
    /// Two 32-bit floats.
    Float32x2,
    /// Three 32-bit floats.
    Float32x3,
    /// Four 32-bit floats.
    Float32x4,
    /// One 32-bit unsigned integer.
    Uint32,
    /// Two 32-bit unsigned integers.
    Uint32x2,
    /// Three 32-bit unsigned integers.
    Uint32x3,
    /// Four 32-bit unsigned integers.
    Uint32x4,
    /// One 32-bit signed integer.
    Sint32,
    /// Two 32-bit signed integers.
    Sint32x2,
    /// Three 32-bit signed integers.
    Sint32x3,
    /// Four 32-bit signed integers.
    Sint32x4,
}

/// Whether vertex data advances per-vertex or per-instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum VertexStepMode {
    /// Advance per vertex.
    #[default]
    Vertex,
    /// Advance per instance.
    Instance,
}

/// Texture dimensionality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureDimension {
    /// 1D texture.
    D1,
    /// 2D texture.
    D2,
    /// 3D texture.
    D3,
}

/// Texture view dimensionality (can differ from the texture's dimension).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureViewDimension {
    /// 1D view.
    D1,
    /// 2D view.
    D2,
    /// 2D array view.
    D2Array,
    /// 3D view.
    D3,
    /// Cube map view.
    Cube,
    /// Cube map array view.
    CubeArray,
}

/// Texture sampling filter mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum FilterMode {
    /// Nearest-neighbor sampling.
    #[default]
    Nearest,
    /// Linear (bilinear) sampling.
    Linear,
}

/// Texture coordinate addressing (wrap) mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum AddressMode {
    /// Clamp to the edge texel.
    #[default]
    ClampToEdge,
    /// Tile the texture.
    Repeat,
    /// Tile the texture, mirroring on each repeat.
    MirrorRepeat,
}

/// Color channel write mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColorWrites(u8);

impl ColorWrites {
    /// Write to the red channel.
    pub const RED: Self = Self(1);
    /// Write to the green channel.
    pub const GREEN: Self = Self(2);
    /// Write to the blue channel.
    pub const BLUE: Self = Self(4);
    /// Write to the alpha channel.
    pub const ALPHA: Self = Self(8);
    /// Write to all channels.
    pub const ALL: Self = Self(0xF);

    /// Returns `true` if all bits in `other` are set in `self`.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for ColorWrites {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// What to do at the start of a render pass for an attachment.
#[derive(Clone, Copy, Debug)]
pub enum LoadOp<V> {
    /// Clear the attachment to the given value.
    Clear(V),
    /// Preserve existing contents.
    Load,
}

/// What to do at the end of a render pass for an attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StoreOp {
    /// Write results back to the attachment.
    Store,
    /// Discard results (for transient attachments).
    Discard,
}

/// Presentation mode for surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PresentMode {
    /// No vsync, may tear. Lowest latency.
    Immediate,
    /// Vsync with frame queuing (traditional double-buffered).
    Fifo,
    /// Vsync but drops stale frames (lower latency than Fifo).
    Mailbox,
    /// Backend chooses best vsync mode.
    AutoVsync,
    /// Backend chooses best no-vsync mode.
    AutoNoVsync,
}

/// Power preference hint for GPU adapter selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum PowerPreference {
    /// No preference.
    #[default]
    None,
    /// Prefer low power (integrated GPU).
    LowPower,
    /// Prefer high performance (discrete GPU).
    HighPerformance,
}
