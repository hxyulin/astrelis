//! Integration tests for the asset system.

use std::path::Path;
use std::sync::Arc;

use astrelis_assets::{
    Asset, AssetEvent, AssetLoadError, AssetLoader, AssetServer, Handle, LoadState,
};

// -- Test asset + loader ------------------------------------------------

struct TextAsset {
    content: String,
}

impl Asset for TextAsset {
    fn type_name() -> &'static str {
        "TextAsset"
    }
}

struct TextLoader;

impl AssetLoader for TextLoader {
    type Asset = TextAsset;

    fn extensions(&self) -> &[&str] {
        &["txt"]
    }

    fn load(&self, bytes: &[u8], _path: &Path) -> Result<Self::Asset, AssetLoadError> {
        let content =
            String::from_utf8(bytes.to_vec()).map_err(|e| AssetLoadError::Parse(e.to_string()))?;
        Ok(TextAsset { content })
    }
}

// -- Helpers -------------------------------------------------------------

/// Creates a temp directory with a test file and an AssetServer pointed at it.
fn setup() -> (tempfile::TempDir, AssetServer) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "Hello, world!").unwrap();
    std::fs::write(dir.path().join("other.txt"), "Other content").unwrap();

    let mut server = AssetServer::new(dir.path());
    server.add_loader(TextLoader);

    (dir, server)
}

/// Polls `update()` until the predicate returns true, or panics after a timeout.
fn poll_until(server: &AssetServer, mut predicate: impl FnMut(&[AssetEvent]) -> bool) {
    let start = std::time::Instant::now();
    loop {
        let events = server.update();
        if predicate(&events) {
            return;
        }
        if start.elapsed() > std::time::Duration::from_secs(5) {
            panic!("poll_until timed out after 5 seconds");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

// -- Tests ---------------------------------------------------------------

#[test]
fn load_from_bytes_returns_valid_handle() {
    let (_dir, server) = setup();

    let handle = server
        .load_from_bytes::<TextAsset>(b"inline content", "test.txt")
        .unwrap();

    let asset = server.get(&handle).unwrap();
    assert_eq!(asset.content, "inline content");
    assert_eq!(server.load_state(&handle), LoadState::Loaded);
}

#[test]
fn async_load_produces_created_event() {
    let (_dir, server) = setup();

    let handle: Handle<TextAsset> = server.load("hello.txt");

    // Initially should be loading.
    assert!(matches!(
        server.load_state(&handle),
        LoadState::Loading
    ));

    // Poll until we see a Created event.
    poll_until(&server, |events| {
        events.iter().any(|e| matches!(e, AssetEvent::Created { .. }))
    });

    // Asset should now be loaded.
    assert_eq!(server.load_state(&handle), LoadState::Loaded);
    let asset = server.get(&handle).unwrap();
    assert_eq!(asset.content, "Hello, world!");
}

#[test]
fn deduplication_returns_same_id() {
    let (_dir, server) = setup();

    let handle1: Handle<TextAsset> = server.load("hello.txt");
    let handle2: Handle<TextAsset> = server.load("hello.txt");

    assert_eq!(handle1, handle2);
}

#[test]
fn weak_handle_upgrade_fails_after_all_strong_dropped() {
    let (_dir, server) = setup();

    let handle = server
        .load_from_bytes::<TextAsset>(b"data", "test.txt")
        .unwrap();
    let weak = handle.downgrade();

    assert!(weak.upgrade().is_some());

    // The server storage also holds a strong Arc<()>, so the weak
    // remains upgradable until the server itself is dropped.
    drop(handle);

    // Server still alive — weak can upgrade because storage holds a ref.
    // This is by design: liveness is tracked via the refcount.
    // Dropping the server releases the storage ref.
    drop(server);
    assert!(weak.upgrade().is_none());
}

#[test]
fn load_nonexistent_file_produces_failed_event() {
    let (_dir, server) = setup();

    let handle: Handle<TextAsset> = server.load("nonexistent.txt");

    poll_until(&server, |events| {
        events.iter().any(|e| matches!(e, AssetEvent::Failed { .. }))
    });

    assert!(matches!(server.load_state(&handle), LoadState::Failed(_)));
}

#[test]
fn load_state_transitions() {
    let (_dir, server) = setup();

    let handle: Handle<TextAsset> = server.load("hello.txt");

    // Should start as Loading.
    let initial = server.load_state(&handle);
    assert!(
        matches!(initial, LoadState::Loading),
        "expected Loading, got {initial:?}"
    );

    // After update processes the load, should be Loaded.
    poll_until(&server, |events| {
        events.iter().any(|e| matches!(e, AssetEvent::Created { .. }))
    });

    assert_eq!(server.load_state(&handle), LoadState::Loaded);
}

#[test]
fn load_from_bytes_no_loader_returns_error() {
    let (_dir, server) = setup();

    let result = server.load_from_bytes::<TextAsset>(b"data", "test.unknown");
    assert!(result.is_err());

    if let Err(AssetLoadError::NoLoader { extension }) = result {
        assert_eq!(extension, "unknown");
    } else {
        panic!("expected NoLoader error");
    }
}

#[test]
fn multiple_loads_of_different_files() {
    let (_dir, server) = setup();

    let h1: Handle<TextAsset> = server.load("hello.txt");
    let h2: Handle<TextAsset> = server.load("other.txt");

    // Different handles for different files.
    assert_ne!(h1, h2);

    // Wait for both to load.
    let mut created_count = 0;
    poll_until(&server, |events| {
        created_count += events
            .iter()
            .filter(|e| matches!(e, AssetEvent::Created { .. }))
            .count();
        created_count >= 2
    });

    let a1 = server.get(&h1).unwrap();
    let a2 = server.get(&h2).unwrap();
    assert_eq!(a1.content, "Hello, world!");
    assert_eq!(a2.content, "Other content");
}

#[test]
fn handle_clone_shares_identity() {
    let (_dir, server) = setup();

    let handle = server
        .load_from_bytes::<TextAsset>(b"test", "a.txt")
        .unwrap();
    let cloned = handle.clone();

    // Both point to the same asset.
    assert_eq!(handle, cloned);
    let a1 = server.get(&handle).unwrap();
    let a2 = server.get(&cloned).unwrap();
    assert!(Arc::ptr_eq(&a1, &a2));
}

#[test]
fn untyped_handle_preserves_type_id() {
    let (_dir, server) = setup();

    let handle = server
        .load_from_bytes::<TextAsset>(b"test", "a.txt")
        .unwrap();
    let untyped = handle.untyped();

    assert_eq!(
        untyped.asset_type_id(),
        std::any::TypeId::of::<TextAsset>()
    );

    // Roundtrip back to typed.
    let typed = untyped.typed::<TextAsset>();
    assert!(typed.is_some());
}
