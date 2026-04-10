//! The central asset server that coordinates loading, storage, and events.
//!
//! [`AssetServer`] is the main entry point for the asset system. It manages
//! a background loading thread, a type-erased storage layer, deduplication
//! of load requests, and optional hot-reload via the `hot-reload` feature.
//!
//! # Lifecycle
//!
//! 1. Create an `AssetServer` with [`AssetServer::new`].
//! 2. Register loaders with [`AssetServer::add_loader`].
//! 3. Optionally enable hot-reload with `enable_hot_reload` (requires `hot-reload` feature).
//! 4. Call [`AssetServer::load`] to request assets — returns a [`Handle`] immediately.
//! 5. Call [`AssetServer::update`] once per frame to process completed loads.
//! 6. Call [`AssetServer::get`] to access loaded assets.

use std::any::TypeId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::event::{AssetEvent, LoadState};
use crate::handle::{AssetId, Handle, UntypedHandle};
use crate::loader::{AssetLoader, ErasedAssetLoader, LoaderRegistry};
use crate::storage::StorageMap;
use crate::{Asset, AssetLoadError};

/// A request sent to the background loading thread.
struct LoadRequest {
    id: AssetId,
    type_id: TypeId,
    path: PathBuf,
    loader: Arc<dyn ErasedAssetLoader>,
}

/// A result returned from the background loading thread.
struct LoadResult {
    id: AssetId,
    type_id: TypeId,
    result: Result<Box<dyn std::any::Any + Send + Sync>, String>,
}

/// The central asset management server.
///
/// Coordinates background loading, type-erased storage, deduplication,
/// and optional hot-reload. Setup methods (`add_loader`, `enable_hot_reload`)
/// take `&mut self`; runtime methods (`load`, `get`, `update`) take `&self`
/// and use interior mutability.
pub struct AssetServer {
    asset_dir: PathBuf,
    loaders: LoaderRegistry,
    storage: RwLock<StorageMap>,
    /// Maps canonical path → (AssetId, Weak refcount) for deduplication.
    path_map: RwLock<HashMap<String, PathEntry>>,
    next_index: AtomicU32,
    load_sender: std::sync::mpsc::Sender<LoadRequest>,
    result_receiver: Mutex<std::sync::mpsc::Receiver<LoadResult>>,
    /// Handle to the background thread, joined on drop.
    _worker: Option<std::thread::JoinHandle<()>>,
    #[cfg(feature = "hot-reload")]
    hot_reload: Mutex<Option<HotReloadState>>,
}

/// Entry in the path deduplication map.
struct PathEntry {
    id: AssetId,
    type_id: TypeId,
    ref_count: Weak<()>,
}

#[cfg(feature = "hot-reload")]
struct HotReloadState {
    _watcher: notify::RecommendedWatcher,
    event_receiver: std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
}

impl AssetServer {
    /// Creates a new asset server rooted at the given directory.
    ///
    /// A background thread is spawned to handle file I/O. The thread
    /// exits automatically when the server is dropped.
    pub fn new(asset_dir: impl Into<PathBuf>) -> Self {
        astrelis_profiling::profile_function!();
        let (load_tx, load_rx) = std::sync::mpsc::channel::<LoadRequest>();
        let (result_tx, result_rx) = std::sync::mpsc::channel::<LoadResult>();

        let worker = std::thread::Builder::new()
            .name("asset-loader".into())
            .spawn(move || {
                Self::worker_loop(load_rx, result_tx);
            })
            .expect("failed to spawn asset loader thread");

        Self {
            asset_dir: asset_dir.into(),
            loaders: LoaderRegistry::new(),
            storage: RwLock::new(StorageMap::new()),
            path_map: RwLock::new(HashMap::new()),
            next_index: AtomicU32::new(0),
            load_sender: load_tx,
            result_receiver: Mutex::new(result_rx),
            _worker: Some(worker),
            #[cfg(feature = "hot-reload")]
            hot_reload: Mutex::new(None),
        }
    }

    /// The background worker loop. Reads files and invokes loaders.
    fn worker_loop(
        receiver: std::sync::mpsc::Receiver<LoadRequest>,
        sender: std::sync::mpsc::Sender<LoadResult>,
    ) {
        while let Ok(request) = receiver.recv() {
            let result = match std::fs::read(&request.path) {
                Ok(bytes) => request
                    .loader
                    .load_erased(&bytes, &request.path)
                    .map_err(|e| e.to_string()),
                Err(e) => Err(format!("I/O error reading {}: {e}", request.path.display())),
            };

            let _ = sender.send(LoadResult {
                id: request.id,
                type_id: request.type_id,
                result,
            });
        }
    }

    /// Registers a loader for a given asset type.
    ///
    /// Must be called during setup, before any `load()` calls.
    pub fn add_loader<L: AssetLoader>(&mut self, loader: L) {
        self.loaders.add(loader);
    }

