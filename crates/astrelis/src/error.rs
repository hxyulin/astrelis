use std::{error, fmt};

/// An initialization, mesh validation, target, or rendering failure.
#[derive(Debug)]
pub enum Error {
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
    /// A zero-sized framebuffer has no color attachment.
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
            Self::CreateSurface(error) => write!(f, "could not create a GPU surface: {error}"),
            Self::Adapter(error) => write!(f, "could not request a GPU adapter: {error}"),
            Self::Device(error) => write!(f, "could not request a GPU device: {error}"),
            Self::UnsupportedSurface => f.write_str("the adapter cannot present to this surface"),
            Self::UnsupportedColorFormat { format } => write!(f, "{format:?} cannot be a color attachment on this device"),
            Self::InvalidFramebufferUsage { format, usage } => write!(f, "invalid framebuffer usage {usage:?} for {format:?}"),
            Self::UnsupportedMeshFormat { format } => write!(f, "the mesh renderer requires a blendable floating-point output, got {format:?}"),
            Self::TargetSuspended => f.write_str("a zero-sized framebuffer has no attachment"),
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
