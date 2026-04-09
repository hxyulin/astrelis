//! Error types for text operations.

/// Errors that can occur in the text rendering system.
#[derive(Debug, Clone)]
pub enum TextError {
    /// Font loading failed.
    FontLoadError(String),

    /// Font file not found.
    FontFileNotFound(std::path::PathBuf),

    /// Invalid font data.
    InvalidFontData(String),

    /// Lock was poisoned (RwLock/Mutex).
    LockPoisoned(String),

    /// Text shaping failed.
    ShapingError(String),

    /// Buffer allocation failed.
    BufferAllocationFailed(String),

    /// Texture atlas is full.
    AtlasFull {
        /// Requested glyph width.
        requested_width: u32,
        /// Requested glyph height.
        requested_height: u32,
        /// Total atlas width.
        atlas_width: u32,
        /// Total atlas height.
        atlas_height: u32,
    },

    /// GPU resource creation failed.
    GpuResourceError(String),

    /// Invalid text range.
    InvalidRange {
        /// Start of the range.
        start: usize,
        /// End of the range.
        end: usize,
        /// Length of the text.
        text_len: usize,
    },

    /// Generic IO error.
    IoError(String),
}

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextError::FontLoadError(msg) => write!(f, "Failed to load font: {msg}"),
            TextError::FontFileNotFound(path) => {
                write!(f, "Font file not found: {}", path.display())
            }
            TextError::InvalidFontData(msg) => write!(f, "Invalid font data: {msg}"),
            TextError::LockPoisoned(msg) => {
                write!(
                    f,
                    "Lock was poisoned (likely due to panic in another thread): {msg}",
                )
            }
            TextError::ShapingError(msg) => write!(f, "Text shaping failed: {msg}"),
            TextError::BufferAllocationFailed(msg) => {
                write!(f, "Buffer allocation failed: {msg}")
            }
            TextError::AtlasFull {
                requested_width,
                requested_height,
                atlas_width,
                atlas_height,
            } => write!(
                f,
                "Texture atlas is full: requested {requested_width}x{requested_height} but atlas is {atlas_width}x{atlas_height}",
            ),
            TextError::GpuResourceError(msg) => write!(f, "GPU resource error: {msg}"),
            TextError::InvalidRange {
                start,
                end,
                text_len,
            } => write!(
                f,
                "Invalid text range: {start}..{end} (text length: {text_len})",
            ),
            TextError::IoError(msg) => write!(f, "IO error: {msg}"),
        }
    }
}

impl std::error::Error for TextError {}

impl From<std::io::Error> for TextError {
    fn from(err: std::io::Error) -> Self {
        TextError::IoError(err.to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for TextError {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        TextError::LockPoisoned(err.to_string())
    }
}

/// Result type for text operations.
pub type TextResult<T> = Result<T, TextError>;
