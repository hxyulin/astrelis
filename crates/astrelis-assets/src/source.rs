//! Asset sources - where assets come from.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The source of an asset - where to load it from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetSource {
    /// Load from a file on disk.
    Disk {
        /// The original path (as provided).
        path: PathBuf,
        /// Canonicalized/normalized key for deduplication.
        canonical_key: String,
    },

    /// Load from a named memory source (e.g., embedded assets).
    Memory {
        /// A unique key identifying this memory source.
        key: String,
    },

    /// Load from raw bytes (already in memory).
    Bytes {
        /// Identifier for this data (required for deduplication).
        id: String,
        /// The raw bytes.
        data: Arc<[u8]>,
    },
}

impl AssetSource {
    /// Create a disk source from a path.
    ///
    /// The path will be normalized for consistent deduplication:
    /// - Converted to absolute path if possible
    /// - On case-insensitive systems (Windows/macOS), lowercased for comparison
    pub fn disk(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let canonical_key = Self::normalize_path(path);
        AssetSource::Disk {
            path: path.to_path_buf(),
            canonical_key,
        }
    }

    /// Create a disk source with a pre-computed canonical key.
    /// Useful when the canonical form is already known.
    pub fn disk_with_key(path: impl AsRef<Path>, canonical_key: impl Into<String>) -> Self {
        AssetSource::Disk {
            path: path.as_ref().to_path_buf(),
            canonical_key: canonical_key.into(),
        }
    }

    /// Create a memory source with a key.
    pub fn memory(key: impl Into<String>) -> Self {
        AssetSource::Memory { key: key.into() }
    }

    /// Create a bytes source with an identifier.
    ///
    /// The identifier is required for deduplication and hot-reload support.
    pub fn bytes(id: impl Into<String>, data: impl Into<Arc<[u8]>>) -> Self {
        AssetSource::Bytes {
            id: id.into(),
            data: data.into(),
        }
    }

    /// Create a bytes source with a content-hash based identifier.
    ///
    /// Use this when you don't have a meaningful ID - the hash ensures
    /// identical content is deduplicated.
    pub fn bytes_hashed(data: impl Into<Arc<[u8]>>) -> Self {
        let data: Arc<[u8]> = data.into();
        // Simple hash for ID (not cryptographic, just for dedup)
        let hash = Self::simple_hash(&data);
        AssetSource::Bytes {
            id: format!("hash:{:016x}", hash),
            data,
        }
    }

    /// Normalize a path for consistent comparison/deduplication.
    fn normalize_path(path: &Path) -> String {
        // Try to get absolute path
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        };

        // Try to canonicalize (resolves symlinks, normalizes case on some systems)
        let normalized = std::fs::canonicalize(&abs_path).unwrap_or(abs_path);

        // Convert to string
        let path_str = normalized.to_string_lossy().to_string();

        // On case-insensitive file systems, lowercase for consistent comparison
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        let path_str = path_str.to_lowercase();

        path_str
    }

    /// Simple non-cryptographic hash for content-based IDs.
    fn simple_hash(data: &[u8]) -> u64 {
        // FNV-1a hash
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in data {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// Get the original path if this is a disk source.
    pub fn path(&self) -> Option<&Path> {
        match self {
            AssetSource::Disk { path, .. } => Some(path),
            _ => None,
        }
    }

    /// Get the extension if this is a disk source.
    pub fn extension(&self) -> Option<&str> {
        match self {
            AssetSource::Disk { path, .. } => path.extension().and_then(|e| e.to_str()),
            AssetSource::Memory { key } => {
                // Try to extract extension from memory key
                key.rsplit('.').next().filter(|e| !e.contains('/'))
            }
            AssetSource::Bytes { id, .. } => {
                // Try to extract extension from bytes ID
                id.rsplit('.')
                    .next()
                    .filter(|e| !e.contains('/') && !e.contains(':'))
            }
        }
    }

    /// Get the unique key for this source (used for deduplication).
    pub fn key(&self) -> &str {
        match self {
            AssetSource::Disk { canonical_key, .. } => canonical_key,
            AssetSource::Memory { key } => key,
            AssetSource::Bytes { id, .. } => id,
        }
    }

    /// Get a string representation of this source for logging/debugging.
    pub fn display_path(&self) -> String {
        match self {
            AssetSource::Disk { path, .. } => path.display().to_string(),
            AssetSource::Memory { key } => format!("memory://{}", key),
            AssetSource::Bytes { id, .. } => format!("bytes://{}", id),
        }
    }

    /// Check if this is a disk source.
    pub fn is_disk(&self) -> bool {
        matches!(self, AssetSource::Disk { .. })
    }

    /// Check if this is a memory source.
    pub fn is_memory(&self) -> bool {
        matches!(self, AssetSource::Memory { .. })
    }

    /// Check if this is a bytes source.
    pub fn is_bytes(&self) -> bool {
        matches!(self, AssetSource::Bytes { .. })
    }
}

impl<P: AsRef<Path>> From<P> for AssetSource {
    fn from(path: P) -> Self {
        AssetSource::disk(path)
    }
}

/// Settings for how an asset should be loaded.
#[derive(Debug, Clone, Default)]
pub struct LoadSettings {
    /// Force reload even if already loaded.
    pub force_reload: bool,

    /// Load synchronously (blocking).
    pub blocking: bool,

    /// Custom loader to use (overrides extension-based selection).
    pub loader_name: Option<String>,
}

impl LoadSettings {
    /// Create default load settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set force reload.
    pub fn force_reload(mut self, force: bool) -> Self {
        self.force_reload = force;
        self
    }

    /// Set blocking mode.
    pub fn blocking(mut self, blocking: bool) -> Self {
        self.blocking = blocking;
        self
    }

    /// Set a specific loader to use.
    pub fn with_loader(mut self, loader_name: impl Into<String>) -> Self {
        self.loader_name = Some(loader_name.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_source_deduplication() {
        // Same path should produce same key
        let source1 = AssetSource::disk("foo/bar.txt");
        let source2 = AssetSource::disk("foo/bar.txt");
        assert_eq!(source1.key(), source2.key());
    }

    #[test]
    fn test_extension_extraction() {
        let disk = AssetSource::disk("textures/player.png");
        assert_eq!(disk.extension(), Some("png"));

        let memory = AssetSource::memory("sprites/enemy.jpg");
        assert_eq!(memory.extension(), Some("jpg"));

        let bytes = AssetSource::bytes("model.gltf", vec![1, 2, 3]);
        assert_eq!(bytes.extension(), Some("gltf"));
    }

    #[test]
    fn test_bytes_hashed() {
        let data1: Vec<u8> = vec![1, 2, 3, 4, 5];
        let data2: Vec<u8> = vec![1, 2, 3, 4, 5];
        let data3: Vec<u8> = vec![5, 4, 3, 2, 1];

        let source1 = AssetSource::bytes_hashed(data1);
        let source2 = AssetSource::bytes_hashed(data2);
        let source3 = AssetSource::bytes_hashed(data3);

        // Same content -> same key
        assert_eq!(source1.key(), source2.key());
        // Different content -> different key
        assert_ne!(source1.key(), source3.key());
    }
}
