//! Asset server - the main coordinator for asset operations.

use std::any::TypeId;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;

use crate::error::AssetError;
use crate::event::{AssetEvent, AssetEventBuffer};
use crate::handle::{Handle, UntypedHandle};
use crate::io::{BytesReader, MemoryReader};
use crate::loader::{AssetLoader, LoaderRegistry};
use crate::source::AssetSource;
use crate::state::AssetState;
use crate::storage::{Assets, AssetStorages};
use crate::Asset;

#[cfg(not(target_arch = "wasm32"))]
use crate::io::FileReader;

/// Pending load task for async loading.
struct PendingLoad {
    /// The handle being loaded.
    handle: UntypedHandle,
    /// The source to load from.
    source: AssetSource,
    /// The raw bytes (once loaded).
    bytes: Option<Vec<u8>>,
    /// The extension for loader selection.
    extension: Option<String>,
}

/// The main asset server that coordinates loading, caching, and events.
///
/// # Example
///
/// ```ignore
/// let mut server = AssetServer::new();
///
/// // Register loaders
/// server.register_loader(TextureLoader);
/// server.register_loader(AudioLoader);
///
/// // Load assets
/// let texture: Handle<Texture> = server.load("sprites/player.png");
/// let audio: Handle<Audio> = server.load("sounds/jump.wav");
///
/// // Check if ready
/// if let Some(tex) = server.get(&texture) {
///     // Use the texture
/// }
///
/// // Process events
/// for event in server.drain_events() {
///     match event {
///         AssetEvent::Created { .. } => {}
///         AssetEvent::Modified { .. } => {}
///         AssetEvent::Removed { .. } => {}
///         AssetEvent::LoadFailed { .. } => {}
///     }
/// }
/// ```
pub struct AssetServer {
    /// Per-type asset storage.
    storages: AssetStorages,
    /// Registered asset loaders.
    loaders: LoaderRegistry,
    /// Event buffer for this frame.
    events: AssetEventBuffer,
    /// Pending loads for async processing.
    pending: VecDeque<PendingLoad>,
    /// Default bytes reader for disk I/O.
    #[cfg(not(target_arch = "wasm32"))]
    file_reader: FileReader,
    /// Memory reader for embedded/memory assets.
    memory_reader: MemoryReader,
    /// File watcher for hot-reload support.
    #[cfg(feature = "hot-reload")]
    watcher: Option<crate::hot_reload::AssetWatcher>,
}

