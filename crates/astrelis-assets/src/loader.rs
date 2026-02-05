//! Asset loader traits and infrastructure.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{AssetError, AssetResult};
use crate::source::AssetSource;

/// Context provided to asset loaders during loading.
pub struct LoadContext<'a> {
    /// The source of the asset being loaded.
    pub source: &'a AssetSource,
    /// The raw bytes of the asset.
    pub bytes: &'a [u8],
    /// File extension (without the dot), if available.
    pub extension: Option<&'a str>,
}

impl<'a> LoadContext<'a> {
    /// Create a new load context.
    pub fn new(source: &'a AssetSource, bytes: &'a [u8], extension: Option<&'a str>) -> Self {
        Self {
            source,
            bytes,
            extension,
        }
    }
}

/// Default priority for loaders.
pub const DEFAULT_LOADER_PRIORITY: i32 = 0;

/// Trait for loading assets from bytes.
///
/// Implement this trait to add support for loading a specific asset type.
///
/// # Example
///
/// ```ignore
/// struct PngLoader;
///
/// impl AssetLoader for PngLoader {
///     type Asset = Texture;
///
///     fn extensions(&self) -> &[&str] {
///         &["png"]
///     }
///
///     fn load(&self, ctx: LoadContext<'_>) -> AssetResult<Self::Asset> {
///         // Decode PNG bytes into Texture...
///     }
/// }
/// ```
pub trait AssetLoader: Send + Sync + 'static {
    /// The asset type this loader produces.
    type Asset: Send + Sync + 'static;

    /// The file extensions this loader handles (without dots).
    ///
    /// Example: `&["png", "jpg", "jpeg"]`
    fn extensions(&self) -> &[&str];

    /// Load an asset from the provided context.
    fn load(&self, ctx: LoadContext<'_>) -> AssetResult<Self::Asset>;

    /// Priority for this loader. Higher priority loaders are tried first
    /// when multiple loaders handle the same extension for the same type.
    ///
    /// Default is 0. Use positive values to override default loaders.
    fn priority(&self) -> i32 {
        DEFAULT_LOADER_PRIORITY
    }
}

/// Type-erased asset loader for dynamic dispatch.
pub trait ErasedAssetLoader: Send + Sync {
    /// Get the type ID of the asset this loader produces.
    fn asset_type_id(&self) -> TypeId;

    /// Get a human-readable name for the asset type.
    fn asset_type_name(&self) -> &'static str;

    /// Get the file extensions this loader handles.
    fn extensions(&self) -> &[&str];

    /// Get the priority of this loader.
    fn priority(&self) -> i32;

    /// Load an asset and return it as a boxed Any.
    fn load_erased(&self, ctx: LoadContext<'_>) -> AssetResult<Box<dyn Any + Send + Sync>>;
}

impl<L: AssetLoader> ErasedAssetLoader for L
where
    L::Asset: crate::Asset,
{
    fn asset_type_id(&self) -> TypeId {
        TypeId::of::<L::Asset>()
    }

    fn asset_type_name(&self) -> &'static str {
        <L::Asset as crate::Asset>::type_name()
    }

    fn extensions(&self) -> &[&str] {
        AssetLoader::extensions(self)
    }

    fn priority(&self) -> i32 {
        AssetLoader::priority(self)
    }

    fn load_erased(&self, ctx: LoadContext<'_>) -> AssetResult<Box<dyn Any + Send + Sync>> {
        let asset = self.load(ctx)?;
        Ok(Box::new(asset))
    }
}

/// Key for indexing loaders by type and extension.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LoaderKey {
    type_id: TypeId,
    extension: String,
}

/// Entry in the loader registry with priority.
struct LoaderEntry {
    loader: Arc<dyn ErasedAssetLoader>,
    priority: i32,
}

/// Registry of asset loaders, indexed by asset type and extension.
///
/// When loading an asset of type `T` with extension `.ext`, the registry
/// finds all loaders that:
/// 1. Produce type `T` (matching `TypeId`)
/// 2. Handle extension `ext`
///
/// If multiple loaders match, the one with highest priority is used.
#[derive(Default)]
pub struct LoaderRegistry {
    /// Loaders indexed by (TypeId, extension) -> sorted by priority (highest first).
    by_type_and_ext: HashMap<LoaderKey, Vec<LoaderEntry>>,
    /// All loaders for a given type (for listing).
    by_type: HashMap<TypeId, Vec<Arc<dyn ErasedAssetLoader>>>,
    /// All loaders for a given extension (for fallback/listing).
    by_extension: HashMap<String, Vec<LoaderEntry>>,
}

