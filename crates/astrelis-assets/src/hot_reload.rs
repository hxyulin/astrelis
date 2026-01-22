//! Hot reload support for assets during development.
//!
//! Watches asset files for changes and automatically reloads them.

#[cfg(feature = "hot-reload")]
use std::collections::HashMap;
#[cfg(feature = "hot-reload")]
use std::path::{Path, PathBuf};
#[cfg(feature = "hot-reload")]
use std::sync::mpsc::{channel, Receiver};
#[cfg(feature = "hot-reload")]
use std::time::Duration;

#[cfg(feature = "hot-reload")]
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

#[cfg(feature = "hot-reload")]
use crate::handle::UntypedHandle;

/// File watcher for hot-reloading assets.
///
/// This watches directories for file changes and tracks which files
/// correspond to which asset handles.
#[cfg(feature = "hot-reload")]
pub struct AssetWatcher {
    watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
    /// Maps file paths to asset handles
    path_to_handle: HashMap<PathBuf, Vec<UntypedHandle>>,
    /// Watched directories
    watched_dirs: Vec<PathBuf>,
}

#[cfg(feature = "hot-reload")]
impl AssetWatcher {
    /// Create a new asset watcher.
    pub fn new() -> Result<Self, notify::Error> {
        let (sender, receiver) = channel();

        let watcher = notify::recommended_watcher(move |res| {
            let _ = sender.send(res);
        })?;

        Ok(Self {
            watcher,
            receiver,
            path_to_handle: HashMap::new(),
            watched_dirs: Vec::new(),
        })
    }

    /// Watch a directory for changes.
    pub fn watch_directory(&mut self, path: impl AsRef<Path>) -> Result<(), notify::Error> {
        let path = path.as_ref();

        if !self.watched_dirs.contains(&path.to_path_buf()) {
            self.watcher
                .watch(path, RecursiveMode::Recursive)?;
            self.watched_dirs.push(path.to_path_buf());
            tracing::debug!("Watching directory for changes: {}", path.display());
        }

        Ok(())
    }

    /// Register that a file is associated with an asset handle.
    pub fn register_file(&mut self, path: impl AsRef<Path>, handle: UntypedHandle) {
        let path = path.as_ref().to_path_buf();
        self.path_to_handle
            .entry(path)
            .or_insert_with(Vec::new)
            .push(handle);
    }

    /// Unregister a file from being watched.
    pub fn unregister_file(&mut self, path: impl AsRef<Path>, handle: &UntypedHandle) {
        let path = path.as_ref();
        if let Some(handles) = self.path_to_handle.get_mut(path) {
            handles.retain(|h| h.id() != handle.id());
            if handles.is_empty() {
                self.path_to_handle.remove(path);
            }
        }
    }

    /// Poll for changed files.
    ///
    /// Returns a list of handles that need to be reloaded.
    pub fn poll_changes(&mut self) -> Vec<UntypedHandle> {
        let mut changed_handles = Vec::new();

        // Process all pending events
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                Ok(event) => {
                    // We only care about modify events
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_)
                    ) {
                        for path in &event.paths {
                            if let Some(handles) = self.path_to_handle.get(path) {
                                tracing::debug!("File changed, marking for reload: {}", path.display());
                                changed_handles.extend(handles.iter().copied());
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("File watcher error: {}", e);
                }
            }
        }

        // Deduplicate by handle ID
        changed_handles.sort_by(|a, b| {
            let a_id = a.id();
            let b_id = b.id();
            // Compare by slot index and generation
            (a_id.slot.index(), a_id.slot.generation()).cmp(&(b_id.slot.index(), b_id.slot.generation()))
        });
        changed_handles.dedup_by(|a, b| {
            let a_id = a.id();
            let b_id = b.id();
            a_id.slot.index() == b_id.slot.index() && a_id.slot.generation() == b_id.slot.generation()
        });

        changed_handles
    }

    /// Get the list of watched directories.
    pub fn watched_directories(&self) -> &[PathBuf] {
        &self.watched_dirs
    }
}

#[cfg(feature = "hot-reload")]
impl Default for AssetWatcher {
    fn default() -> Self {
        Self::new().expect("Failed to create asset watcher")
    }
}

#[cfg(not(feature = "hot-reload"))]
/// Dummy type when hot-reload feature is disabled.
pub struct AssetWatcher;