    /// Loads an asset by path, returning a handle immediately.
    ///
    /// The asset loads asynchronously in the background. Use [`Self::update`]
    /// to process completed loads and [`Self::get`] to access the result.
    ///
    /// If the same path has already been loaded (and a strong handle still
    /// exists), the existing handle is returned without re-loading.
    pub fn load<T: Asset>(&self, path: impl AsRef<Path>) -> Handle<T> {
        astrelis_profiling::profile_function!();
        let path = path.as_ref();
        let key = path.to_string_lossy().to_string();

        // Fast path: check for existing handle.
        {
            let map = self.path_map.read().unwrap();
            if let Some(entry) = map.get(&key)
                && entry.type_id == TypeId::of::<T>()
                && let Some(rc) = entry.ref_count.upgrade()
            {
                return Handle::new(entry.id, rc);
            }
        }

        // Slow path: allocate new handle.
        let mut map = self.path_map.write().unwrap();

        // Double-check after acquiring write lock.
        if let Some(entry) = map.get(&key)
            && entry.type_id == TypeId::of::<T>()
            && let Some(rc) = entry.ref_count.upgrade()
        {
            return Handle::new(entry.id, rc);
        }

        let id = self.alloc_id();
        let ref_count = Arc::new(());

        map.insert(
            key,
            PathEntry {
                id,
                type_id: TypeId::of::<T>(),
                ref_count: Arc::downgrade(&ref_count),
            },
        );

        // Insert into storage as Loading.
        {
            let mut storage = self.storage.write().unwrap();
            storage
                .get_or_create::<T>()
                .insert_loading(id, Arc::clone(&ref_count));
        }

        // Find loader and dispatch.
        let full_path = self.asset_dir.join(path);
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if let Some(loader) = self.loaders.find(TypeId::of::<T>(), extension) {
            let _ = self.load_sender.send(LoadRequest {
                id,
                type_id: TypeId::of::<T>(),
                path: full_path,
                loader,
            });
        } else {
            // No loader — mark as failed immediately via the result channel
            // so it gets picked up in the next update().
            let _ = self.load_sender.send(LoadRequest {
                id,
                type_id: TypeId::of::<T>(),
                path: full_path,
                loader: Arc::new(FailLoader {
                    extension: extension.to_string(),
                }),
            });
        }

        Handle::new(id, ref_count)
    }

    /// Loads an asset from raw bytes synchronously (no file I/O).
    ///
    /// The asset is immediately available via [`Self::get`] after this call.
    /// The `label` parameter is used for debugging and error messages.
    pub fn load_from_bytes<T: Asset>(
        &self,
        bytes: &[u8],
        label: &str,
    ) -> Result<Handle<T>, AssetLoadError> {
        astrelis_profiling::profile_function!();
        let extension = Path::new(label)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let loader = self
            .loaders
            .find(TypeId::of::<T>(), extension)
            .ok_or_else(|| AssetLoadError::NoLoader {
                extension: extension.to_string(),
            })?;

        let boxed = loader.load_erased(bytes, Path::new(label))?;
        let arc = *boxed
            .downcast::<Arc<T>>()
            .expect("loader produced wrong type");

        let id = self.alloc_id();
        let ref_count = Arc::new(());

        {
            let mut storage = self.storage.write().unwrap();
            storage
                .get_or_create::<T>()
                .insert_loaded(id, arc, Arc::clone(&ref_count));
        }

        Ok(Handle::new(id, ref_count))
    }

    /// Returns the current load state of the asset behind `handle`.
    pub fn load_state<T: Asset>(&self, handle: &Handle<T>) -> LoadState {
        let storage = self.storage.read().unwrap();
        storage
            .get::<T>()
            .map(|assets| assets.load_state(&handle.id))
            .unwrap_or(LoadState::NotLoaded)
    }

    /// Returns a clone of the loaded asset, if ready.
    ///
    /// Returns `None` if the asset is still loading or failed.
    /// The returned `Arc<T>` is cheap to clone and can be cached locally.
    pub fn get<T: Asset>(&self, handle: &Handle<T>) -> Option<Arc<T>> {
        let storage = self.storage.read().unwrap();
        storage.get::<T>().and_then(|assets| assets.get(&handle.id))
    }

    /// Processes completed loads, cleans up dead handles, and returns events.
    ///
    /// Call this once per frame. It drains all completed background loads,
    /// updates storage, emits events for state changes, and optionally
    /// processes hot-reload file change notifications.
    pub fn update(&self) -> Vec<AssetEvent> {
        astrelis_profiling::profile_function!();
        let mut events = Vec::new();

        // Drain completed loads.
        {
            let receiver = self.result_receiver.lock().unwrap();
            while let Ok(result) = receiver.try_recv() {
                let mut storage = self.storage.write().unwrap();
                if let Some(erased) = storage.get_erased_mut(&result.type_id) {
                    match result.result {
                        Ok(asset) => {
                            erased.set_loaded(&result.id, asset);
                            // Build an UntypedHandle for the event.
                            // We need to get the ref_count from the entry — it's already stored.
                            // For the event, we can create a lightweight handle.
                            events.push(AssetEvent::Created {
                                handle: self.make_event_handle(result.id, result.type_id),
                            });
                        }
                        Err(error) => {
                            erased.set_failed(&result.id, error.clone());
                            events.push(AssetEvent::Failed {
                                handle: self.make_event_handle(result.id, result.type_id),
                                error,
                            });
                        }
                    }
                }
            }
        }

        // Hot-reload processing.
        #[cfg(feature = "hot-reload")]
        self.process_hot_reload(&mut events);

        events
    }

