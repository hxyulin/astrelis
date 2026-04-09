//! GPU error types.

use std::fmt;

/// Errors that can occur during GPU operations.
#[derive(Debug)]
#[non_exhaustive]
pub enum GpuError {
    /// The GPU backend failed to initialize.
    BackendInitFailed(String),
    /// No suitable GPU adapter was found.
    NoAdapter(String),
    /// Device creation failed (e.g., requested features not available).
    DeviceCreationFailed(String),
    /// Surface creation or configuration failed.
    SurfaceError(String),
    /// A shader failed to compile or validate.
    ShaderError(String),
    /// A pipeline failed to create (layout mismatch, etc.).
    PipelineError(String),
    /// A buffer or texture operation failed (out of memory, etc.).
    ResourceError(String),
    /// The surface was lost and needs reconfiguration.
    SurfaceLost,
    /// The surface is outdated (e.g., after resize) and needs reconfiguration.
    SurfaceOutdated,
    /// Timeout waiting for a GPU operation.
    Timeout,
    /// A resource handle was invalid or has been destroyed.
    InvalidHandle(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendInitFailed(msg) => write!(f, "GPU backend initialization failed: {msg}"),
            Self::NoAdapter(msg) => write!(f, "no suitable GPU adapter found: {msg}"),
            Self::DeviceCreationFailed(msg) => write!(f, "GPU device creation failed: {msg}"),
            Self::SurfaceError(msg) => write!(f, "surface error: {msg}"),
            Self::ShaderError(msg) => write!(f, "shader error: {msg}"),
            Self::PipelineError(msg) => write!(f, "pipeline error: {msg}"),
            Self::ResourceError(msg) => write!(f, "resource error: {msg}"),
            Self::SurfaceLost => write!(f, "surface lost"),
            Self::SurfaceOutdated => write!(f, "surface outdated"),
            Self::Timeout => write!(f, "GPU operation timed out"),
            Self::InvalidHandle(msg) => write!(f, "invalid resource handle: {msg}"),
        }
    }
}

impl std::error::Error for GpuError {}
