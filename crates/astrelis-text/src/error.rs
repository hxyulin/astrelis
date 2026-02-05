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
        requested_width: u32,
        requested_height: u32,
        atlas_width: u32,
        atlas_height: u32,
    },

    /// GPU resource creation failed.
    GpuResourceError(String),

    /// Invalid text range.
    InvalidRange {
        start: usize,
        end: usize,
        text_len: usize,
    },

    /// Generic IO error.
    IoError(String),
}

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextError::FontLoadError(msg) => write!(f, "Failed to load font: {}", msg),
            TextError::FontFileNotFound(path) => {
                write!(f, "Font file not found: {}", path.display())
            }
            TextError::InvalidFontData(msg) => write!(f, "Invalid font data: {}", msg),
            TextError::LockPoisoned(msg) => {
                write!(
                    f,
                    "Lock was poisoned (likely due to panic in another thread): {}",
                    msg
                )
            }
            TextError::ShapingError(msg) => write!(f, "Text shaping failed: {}", msg),
            TextError::BufferAllocationFailed(msg) => {
                write!(f, "Buffer allocation failed: {}", msg)
            }
            TextError::AtlasFull {
                requested_width,
                requested_height,
                atlas_width,
                atlas_height,
            } => write!(
                f,
                "Texture atlas is full: requested {}x{} but atlas is {}x{}",
                requested_width, requested_height, atlas_width, atlas_height
            ),
            TextError::GpuResourceError(msg) => write!(f, "GPU resource error: {}", msg),
            TextError::InvalidRange {
                start,
                end,
                text_len,
            } => write!(
                f,
                "Invalid text range: {}..{} (text length: {})",
                start, end, text_len
            ),
            TextError::IoError(msg) => write!(f, "IO error: {}", msg),
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