    /// Constructs an [`UntypedHandle`] for event emission.
    ///
    /// Uses a temporary `Arc<()>` since events just need the ID and type.
    fn make_event_handle(&self, id: AssetId, type_id: TypeId) -> UntypedHandle {
        UntypedHandle {
            id,
            type_id,
            ref_count: Arc::new(()),
        }
    }

    /// Allocates a new unique [`AssetId`].
    fn alloc_id(&self) -> AssetId {
        let index = self.next_index.fetch_add(1, Ordering::Relaxed);
        AssetId {
            index,
            generation: 1,
        }
    }

    /// Enables hot-reload file watching on the asset directory.
    ///
    /// When enabled, file changes are detected during [`Self::update`] and
    /// assets are automatically reloaded. Requires the `hot-reload` feature.
    #[cfg(feature = "hot-reload")]
    pub fn enable_hot_reload(&mut self) -> Result<(), AssetLoadError> {
        use notify::{RecursiveMode, Watcher};

        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(tx).map_err(|e| AssetLoadError::Io(e.to_string()))?;

        watcher
            .watch(&self.asset_dir, RecursiveMode::Recursive)
            .map_err(|e| AssetLoadError::Io(e.to_string()))?;

        *self.hot_reload.lock().unwrap() = Some(HotReloadState {
            _watcher: watcher,
            event_receiver: rx,
        });

        Ok(())
    }

    /// Processes hot-reload file events and queues reloads.
    #[cfg(feature = "hot-reload")]
    fn process_hot_reload(&self, events: &mut Vec<AssetEvent>) {
        use notify::EventKind;

        let hot_reload = self.hot_reload.lock().unwrap();
        let Some(state) = hot_reload.as_ref() else {
            return;
        };

        let mut reload_paths = Vec::new();
        while let Ok(event_result) = state.event_receiver.try_recv() {
            if let Ok(event) = event_result {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        for path in event.paths {
                            reload_paths.push(path);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Drop the hot_reload lock before acquiring path_map lock.
        drop(hot_reload);

        for full_path in reload_paths {
            // Convert absolute path back to relative key.
            let key = full_path
                .strip_prefix(&self.asset_dir)
                .unwrap_or(&full_path)
                .to_string_lossy()
                .to_string();

            let map = self.path_map.read().unwrap();
            if let Some(entry) = map.get(&key) {
                if let Some(rc) = entry.ref_count.upgrade() {
                    let id = entry.id;
                    let type_id = entry.type_id;
                    let extension = Path::new(&key)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");

                    if let Some(loader) = self.loaders.find(type_id, extension) {
                        // Re-read and reload synchronously for hot-reload.
                        if let Ok(bytes) = std::fs::read(&full_path) {
                            match loader.load_erased(&bytes, &full_path) {
                                Ok(asset) => {
                                    let mut storage = self.storage.write().unwrap();
                                    if let Some(erased) = storage.get_erased_mut(&type_id) {
                                        erased.set_loaded(&id, asset);
                                    }
                                    events.push(AssetEvent::Modified {
                                        handle: UntypedHandle {
                                            id,
                                            type_id,
                                            ref_count: rc,
                                        },
                                    });
                                }
                                Err(e) => {
                                    events.push(AssetEvent::Failed {
                                        handle: self.make_event_handle(id, type_id),
                                        error: e.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Drop for AssetServer {
    fn drop(&mut self) {
        // Dropping load_sender closes the channel, causing the worker to exit.
        // The worker handle is dropped automatically, but we don't join here
        // to avoid blocking on drop.
    }
}

/// A dummy loader used when no real loader matches — always returns an error.
struct FailLoader {
    extension: String,
}

impl ErasedAssetLoader for FailLoader {
    fn load_erased(
        &self,
        _bytes: &[u8],
        _path: &Path,
    ) -> Result<Box<dyn std::any::Any + Send + Sync>, AssetLoadError> {
        Err(AssetLoadError::NoLoader {
            extension: self.extension.clone(),
        })
    }
}

// SAFETY: All interior fields are individually Send+Sync.
// - RwLock<StorageMap>, RwLock<HashMap>, Mutex<Receiver> are Send+Sync.
// - mpsc::Sender is Send+Sync.
// - AtomicU32 is Send+Sync.
unsafe impl Send for AssetServer {}
unsafe impl Sync for AssetServer {}