impl LoaderRegistry {
    /// Create a new empty loader registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a loader for its declared extensions.
    ///
    /// The loader will be used when loading assets of type `L::Asset`
    /// with any of the extensions returned by `extensions()`.
    pub fn register<L: AssetLoader>(&mut self, loader: L)
    where
        L::Asset: crate::Asset,
    {
        let loader = Arc::new(loader);
        let type_id = loader.asset_type_id();
        let priority = loader.priority();

        // Register for each extension
        for ext in loader.extensions() {
            let ext_lower = ext.to_lowercase();

            // Index by (type, extension)
            let key = LoaderKey {
                type_id,
                extension: ext_lower.clone(),
            };

            let entries = self.by_type_and_ext.entry(key).or_default();
            entries.push(LoaderEntry {
                loader: loader.clone(),
                priority,
            });
            // Sort by priority (highest first)
            entries.sort_by(|a, b| b.priority.cmp(&a.priority));

            // Index by extension only (for fallback)
            let ext_entries = self.by_extension.entry(ext_lower).or_default();
            ext_entries.push(LoaderEntry {
                loader: loader.clone(),
                priority,
            });
            ext_entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        }

        // Index by type
        self.by_type.entry(type_id).or_default().push(loader);
    }

    /// Get the best loader for a specific type and extension.
    ///
    /// Returns the highest-priority loader that produces type `T` and handles `extension`.
    pub fn get_for_type_and_extension<T: 'static>(
        &self,
        extension: &str,
    ) -> Option<&Arc<dyn ErasedAssetLoader>> {
        let key = LoaderKey {
            type_id: TypeId::of::<T>(),
            extension: extension.to_lowercase(),
        };

        self.by_type_and_ext
            .get(&key)
            .and_then(|entries| entries.first())
            .map(|entry| &entry.loader)
    }

    /// Get all loaders for a specific asset type.
    pub fn get_by_type<T: 'static>(&self) -> Option<&[Arc<dyn ErasedAssetLoader>]> {
        self.by_type.get(&TypeId::of::<T>()).map(|v| v.as_slice())
    }

    /// Get all loaders for an extension (any type), sorted by priority.
    pub fn get_by_extension(&self, extension: &str) -> Option<&Arc<dyn ErasedAssetLoader>> {
        let ext_lower = extension.to_lowercase();
        self.by_extension
            .get(&ext_lower)
            .and_then(|entries| entries.first())
            .map(|entry| &entry.loader)
    }

    /// Check if a loader is registered for an extension and type.
    pub fn has_loader_for<T: 'static>(&self, extension: &str) -> bool {
        let key = LoaderKey {
            type_id: TypeId::of::<T>(),
            extension: extension.to_lowercase(),
        };
        self.by_type_and_ext.contains_key(&key)
    }

    /// Check if a loader is registered for a type.
    pub fn has_loader_for_type<T: 'static>(&self) -> bool {
        self.by_type.contains_key(&TypeId::of::<T>())
    }

    /// Check if any loader is registered for an extension.
    pub fn has_loader_for_extension(&self, extension: &str) -> bool {
        let ext_lower = extension.to_lowercase();
        self.by_extension.contains_key(&ext_lower)
    }

    /// Load an asset of type `T` using the appropriate loader.
    ///
    /// Finds a loader that:
    /// 1. Produces type `T`
    /// 2. Handles the given extension
    ///
    /// Returns an error if no such loader exists.
    pub fn load_typed<T: crate::Asset>(
        &self,
        source: &AssetSource,
        bytes: &[u8],
        extension: Option<&str>,
    ) -> AssetResult<T> {
        let ext = extension.ok_or_else(|| AssetError::NoLoaderForExtension {
            extension: "<none>".to_string(),
        })?;

        let loader =
            self.get_for_type_and_extension::<T>(ext)
                .ok_or_else(|| AssetError::NoLoader {
                    type_id: TypeId::of::<T>(),
                    type_name: Some(T::type_name()),
                })?;

        let ctx = LoadContext::new(source, bytes, Some(ext));
        let boxed = loader.load_erased(ctx)?;

        // This should always succeed since we looked up by TypeId
        boxed
            .downcast::<T>()
            .map(|b| *b)
            .map_err(|_| AssetError::TypeMismatch {
                expected: T::type_name(),
                actual: TypeId::of::<T>(),
            })
    }

    /// Load an asset using extension-based lookup (type-erased).
    ///
    /// Uses the highest-priority loader for the extension regardless of type.
    /// Primarily for backwards compatibility or when type is not known at compile time.
    pub fn load(
        &self,
        source: &AssetSource,
        bytes: &[u8],
        extension: Option<&str>,
    ) -> AssetResult<Box<dyn Any + Send + Sync>> {
        let ext = extension.ok_or_else(|| AssetError::NoLoaderForExtension {
            extension: "<none>".to_string(),
        })?;

        let loader =
            self.get_by_extension(ext)
                .ok_or_else(|| AssetError::NoLoaderForExtension {
                    extension: ext.to_string(),
                })?;

        let ctx = LoadContext::new(source, bytes, Some(ext));
        loader.load_erased(ctx)
    }

    /// List all registered extensions for a type.
    pub fn extensions_for_type<T: 'static>(&self) -> Vec<&str> {
        self.by_type
            .get(&TypeId::of::<T>())
            .map(|loaders| {
                loaders
                    .iter()
                    .flat_map(|l| l.extensions().iter().copied())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// A simple text loader that loads UTF-8 strings.
pub struct TextLoader;

impl AssetLoader for TextLoader {
    type Asset = String;

    fn extensions(&self) -> &[&str] {
        &["txt", "text", "md", "markdown"]
    }

    fn load(&self, ctx: LoadContext<'_>) -> AssetResult<Self::Asset> {
        String::from_utf8(ctx.bytes.to_vec()).map_err(|e| AssetError::LoaderError {
            path: ctx.source.display_path(),
            message: format!("Invalid UTF-8: {}", e),
        })
    }
}

/// A simple binary loader that loads raw bytes.
pub struct BytesLoader;

impl AssetLoader for BytesLoader {
    type Asset = Vec<u8>;

    fn extensions(&self) -> &[&str] {
        &["bin", "bytes", "dat"]
    }

    fn load(&self, ctx: LoadContext<'_>) -> AssetResult<Self::Asset> {
        Ok(ctx.bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Asset;

    // Test asset type
    #[derive(Debug, PartialEq)]
    struct TestData {
        value: i32,
    }

    impl Asset for TestData {
        fn type_name() -> &'static str {
            "TestData"
        }
    }

    // Low priority loader
    struct LowPriorityLoader;

    impl AssetLoader for LowPriorityLoader {
        type Asset = TestData;

        fn extensions(&self) -> &[&str] {
            &["dat"]
        }

        fn priority(&self) -> i32 {
            -10
        }

        fn load(&self, _ctx: LoadContext<'_>) -> AssetResult<Self::Asset> {
            Ok(TestData { value: 1 })
        }
    }

    // High priority loader
    struct HighPriorityLoader;

    impl AssetLoader for HighPriorityLoader {
        type Asset = TestData;

        fn extensions(&self) -> &[&str] {
            &["dat"]
        }

        fn priority(&self) -> i32 {
            10
        }

        fn load(&self, _ctx: LoadContext<'_>) -> AssetResult<Self::Asset> {
            Ok(TestData { value: 100 })
        }
    }

    #[test]
    fn test_text_loader() {
        let loader = TextLoader;
        let source = AssetSource::memory("test.txt");
        let bytes = b"Hello, World!";
        let ctx = LoadContext::new(&source, bytes, Some("txt"));

        let result = loader.load(ctx).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_bytes_loader() {
        let loader = BytesLoader;
        let source = AssetSource::memory("test.bin");
        let bytes = &[0u8, 1, 2, 3, 4];
        let ctx = LoadContext::new(&source, bytes, Some("bin"));

        let result = loader.load(ctx).unwrap();
        assert_eq!(result, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_loader_registry_by_type() {
        let mut registry = LoaderRegistry::new();
        registry.register(TextLoader);
        registry.register(BytesLoader);

        // Should find loader for String + txt
        assert!(registry.has_loader_for::<String>("txt"));
        assert!(registry.has_loader_for::<String>("TXT")); // Case insensitive

        // Should find loader for Vec<u8> + bin
        assert!(registry.has_loader_for::<Vec<u8>>("bin"));

        // Should NOT find String loader for bin (wrong type)
        assert!(!registry.has_loader_for::<String>("bin"));

        // Should NOT find Vec<u8> loader for txt (wrong type)
        assert!(!registry.has_loader_for::<Vec<u8>>("txt"));

        assert!(registry.has_loader_for_type::<String>());
        assert!(registry.has_loader_for_type::<Vec<u8>>());
    }

    #[test]
    fn test_loader_priority() {
        let mut registry = LoaderRegistry::new();

        // Register low priority first, then high priority
        registry.register(LowPriorityLoader);
        registry.register(HighPriorityLoader);

        // Should use high priority loader
        let source = AssetSource::memory("test.dat");
        let result: TestData = registry.load_typed(&source, b"", Some("dat")).unwrap();
        assert_eq!(result.value, 100);
    }

    #[test]
    fn test_loader_priority_reverse_order() {
        let mut registry = LoaderRegistry::new();

        // Register high priority first, then low priority
        registry.register(HighPriorityLoader);
        registry.register(LowPriorityLoader);

        // Should still use high priority loader
        let source = AssetSource::memory("test.dat");
        let result: TestData = registry.load_typed(&source, b"", Some("dat")).unwrap();
        assert_eq!(result.value, 100);
    }

    #[test]
    fn test_typed_load() {
        let mut registry = LoaderRegistry::new();
        registry.register(TextLoader);

        let source = AssetSource::memory("test.txt");
        let result: String = registry
            .load_typed(&source, b"Hello!", Some("txt"))
            .unwrap();
        assert_eq!(result, "Hello!");
    }

    #[test]
    fn test_no_loader_for_type_extension_combo() {
        let mut registry = LoaderRegistry::new();
        registry.register(TextLoader); // Only handles String

        // Try to load TestData from .txt - should fail
        let source = AssetSource::memory("test.txt");
        let result: AssetResult<TestData> = registry.load_typed(&source, b"data", Some("txt"));
        assert!(result.is_err());
    }
}
