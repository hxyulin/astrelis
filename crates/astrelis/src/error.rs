use std::{error, fmt};

/// An initialization, mesh validation, target, or rendering failure.
#[derive(Debug)]
pub enum Error {
    /// Attachments are absent or have inconsistent dimensions, samples, or formats.
    InvalidAttachments,
    /// A built-in color renderer requires exactly one color output at slot zero.
    ExpectedSingleColor,
    /// wgpu rejected a prepared shader or pipeline.
    Validation(wgpu::Error),
    /// Geometry layout, buffer sizes, or draw ranges are invalid.
    InvalidGeometry,
    /// Texture dimensions must be nonzero and fit the device's 2D limit.
    InvalidTextureSize {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
        /// Maximum dimension supported by the device.
        max: u32,
    },
    /// The texture helper cannot allocate this format with the enabled device features.
    UnsupportedTextureFormat {
        /// Requested color format.
        format: wgpu::TextureFormat,
    },
    /// Texture usages are unavailable or omit a required upload usage.
    InvalidTextureUsage {
        /// Texture format.
        format: wgpu::TextureFormat,
        /// Selected usages.
        usage: wgpu::TextureUsages,
    },
    /// A texture upload rectangle is empty, overflows, or exceeds storage bounds.
    InvalidTextureRegion,
    /// A tightly packed texture upload has the wrong byte count.
    InvalidTextureData {
        /// Required byte count.
        expected: u64,
        /// Supplied byte count.
        actual: usize,
    },
    /// The source is not single-sampled float color with the required sampling support.
    InvalidTextureSource {
        /// Underlying source format.
        format: wgpu::TextureFormat,
    },
    /// A prepared texture rectangle/tint is nonfinite, has negative extents, or invalid opacity.
    InvalidTextureDraw,
    /// Solid shape geometry, color, transform, or transformed bounds are invalid.
    InvalidShapeDraw,
    /// Line endpoints, width, color, transform, or transformed bounds are invalid.
    InvalidLineDraw,
    /// Sampling would read from an active color or resolve attachment.
    TextureFeedback,
    /// A window or canvas could not be turned into a wgpu surface.
    CreateSurface(wgpu::CreateSurfaceError),
    /// No suitable GPU adapter could be selected.
    Adapter(wgpu::RequestAdapterError),
    /// A GPU device could not be created.
    Device(wgpu::RequestDeviceError),
    /// The context's adapter cannot present to this surface.
    UnsupportedSurface,
    /// This format cannot be a color attachment with the device's enabled features.
    UnsupportedColorFormat {
        /// Requested color format.
        format: wgpu::TextureFormat,
    },
    /// Framebuffer usages omit rendering, request transient storage, or exceed format support.
    InvalidFramebufferUsage {
        /// Requested color format.
        format: wgpu::TextureFormat,
        /// Requested texture usages.
        usage: wgpu::TextureUsages,
    },
    /// The colored mesh shader requires a floating-point color output and alpha blending.
    UnsupportedMeshFormat {
        /// Pass format incompatible with the built-in mesh renderer.
        format: wgpu::TextureFormat,
    },
    /// A material requests blending on a color format that does not support it.
    UnsupportedMaterialFormat {
        /// Pass format incompatible with the material's blend settings.
        format: wgpu::TextureFormat,
    },
    /// The format is not an available depth/stencil attachment on this device.
    UnsupportedDepthStencilFormat {
        /// Requested attachment format.
        format: wgpu::TextureFormat,
    },
    /// Depth/stencil usages omit rendering, request transient storage, or exceed support.
    InvalidDepthStencilUsage {
        /// Requested attachment format.
        format: wgpu::TextureFormat,
        /// Requested texture usages.
        usage: wgpu::TextureUsages,
    },
    /// A depth operation was selected without an attachment containing depth.
    MissingDepthAttachment,
    /// A stencil operation was selected without an attachment containing stencil.
    MissingStencilAttachment,
    /// Depth clear values must be finite and in zero to one.
    InvalidClearDepth,
    /// A depth load or read-only pass needs initialized, stored depth contents.
    UninitializedDepth,
    /// A stencil load or read-only pass needs initialized, stored stencil contents.
    UninitializedStencil,
    /// The material's explicit depth/stencil format does not match the pass.
    DepthStencilMismatch {
        /// Depth/stencil format required by the material.
        material: wgpu::TextureFormat,
        /// Depth/stencil format present in the pass, if any.
        pass: Option<wgpu::TextureFormat>,
    },
    /// The material writes depth in a pass whose depth aspect is read-only.
    ReadOnlyDepth,
    /// The material writes stencil in a pass whose stencil aspect is read-only.
    ReadOnlyStencil,
    /// A zero-sized target has no allocated attachment.
    TargetSuspended,
    /// A framebuffer load requires a submitted clear or an earlier clear in this recording.
    UninitializedFramebuffer,
    /// The selected color format cannot render and resolve this sample count on the device.
    UnsupportedSampleCount {
        /// Color attachment format.
        format: wgpu::TextureFormat,
        /// Requested number of samples per pixel; one disables MSAA.
        count: u32,
    },
    /// Target dimensions exceed the device's two-dimensional texture limit.
    InvalidTargetSize {
        /// Requested physical width.
        width: u32,
        /// Requested physical height.
        height: u32,
        /// Maximum dimension supported by the device.
        max: u32,
    },
    /// A mesh must have at least one vertex.
    EmptyVertices,
    /// Triangle-list indices must be nonempty and a multiple of three.
    InvalidIndexCount {
        /// Number of supplied indices.
        count: usize,
    },
    /// A vertex has a nonfinite position or color component.
    InvalidVertex {
        /// Index of the invalid vertex.
        index: usize,
    },
    /// An index references a vertex outside the supplied vertex array.
    IndexOutOfBounds {
        /// Invalid vertex index.
        index: u32,
        /// Number of supplied vertices.
        vertex_count: usize,
    },
    /// Mesh data exceeds the device's buffer limit or the index-count limit.
    MeshTooLarge,
    /// A renderer, mesh, or target belongs to a different wgpu device.
    DeviceMismatch,
    /// The clear color contains a nonfinite component.
    InvalidClearColor,
    /// Viewport values are nonfinite or exceed device bounds or depth constraints.
    InvalidViewport,
    /// The scissor rectangle does not fit the pass attachment.
    InvalidScissorRect,
    /// A first surface pass tried to load contents, or finish had no surface clear pass.
    UninitializedFrame,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAttachments => f.write_str("pass attachments must have compatible formats, dimensions, and samples"),
            Self::ExpectedSingleColor => f.write_str("this renderer requires a single color output at slot zero"),
            Self::Validation(error) => write!(f, "GPU validation failed: {error}"),
            Self::InvalidGeometry => f.write_str("invalid geometry layout, buffer size, or draw range"),
            Self::InvalidTextureSize { width, height, max } => write!(f, "texture size {width}x{height} must be nonzero and fit {max}"),
            Self::UnsupportedTextureFormat { format } => write!(f, "{format:?} is not a supported uncompressed color texture format"),
            Self::InvalidTextureUsage { format, usage } => write!(f, "invalid texture usage {usage:?} for {format:?}"),
            Self::InvalidTextureRegion => f.write_str("texture upload region must be nonempty and fit the texture"),
            Self::InvalidTextureData { expected, actual } => write!(f, "texture upload requires {expected} bytes, got {actual}"),
            Self::InvalidTextureSource { format } => write!(f, "{format:?} requires single-sampled float color storage with compatible sampling usages/filtering"),
            Self::InvalidTextureDraw => f.write_str("texture rectangles/tint must be finite, extents nonnegative, and opacity in 0..=1"),
            Self::InvalidShapeDraw => f.write_str("shape geometry/color/transform must be finite, extents/radius nonnegative, alpha in 0..=1"),
            Self::InvalidLineDraw => f.write_str("line geometry/color/transform must be finite, width nonnegative, alpha in 0..=1"),
            Self::TextureFeedback => f.write_str("cannot sample an active color or resolve attachment"),
            Self::CreateSurface(error) => write!(f, "could not create a GPU surface: {error}"),
            Self::Adapter(error) => write!(f, "could not request a GPU adapter: {error}"),
            Self::Device(error) => write!(f, "could not request a GPU device: {error}"),
            Self::UnsupportedSurface => f.write_str("the adapter cannot present to this surface"),
            Self::UnsupportedColorFormat { format } => write!(f, "{format:?} cannot be a color attachment on this device"),
            Self::InvalidFramebufferUsage { format, usage } => write!(f, "invalid framebuffer usage {usage:?} for {format:?}"),
            Self::UnsupportedMeshFormat { format } => write!(f, "the mesh renderer requires a blendable floating-point output, got {format:?}"),
            Self::UnsupportedMaterialFormat { format } => write!(f, "material blending is unsupported for {format:?}"),
            Self::UnsupportedDepthStencilFormat { format } => write!(f, "{format:?} is not an available depth/stencil attachment"),
            Self::InvalidDepthStencilUsage { format, usage } => write!(f, "invalid depth/stencil usage {usage:?} for {format:?}"),
            Self::MissingDepthAttachment => f.write_str("the pass has no depth aspect"),
            Self::MissingStencilAttachment => f.write_str("the pass has no stencil aspect"),
            Self::InvalidClearDepth => f.write_str("depth clear must be finite and in 0..=1"),
            Self::UninitializedDepth => f.write_str("depth must be cleared and stored before loading or read-only use"),
            Self::UninitializedStencil => f.write_str("stencil must be cleared and stored before loading or read-only use"),
            Self::DepthStencilMismatch { material, pass } => write!(f, "material depth/stencil format {material:?} does not match pass {pass:?}"),
            Self::ReadOnlyDepth => f.write_str("the material writes to read-only depth"),
            Self::ReadOnlyStencil => f.write_str("the material writes to read-only stencil"),
            Self::TargetSuspended => f.write_str("a zero-sized target has no attachment"),
            Self::UninitializedFramebuffer => f.write_str("a framebuffer must have a submitted clear or an earlier clear in this recording before loading"),
            Self::UnsupportedSampleCount { format, count } => {
                write!(f, "sample count {count} is unsupported for {format:?} on this device")
            }
            Self::InvalidTargetSize { width, height, max } => {
                write!(
                    f,
                    "target size {width}x{height} exceeds the device limit {max}"
                )
            }
            Self::EmptyVertices => f.write_str("a mesh must have at least one vertex"),
            Self::InvalidIndexCount { count } => {
                write!(
                    f,
                    "expected a nonempty triangle index list, got {count} indices"
                )
            }
            Self::InvalidVertex { index } => write!(f, "vertex {index} contains nonfinite data"),
            Self::IndexOutOfBounds {
                index,
                vertex_count,
            } => {
                write!(
                    f,
                    "index {index} exceeds the mesh's {vertex_count} vertices"
                )
            }
            Self::MeshTooLarge => f.write_str("mesh data exceeds device or draw-count limits"),
            Self::DeviceMismatch => f.write_str("resources belong to different GPU devices"),
            Self::InvalidClearColor => f.write_str("clear color must contain finite components"),
            Self::InvalidViewport => f.write_str("viewport must be finite and within device bounds with an ordered depth range in 0..=1"),
            Self::InvalidScissorRect => f.write_str("scissor rectangle must fit the pass attachment"),
            Self::UninitializedFrame => {
                f.write_str("a frame must start with a clear pass before presentation")
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::CreateSurface(error) => Some(error),
            Self::Adapter(error) => Some(error),
            Self::Device(error) => Some(error),
            _ => None,
        }
    }
}