#[cfg(not(feature = "hot-reload"))]
impl AssetWatcher {
    pub fn new() -> Result<Self, String> {
        Err("Hot reload feature not enabled".to_string())
    }
}

#[cfg(all(test, feature = "hot-reload"))]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_asset_watcher_creation() {
        let watcher = AssetWatcher::new();
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_watch_directory() {
        let temp_dir = TempDir::new().unwrap();
        let mut watcher = AssetWatcher::new().unwrap();

        let result = watcher.watch_directory(temp_dir.path());
        assert!(result.is_ok());
        assert_eq!(watcher.watched_directories().len(), 1);
    }

    #[test]
    fn test_watch_same_directory_twice() {
        let temp_dir = TempDir::new().unwrap();
        let mut watcher = AssetWatcher::new().unwrap();

        watcher.watch_directory(temp_dir.path()).unwrap();
        watcher.watch_directory(temp_dir.path()).unwrap();

        // Should only have one entry, not two
        assert_eq!(watcher.watched_directories().len(), 1);
    }

    #[test]
    fn test_register_file() {
        let mut watcher = AssetWatcher::new().unwrap();
        let handle = UntypedHandle::test_handle(0, 1);

        watcher.register_file("/some/path/file.txt", handle);

        // Verify the file is registered
        assert!(watcher.path_to_handle.contains_key(Path::new("/some/path/file.txt")));
    }

    #[test]
    fn test_register_multiple_handles_for_same_file() {
        let mut watcher = AssetWatcher::new().unwrap();
        let handle1 = UntypedHandle::test_handle(0, 1);
        let handle2 = UntypedHandle::test_handle(1, 1);

        watcher.register_file("/path/file.txt", handle1);
        watcher.register_file("/path/file.txt", handle2);

        let handles = watcher.path_to_handle.get(Path::new("/path/file.txt")).unwrap();
        assert_eq!(handles.len(), 2);
    }

    #[test]
    fn test_unregister_file() {
        let mut watcher = AssetWatcher::new().unwrap();
        let handle1 = UntypedHandle::test_handle(0, 1);
        let handle2 = UntypedHandle::test_handle(1, 1);

        watcher.register_file("/path/file.txt", handle1);
        watcher.register_file("/path/file.txt", handle2);

        // Unregister handle1
        watcher.unregister_file("/path/file.txt", &handle1);

        let handles = watcher.path_to_handle.get(Path::new("/path/file.txt")).unwrap();
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].id().slot.index(), 1);
    }

    #[test]
    fn test_unregister_last_handle_removes_path() {
        let mut watcher = AssetWatcher::new().unwrap();
        let handle = UntypedHandle::test_handle(0, 1);

        watcher.register_file("/path/file.txt", handle);
        watcher.unregister_file("/path/file.txt", &handle);

        // Path should be removed when no handles remain
        assert!(!watcher.path_to_handle.contains_key(Path::new("/path/file.txt")));
    }

    #[test]
    fn test_poll_changes_no_events() {
        let mut watcher = AssetWatcher::new().unwrap();
        let changes = watcher.poll_changes();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_poll_changes_deduplicates() {
        let temp_dir = TempDir::new().unwrap();
        let mut watcher = AssetWatcher::new().unwrap();
        watcher.watch_directory(temp_dir.path()).unwrap();

        let file_path = temp_dir.path().join("test.txt");
        let handle = UntypedHandle::test_handle(0, 1);

        // Create the file
        fs::write(&file_path, "initial content").unwrap();

        // Register the file with the watcher
        watcher.register_file(&file_path, handle);

        // Give the watcher time to notice the file
        thread::sleep(Duration::from_millis(100));

        // Drain any initial events
        let _ = watcher.poll_changes();

        // Modify the file multiple times rapidly
        fs::write(&file_path, "modified content 1").unwrap();
        fs::write(&file_path, "modified content 2").unwrap();

        // Give the watcher time to process events
        thread::sleep(Duration::from_millis(200));

        // Poll should return deduped handles
        let changes = watcher.poll_changes();

        // Even with multiple modifications, we should only get one handle back
        // (due to deduplication)
        assert!(changes.len() <= 1, "Expected at most 1 change, got {}", changes.len());
    }
}
