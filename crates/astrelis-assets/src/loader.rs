//! Asset loader traits and registry.
//!
//! [`AssetLoader`] is the user-facing trait for defining how to load a
//! specific asset type from raw bytes. The `LoaderRegistry` stores
//! type-erased loaders and looks them up by file extension.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::{Asset, AssetLoadError};

/// Trait for types that can load assets from raw bytes.
///
/// Each loader declares which file extensions it handles and provides a
/// `load` method that transforms raw bytes into a concrete asset type.
///
/// # Example
///
/// ```
/// use astrelis_assets::{Asset, AssetLoader, AssetLoadError};
/// use std::path::Path;
///
/// struct JsonAsset { pub data: String }
///
/// impl Asset for JsonAsset {
///     fn type_name() -> &'static str { "JsonAsset" }
/// }
///
/// struct JsonLoader;
///
/// impl AssetLoader for JsonLoader {
///     type Asset = JsonAsset;
///
///     fn extensions(&self) -> &[&str] { &["json"] }
///
///     fn load(&self, bytes: &[u8], _path: &Path) -> Result<Self::Asset, AssetLoadError> {
///         let data = String::from_utf8(bytes.to_vec())
///             .map_err(|e| AssetLoadError::Parse(e.to_string()))?;
///         Ok(JsonAsset { data })
///     }
/// }
/// ```
pub trait AssetLoader: Send + Sync + 'static {
    /// The asset type this loader produces.
    type Asset: Asset;

    /// File extensions this loader handles (e.g., `["png", "jpg"]`).
    fn extensions(&self) -> &[&str];

    /// Load an asset from raw bytes.
    ///
    /// The `path` parameter is provided for error messages and context;
    /// the actual bytes have already been read.
    fn load(&self, bytes: &[u8], path: &Path) -> Result<Self::Asset, AssetLoadError>;
}

/// Type-erased asset loader for dynamic dispatch inside the asset server.
///
/// This allows the server to store loaders of different concrete types
/// in a single collection and invoke them without knowing the asset type.
pub(crate) trait ErasedAssetLoader: Send + Sync {
    /// Load from raw bytes, returning a type-erased `Box<dyn Any>`.
    fn load_erased(
        &self,
        bytes: &[u8],
        path: &Path,
    ) -> Result<Box<dyn Any + Send + Sync>, AssetLoadError>;
}

impl<L: AssetLoader> ErasedAssetLoader for L {
    fn load_erased(
        &self,
        bytes: &[u8],
        path: &Path,
    ) -> Result<Box<dyn Any + Send + Sync>, AssetLoadError> {
        let asset = self.load(bytes, path)?;
        Ok(Box::new(Arc::new(asset)) as Box<dyn Any + Send + Sync>)
    }
}

/// Registry mapping `(TypeId, extension)` to type-erased loaders.
///
/// Loaders are registered during setup and looked up at load time
/// by the asset's `TypeId` and the file extension.
pub(crate) struct LoaderRegistry {
    /// Maps (TypeId, extension) → erased loader.
    loaders: HashMap<(TypeId, String), Arc<dyn ErasedAssetLoader>>,
}

impl LoaderRegistry {
    /// Creates an empty registry.
    pub(crate) fn new() -> Self {
        Self {
            loaders: HashMap::new(),
        }
    }

    /// Registers a loader for all of its declared extensions.
    pub(crate) fn add<L: AssetLoader>(&mut self, loader: L) {
        let type_id = TypeId::of::<L::Asset>();
        let extensions: Vec<String> = loader.extensions().iter().map(|e| e.to_lowercase()).collect();
        let arc: Arc<dyn ErasedAssetLoader> = Arc::new(loader);

        for ext in extensions {
            self.loaders.insert((type_id, ext), Arc::clone(&arc));
        }
    }

    /// Looks up a loader by asset type and file extension.
    pub(crate) fn find(
        &self,
        type_id: TypeId,
        extension: &str,
    ) -> Option<Arc<dyn ErasedAssetLoader>> {
        self.loaders
            .get(&(type_id, extension.to_lowercase()))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Asset;

    struct DummyAsset;
    impl Asset for DummyAsset {
        fn type_name() -> &'static str {
            "DummyAsset"
        }
    }

    struct DummyLoader;
    impl AssetLoader for DummyLoader {
        type Asset = DummyAsset;

        fn extensions(&self) -> &[&str] {
            &["bin", "dat"]
        }

        fn load(&self, _bytes: &[u8], _path: &Path) -> Result<Self::Asset, AssetLoadError> {
            Ok(DummyAsset)
        }
    }

    #[test]
    fn register_and_find() {
        let mut reg = LoaderRegistry::new();
        reg.add(DummyLoader);

        assert!(reg.find(TypeId::of::<DummyAsset>(), "bin").is_some());
        assert!(reg.find(TypeId::of::<DummyAsset>(), "dat").is_some());
        assert!(reg.find(TypeId::of::<DummyAsset>(), "png").is_none());
    }

    #[test]
    fn case_insensitive_extension() {
        let mut reg = LoaderRegistry::new();
        reg.add(DummyLoader);

        assert!(reg.find(TypeId::of::<DummyAsset>(), "BIN").is_some());
        assert!(reg.find(TypeId::of::<DummyAsset>(), "Dat").is_some());
    }

    #[test]
    fn erased_load_roundtrip() {
        let mut reg = LoaderRegistry::new();
        reg.add(DummyLoader);

        let loader = reg.find(TypeId::of::<DummyAsset>(), "bin").unwrap();
        let result = loader.load_erased(&[], Path::new("test.bin"));
        assert!(result.is_ok());

        let boxed = result.unwrap();
        assert!(boxed.downcast::<Arc<DummyAsset>>().is_ok());
    }
}
