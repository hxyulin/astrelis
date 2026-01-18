//! Platform-abstracted I/O for asset loading.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use crate::error::AssetError;

/// Result type for async I/O operations.
pub type IoResult<T> = Result<T, AssetError>;

/// Future type for async byte loading.
pub type BytesFuture = Pin<Box<dyn Future<Output = IoResult<Vec<u8>>> + Send + 'static>>;

/// Trait for loading bytes from various sources.
pub trait BytesReader: Send + Sync {
    /// Read all bytes from a path.
    fn read_bytes(&self, path: &Path) -> BytesFuture;

    /// Check if a path exists.
    fn exists(&self, path: &Path) -> Pin<Box<dyn Future<Output = bool> + Send + 'static>>;
}

/// Synchronous file reader (native platforms).
/// Uses blocking I/O wrapped in ready futures.
#[cfg(not(target_arch = "wasm32"))]
pub struct FileReader {
    /// Base path for relative paths.
    base_path: std::path::PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileReader {
    /// Create a new file reader with a base path.
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Resolve a path relative to the base path.
    fn resolve_path(&self, path: &Path) -> std::path::PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_path.join(path)
        }
    }

    /// Read bytes synchronously.
    pub fn read_bytes_sync(&self, path: &Path) -> IoResult<Vec<u8>> {
        let full_path = self.resolve_path(path);
        std::fs::read(&full_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AssetError::NotFound {
                    path: full_path.display().to_string(),
                }
            } else {
                AssetError::IoError {
                    path: full_path.clone(),
                    source: e,
                }
            }
        })
    }

    /// Check if a path exists synchronously.
    pub fn exists_sync(&self, path: &Path) -> bool {
        let full_path = self.resolve_path(path);
        full_path.exists()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl BytesReader for FileReader {
    fn read_bytes(&self, path: &Path) -> BytesFuture {
        let result = self.read_bytes_sync(path);
        Box::pin(async move { result })
    }

    fn exists(&self, path: &Path) -> Pin<Box<dyn Future<Output = bool> + Send + 'static>> {
        let exists = self.exists_sync(path);
        Box::pin(async move { exists })
    }
}

/// In-memory bytes reader for testing or embedded assets.
#[derive(Default)]
pub struct MemoryReader {
    /// Stored bytes keyed by path string.
    files: std::collections::HashMap<String, Vec<u8>>,
}

impl MemoryReader {
    /// Create a new empty memory reader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add bytes for a path.
    pub fn insert(&mut self, path: impl AsRef<str>, bytes: Vec<u8>) {
        self.files.insert(path.as_ref().to_string(), bytes);
    }

    /// Add bytes from static data.
    pub fn insert_static(&mut self, path: impl AsRef<str>, bytes: &'static [u8]) {
        self.files.insert(path.as_ref().to_string(), bytes.to_vec());
    }

    /// Remove bytes for a path.
    pub fn remove(&mut self, path: impl AsRef<str>) -> Option<Vec<u8>> {
        self.files.remove(path.as_ref())
    }

    /// Check if bytes exist for a path.
    pub fn contains(&self, path: impl AsRef<str>) -> bool {
        self.files.contains_key(path.as_ref())
    }
}

impl BytesReader for MemoryReader {
    fn read_bytes(&self, path: &Path) -> BytesFuture {
        let key = path.to_string_lossy().to_string();
        let result = self
            .files
            .get(&key)
            .cloned()
            .ok_or(AssetError::NotFound { path: key });

        Box::pin(async move { result })
    }

    fn exists(&self, path: &Path) -> Pin<Box<dyn Future<Output = bool> + Send + 'static>> {
        let key = path.to_string_lossy().to_string();
        let exists = self.files.contains_key(&key);
        Box::pin(async move { exists })
    }
}

/// Web fetch reader for WASM targets.
#[cfg(target_arch = "wasm32")]
pub struct FetchReader {
    /// Base URL for relative paths.
    base_url: String,
}

#[cfg(target_arch = "wasm32")]
impl FetchReader {
    /// Create a new fetch reader with a base URL.
    pub fn new(base_url: impl AsRef<str>) -> Self {
        let mut base = base_url.as_ref().to_string();
        if !base.ends_with('/') {
            base.push('/');
        }
        Self { base_url: base }
    }

    /// Resolve a path to a full URL.
    fn resolve_url(&self, path: &Path) -> String {
        let path_str = path.to_string_lossy();
        if path_str.starts_with("http://") || path_str.starts_with("https://") {
            path_str.to_string()
        } else {
            format!("{}{}", self.base_url, path_str)
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl BytesReader for FetchReader {
    fn read_bytes(&self, path: &Path) -> BytesFuture {
        let url = self.resolve_url(path);

        Box::pin(async move {
            use wasm_bindgen::JsCast;
            use wasm_bindgen_futures::JsFuture;
            use web_sys::{Request, RequestInit, Response};

            let window = web_sys::window().ok_or_else(|| {
                AssetError::IoError("No window object available".to_string())
            })?;

            let opts = RequestInit::new();
            opts.set_method("GET");

            let request = Request::new_with_str_and_init(&url, &opts)
                .map_err(|e| AssetError::IoError(format!("Failed to create request: {:?}", e)))?;

            let resp_value = JsFuture::from(window.fetch_with_request(&request))
                .await
                .map_err(|e| AssetError::IoError(format!("Fetch failed: {:?}", e)))?;

            let resp: Response = resp_value.dyn_into().map_err(|_| {
                AssetError::IoError("Response is not a Response object".to_string())
            })?;

            if !resp.ok() {
                return Err(AssetError::NotFound(url));
            }

            let array_buffer = JsFuture::from(
                resp.array_buffer()
                    .map_err(|e| AssetError::IoError(format!("Failed to get array buffer: {:?}", e)))?,
            )
            .await
            .map_err(|e| AssetError::IoError(format!("Failed to read response: {:?}", e)))?;

            let uint8_array = js_sys::Uint8Array::new(&array_buffer);
            Ok(uint8_array.to_vec())
        })
    }

    fn exists(&self, path: &Path) -> Pin<Box<dyn Future<Output = bool> + Send + 'static>> {
        let url = self.resolve_url(path);

        Box::pin(async move {
            use wasm_bindgen_futures::JsFuture;
            use web_sys::{Request, RequestInit};

            let Some(window) = web_sys::window() else {
                return false;
            };

            let opts = RequestInit::new();
            opts.set_method("HEAD");

            let Ok(request) = Request::new_with_str_and_init(&url, &opts) else {
                return false;
            };

            match JsFuture::from(window.fetch_with_request(&request)).await {
                Ok(resp) => {
                    use wasm_bindgen::JsCast;
                    resp.dyn_ref::<web_sys::Response>()
                        .map(|r| r.ok())
                        .unwrap_or(false)
                }
                Err(_) => false,
            }
        })
    }
}
