//! Asset plugin for loading and managing assets.

use astrelis_assets::{AssetServer, AssetLoader};

use crate::plugin::Plugin;
use crate::resource::Resources;

#[cfg(feature = "text")]
use astrelis_text::FontLoader;

/// Plugin that provides asset loading and management.
///
/// This plugin registers an `AssetServer` resource that can be used
/// to load assets from disk, memory, or raw bytes.
///
/// # Resources Provided
///
/// - `AssetServer` - The main asset loading and caching system
///
/// # Default Loaders
///
/// The plugin registers these loaders by default:
/// - `TextLoader` - Loads `.txt`, `.text`, `.md` files as `String`
/// - `BytesLoader` - Loads `.bin`, `.bytes`, `.dat` files as `Vec<u8>`
/// - `FontLoader` - Loads `.ttf`, `.otf`, `.woff` files as `FontAsset` (with `text` feature)
///
/// # Example
///
/// ```ignore
/// use astrelis::prelude::*;
///
/// let engine = Engine::builder()
///     .add_plugin(AssetPlugin::default())
///     .build();
///
/// let server = engine.get::<AssetServer>().unwrap();
/// let text: Handle<String> = server.load_sync("hello.txt").unwrap();
/// ```
#[derive(Default)]
pub struct AssetPlugin {
    /// Base path for loading assets from disk.
    pub base_path: Option<String>,
}


impl AssetPlugin {
    /// Create a new asset plugin.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base path for loading assets.
    pub fn with_base_path(mut self, path: impl Into<String>) -> Self {
        self.base_path = Some(path.into());
        self
    }
}

impl Plugin for AssetPlugin {
    type Dependencies = ();
    fn name(&self) -> &'static str {
        "AssetPlugin"
    }

    fn build(&self, resources: &mut Resources) {
        // Create the asset server
        let mut server = match &self.base_path {
            Some(path) => AssetServer::with_base_path(path),
            None => AssetServer::new(),
        };

        // Register default loaders
        server.register_loader(astrelis_assets::TextLoader);
        server.register_loader(astrelis_assets::BytesLoader);

        // Register font loader if text feature is enabled
        #[cfg(feature = "text")]
        server.register_loader(FontLoader);

        tracing::debug!("AssetPlugin: Registered default loaders");

        resources.insert(server);
    }
}

/// Extension trait for easily registering loaders with the engine.
#[allow(dead_code)]
pub trait AssetServerExt {
    /// Register an asset loader with the asset server.
    fn register_loader<L: AssetLoader>(&mut self, loader: L)
    where
        L::Asset: astrelis_assets::Asset;
}

impl AssetServerExt for crate::Engine {
    fn register_loader<L: AssetLoader>(&mut self, loader: L)
    where
        L::Asset: astrelis_assets::Asset,
    {
        if let Some(server) = self.get_mut::<AssetServer>() {
            server.register_loader(loader);
        } else {
            tracing::warn!("AssetServer not found - is AssetPlugin added?");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EngineBuilder;

    #[test]
    fn test_asset_plugin_registers_server() {
        let engine = EngineBuilder::new()
            .add_plugin(AssetPlugin::default())
            .build();

        assert!(engine.get::<AssetServer>().is_some());
    }

    #[test]
    fn test_asset_plugin_with_base_path() {
        let engine = EngineBuilder::new()
            .add_plugin(AssetPlugin::new().with_base_path("assets"))
            .build();

        assert!(engine.get::<AssetServer>().is_some());
    }

    #[test]
    fn test_default_loaders_registered() {
        let engine = EngineBuilder::new()
            .add_plugin(AssetPlugin::default())
            .build();

        let server = engine.get::<AssetServer>().unwrap();

        // Check that text loader is registered
        assert!(server.has_loader_for::<String>("txt"));
        assert!(server.has_loader_for::<Vec<u8>>("bin"));
    }
}