impl AssetServer {
    /// Create a new asset server with default settings.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new() -> Self {
        Self::with_base_path(".")
    }

    /// Create a new asset server with a custom base path.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_base_path(base_path: impl AsRef<Path>) -> Self {
        Self {
            storages: AssetStorages::new(),
            loaders: LoaderRegistry::new(),
            events: AssetEventBuffer::new(),
            pending: VecDeque::new(),
            file_reader: FileReader::new(base_path),
            memory_reader: MemoryReader::new(),
            #[cfg(feature = "hot-reload")]
            watcher: None,
        }
    }

    /// Create a new asset server (WASM version).
    #[cfg(target_arch = "wasm32")]
    pub fn new() -> Self {
        Self {
            storages: AssetStorages::new(),
            loaders: LoaderRegistry::new(),
            events: AssetEventBuffer::new(),
            pending: VecDeque::new(),
            memory_reader: MemoryReader::new(),
        }
    }

    /// Register an asset loader.
    pub fn register_loader<L: AssetLoader>(&mut self, loader: L)
    where
        L::Asset: Asset,
    {
        self.loaders.register(loader);
    }

    /// Add embedded bytes to the memory reader.
    pub fn add_embedded(&mut self, path: impl AsRef<str>, bytes: &'static [u8]) {
        self.memory_reader.insert_static(path, bytes);
    }

    /// Load an asset from a path.
    ///
    /// Returns a handle immediately. The asset will be loaded in the background.
    /// Check if ready using `is_ready()` or `get()`.
    pub fn load<T: Asset>(&mut self, path: impl AsRef<Path>) -> Handle<T> {
        let source = AssetSource::disk(path.as_ref());
        self.load_from_source::<T>(source)
    }

    /// Load an asset from a custom source.
    pub fn load_from_source<T: Asset>(&mut self, source: AssetSource) -> Handle<T> {
        let storage = self.storages.get_or_create::<T>();

        // Check if already loaded/loading
        if let Some(existing) = storage.find_by_source(&source) {
            return existing;
        }

        // Reserve a handle
        let handle = storage.reserve(source.clone());
        storage.set_loading(&handle);

        // Queue for loading
        let extension = source
            .extension()
            .map(|s| s.to_string())
            .or_else(|| {
                if let AssetSource::Disk { path, .. } = &source {
                    path.extension().and_then(|e| e.to_str()).map(String::from)
                } else {
                    None
                }
            });

        self.pending.push_back(PendingLoad {
            handle: handle.untyped(),
            source,
            bytes: None,
            extension,
        });

        handle
    }

    /// Load an asset synchronously (blocking).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_sync<T: Asset>(&mut self, path: impl AsRef<Path>) -> Result<Handle<T>, AssetError> {
        let source = AssetSource::disk(path.as_ref());
        self.load_from_source_sync::<T>(source)
    }

    /// Load an asset synchronously from a custom source.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_source_sync<T: Asset>(
        &mut self,
        source: AssetSource,
    ) -> Result<Handle<T>, AssetError> {
        let storage = self.storages.get_or_create::<T>();

        // Check if already loaded (use canonical key for dedup)
        if let Some(existing) = storage.find_by_source(&source)
            && storage.is_ready(&existing) {
                return Ok(existing);
            }

        // Reserve a handle
        let handle = storage.reserve(source.clone());

        // Read bytes
        let bytes = match &source {
            AssetSource::Disk { path, .. } => self.file_reader.read_bytes_sync(path)?,
            AssetSource::Memory { key } => {
                let path = Path::new(key);
                futures_lite::future::block_on(self.memory_reader.read_bytes(path))?
            }
            AssetSource::Bytes { data, .. } => data.to_vec(),
        };

        // Get extension
        let extension = source.extension().map(String::from);

        // Use the type-indexed load method to find the right loader for type T
        let asset: T = self
            .loaders
            .load_typed::<T>(&source, &bytes, extension.as_deref())?;

        // Store the asset
        let storage = self.storages.get_or_create::<T>();
        storage.set_loaded(&handle, asset);

        // Emit event
        let version = storage.version(&handle).unwrap_or(1);
        self.events.push(AssetEvent::Created {
            handle: handle.untyped(),
            type_id: TypeId::of::<T>(),
            version,
        });

        Ok(handle)
    }

    /// Insert an already-loaded asset directly.
    pub fn insert<T: Asset>(&mut self, source: AssetSource, asset: T) -> Handle<T> {
        let storage = self.storages.get_or_create::<T>();
        let handle = storage.insert(source, asset);

        // Emit event
        let version = storage.version(&handle).unwrap_or(1);
        self.events.push(AssetEvent::Created {
            handle: handle.untyped(),
            type_id: TypeId::of::<T>(),
            version,
        });

        handle
    }

    /// Get an asset if it's ready.
    pub fn get<T: Asset>(&self, handle: &Handle<T>) -> Option<&Arc<T>> {
        self.storages.get::<T>().and_then(|s| s.get(handle))
    }

    /// Check if an asset is ready.
    pub fn is_ready<T: Asset>(&self, handle: &Handle<T>) -> bool {
        self.storages
            .get::<T>()
            .map(|s| s.is_ready(handle))
            .unwrap_or(false)
    }

    /// Check if an asset is loading.
    pub fn is_loading<T: Asset>(&self, handle: &Handle<T>) -> bool {
        self.storages
            .get::<T>()
            .map(|s| s.is_loading(handle))
            .unwrap_or(false)
    }

    /// Get the version of an asset.
    pub fn version<T: Asset>(&self, handle: &Handle<T>) -> Option<u32> {
        self.storages.get::<T>().and_then(|s| s.version(handle))
    }

    /// Remove an asset by handle.
    pub fn remove<T: Asset>(&mut self, handle: &Handle<T>) {
        if let Some(storage) = self.storages.get_mut::<T>()
            && storage.remove(handle).is_some() {
                self.events.push(AssetEvent::Removed {
                    handle_id: handle.id(),
                    type_id: TypeId::of::<T>(),
                });
            }
    }

    /// Drain all events from this frame.
    pub fn drain_events(&mut self) -> impl Iterator<Item = AssetEvent> + '_ {
        self.events.drain()
    }

    /// Get an iterator over events without draining.
    pub fn iter_events(&self) -> impl Iterator<Item = &AssetEvent> {
        self.events.iter()
    }

    /// Process pending loads (call each frame).
    ///
    /// This processes a batch of pending loads synchronously.
    /// Returns the number of loads processed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn process_pending(&mut self, max_loads: usize) -> usize {
        let mut processed = 0;

        while processed < max_loads {
            let Some(pending) = self.pending.pop_front() else {
                break;
            };

            processed += 1;
            self.process_single_load(pending);
        }

        processed
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn process_single_load(&mut self, pending: PendingLoad) {
        // Read bytes if not already loaded
        let bytes = match pending.bytes {
            Some(b) => b,
            None => {
                let result = match &pending.source {
                    AssetSource::Disk { path, .. } => self.file_reader.read_bytes_sync(path),
                    AssetSource::Memory { key } => {
                        let path = Path::new(key);
                        futures_lite::future::block_on(self.memory_reader.read_bytes(path))
                    }
                    AssetSource::Bytes { data, .. } => Ok(data.to_vec()),
                };

                match result {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        self.events.push(AssetEvent::LoadFailed {
                            handle: pending.handle,
                            type_id: pending.handle.type_id(),
                            error: err.to_string(),
                        });
                        return;
                    }
                }
            }
        };

        // Load using the appropriate loader
        let result = self
            .loaders
            .load(&pending.source, &bytes, pending.extension.as_deref());

        match result {
            Ok(_asset) => {
                // We need to set the asset in storage, but we don't know the type here
                // This is a limitation of the type-erased approach
                // For now, we just emit the event - the typed API handles storage
                self.events.push(AssetEvent::Created {
                    handle: pending.handle,
                    type_id: pending.handle.type_id(),
                    version: 1,
                });
            }
            Err(err) => {
                self.events.push(AssetEvent::LoadFailed {
                    handle: pending.handle,
                    type_id: pending.handle.type_id(),
                    error: err.to_string(),
                });
            }
        }
    }

    /// Get the number of pending loads.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Check if there are any pending loads.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Get direct access to typed storage.
    pub fn assets<T: Asset>(&self) -> Option<&Assets<T>> {
        self.storages.get::<T>()
    }

    /// Get mutable access to typed storage.
    pub fn assets_mut<T: Asset>(&mut self) -> &mut Assets<T> {
        self.storages.get_or_create::<T>()
    }

    /// Wait for an asset to be loaded, returning a reference to it.
    ///
    /// This is a convenience method that checks if an asset is ready,
    /// and if not, blocks until it becomes ready (or fails).
    ///
    /// # Warning
    ///
    /// This will block if used with async loading. For synchronous loading,
    /// the asset should already be ready.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn ensure_loaded<T: Asset>(&mut self, handle: &Handle<T>) -> Option<&Arc<T>> {
        // If already ready, return immediately
        if self.is_ready(handle) {
            return self.get(handle);
        }

        // Process pending loads until this handle is ready
        let max_iterations = 1000;
        for _ in 0..max_iterations {
            if self.is_ready(handle) || !self.is_loading(handle) {
                break;
            }
            self.process_pending(1);
        }

        self.get(handle)
    }

    /// Find an asset handle by its path (for disk sources).
    ///
    /// Returns `None` if no asset with this path has been loaded.
    pub fn find_by_path<T: Asset>(&self, path: impl AsRef<Path>) -> Option<Handle<T>> {
        let source = AssetSource::disk(path);
        self.storages.get::<T>()?.find_by_source(&source)
    }

    /// Drain events for a specific asset type.
    ///
    /// This is useful when you only care about events for one type of asset.
    pub fn drain_events_for<T: Asset>(&mut self) -> impl Iterator<Item = AssetEvent> + '_ {
        let target_type = TypeId::of::<T>();
        self.events.drain().filter(move |e| e.type_id() == target_type)
    }

    /// Get the asset state for a handle.
    pub fn state<T: Asset>(&self, handle: &Handle<T>) -> Option<&AssetState<T>> {
        self.storages.get::<T>()?.state(handle)
    }

    /// Check if a loader is registered for a type and extension.
    pub fn has_loader_for<T: 'static>(&self, extension: &str) -> bool {
        self.loaders.has_loader_for::<T>(extension)
    }

    /// Check if any loader is registered for a type.
    pub fn has_loader_for_type<T: 'static>(&self) -> bool {
        self.loaders.has_loader_for_type::<T>()
    }

    /// Enable hot reload support for a directory.
    ///
    /// When enabled, the asset server will watch the specified directory
    /// for file changes and automatically reload affected assets.
    ///
    /// # Feature Flag
    ///
    /// This method is only available with the `hot-reload` feature enabled.
    #[cfg(feature = "hot-reload")]
    pub fn enable_hot_reload(&mut self, watch_dir: impl AsRef<Path>) -> Result<(), String> {
        use crate::hot_reload::AssetWatcher;

        if self.watcher.is_none() {
            self.watcher = Some(AssetWatcher::new().map_err(|e| e.to_string())?);
        }

        if let Some(watcher) = &mut self.watcher {
            watcher.watch_directory(&watch_dir).map_err(|e| e.to_string())?;
            tracing::info!("Hot reload enabled for directory: {}", watch_dir.as_ref().display());
        }

        Ok(())
    }

    /// Process hot reload events.
    ///
    /// Call this each frame to check for file changes and reload affected assets.
    /// Returns the number of assets that were reloaded.
    ///
    /// # Feature Flag
    ///
    /// This method is only available with the `hot-reload` feature enabled.
    #[cfg(feature = "hot-reload")]
    pub fn process_hot_reload(&mut self) -> usize {
        let Some(watcher) = &mut self.watcher else {
            return 0;
        };

        let changed_handles = watcher.poll_changes();
        if changed_handles.is_empty() {
            return 0;
        }

        tracing::debug!("Hot reload: {} assets changed", changed_handles.len());

        let mut reloaded = 0;
        for handle in changed_handles {
            // Find the source for this handle and queue a reload
            // We need to look through all storages to find the asset
            if let Some(source) = self.storages.find_source(&handle) {
                tracing::debug!("Reloading asset from: {:?}", source);

                // Queue for reload
                self.pending.push_back(PendingLoad {
                    handle,
                    source: source.clone(),
                    bytes: None,
                    extension: source.extension().map(String::from),
                });

                reloaded += 1;
            }
        }

        reloaded
    }

    /// Register a file path with the hot reload system.
    ///
    /// This is called automatically when assets are loaded, but can be called
    /// manually if needed.
    ///
    /// # Feature Flag
    ///
    /// This method is only available with the `hot-reload` feature enabled.
    #[cfg(feature = "hot-reload")]
    pub fn register_hot_reload_path(&mut self, path: impl AsRef<Path>, handle: UntypedHandle) {
        if let Some(watcher) = &mut self.watcher {
            watcher.register_file(path, handle);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for AssetServer {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU task queue for deferred GPU resource creation.
///
/// Assets that need GPU resources (textures, buffers, etc.) can queue
/// creation tasks here to be processed when a GPU context is available.
pub struct GpuTaskQueue {
    /// Pending GPU creation tasks.
    tasks: VecDeque<Box<dyn GpuTask>>,
}

impl Default for GpuTaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuTaskQueue {
    /// Create a new empty task queue.
    pub fn new() -> Self {
        Self {
            tasks: VecDeque::new(),
        }
    }

    /// Queue a GPU task.
    pub fn queue(&mut self, task: impl GpuTask + 'static) {
        self.tasks.push_back(Box::new(task));
    }

    /// Process all pending tasks with the given context.
    pub fn process_all<Ctx>(&mut self, ctx: &Ctx)
    where
        Ctx: GpuContext,
    {
        while let Some(task) = self.tasks.pop_front() {
            task.execute(ctx);
        }
    }

    /// Get the number of pending tasks.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Check if there are any pending tasks.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

/// Trait for GPU resource creation tasks.
pub trait GpuTask: Send + Sync {
    /// Execute the GPU task with the given context.
    fn execute(&self, ctx: &dyn GpuContext);
}

/// Trait for GPU context that can create resources.
///
/// Implement this trait for your rendering backend to enable
/// asset-GPU integration.
pub trait GpuContext: Send + Sync {
    /// Create a texture from raw data.
    fn create_texture(&self, data: &[u8], width: u32, height: u32, format: TextureFormat) -> u64;

    /// Create a buffer with the given data.
    fn create_buffer(&self, data: &[u8], usage: BufferUsage) -> u64;
}

/// Texture format for GPU textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8Unorm,
    Rgba8Srgb,
    Bgra8Unorm,
    Bgra8Srgb,
    R8Unorm,
    Rg8Unorm,
}

/// Buffer usage flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferUsage {
    pub vertex: bool,
    pub index: bool,
    pub uniform: bool,
    pub storage: bool,
    pub copy_src: bool,
    pub copy_dst: bool,
}

impl Default for BufferUsage {
    fn default() -> Self {
        Self {
            vertex: false,
            index: false,
            uniform: false,
            storage: false,
            copy_src: false,
            copy_dst: true,
        }
    }
}

impl BufferUsage {
    pub fn vertex() -> Self {
        Self {
            vertex: true,
            ..Default::default()
        }
    }

    pub fn index() -> Self {
        Self {
            index: true,
            ..Default::default()
        }
    }

    pub fn uniform() -> Self {
        Self {
            uniform: true,
            ..Default::default()
        }
    }

    pub fn storage() -> Self {
        Self {
            storage: true,
            ..Default::default()
        }
    }
}
