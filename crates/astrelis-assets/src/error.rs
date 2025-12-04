//! Error types for the asset system.

use std::any::TypeId;
use std::fmt;
use std::path::PathBuf;

/// Errors that can occur during asset operations.
#[derive(Debug)]
pub enum AssetError {
    /// The requested asset was not found.
    NotFound {
        /// The path or identifier of the asset.
        path: String,
    },

    /// Failed to read asset data from the source.
    IoError {
        /// The path that failed to load.
        path: PathBuf,
        /// The underlying IO error.
        source: std::io::Error,
    },

    /// No loader registered for this asset type.
    NoLoader {
        /// The type ID of the asset.
        type_id: TypeId,
        /// Human-readable type name if available.
        type_name: Option<&'static str>,
    },

    /// No loader found for the given file extension.
    NoLoaderForExtension {
        /// The file extension.
        extension: String,
    },

    /// The loader failed to parse/decode the asset.
    LoaderError {
        /// The path being loaded.
        path: String,
        /// Description of the error.
        message: String,
    },

    /// The asset handle is invalid (use-after-free or wrong type).
    InvalidHandle {
        /// Description of why the handle is invalid.
        reason: String,
    },

    /// Type mismatch when accessing an asset.
    TypeMismatch {
        /// Expected type name.
        expected: &'static str,
        /// Actual type ID.
        actual: TypeId,
    },

    /// The asset is not ready yet (still loading).
    NotReady {
        /// The path of the asset.
        path: String,
    },

    /// Generic error with a message.
    Other {
        /// Error message.
        message: String,
    },
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetError::NotFound { path } => {
                write!(f, "Asset not found: {}", path)
            }
            AssetError::IoError { path, source } => {
                write!(f, "IO error loading '{}': {}", path.display(), source)
            }
            AssetError::NoLoader { type_name, .. } => {
                if let Some(name) = type_name {
                    write!(f, "No loader registered for asset type: {}", name)
                } else {
                    write!(f, "No loader registered for asset type")
                }
            }
            AssetError::NoLoaderForExtension { extension } => {
                write!(f, "No loader registered for extension: .{}", extension)
            }
            AssetError::LoaderError { path, message } => {
                write!(f, "Failed to load '{}': {}", path, message)
            }
            AssetError::InvalidHandle { reason } => {
                write!(f, "Invalid asset handle: {}", reason)
            }
            AssetError::TypeMismatch { expected, .. } => {
                write!(f, "Type mismatch: expected {}", expected)
            }
            AssetError::NotReady { path } => {
                write!(f, "Asset not ready: {}", path)
            }
            AssetError::Other { message } => {
                write!(f, "Asset error: {}", message)
            }
        }
    }
}

impl std::error::Error for AssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AssetError::IoError { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AssetError {
    fn from(err: std::io::Error) -> Self {
        AssetError::IoError {
            path: PathBuf::new(),
            source: err,
        }
    }
}

/// Result type alias for asset operations.
pub type AssetResult<T> = Result<T, AssetError>;
