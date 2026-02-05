//! Integration tests for the asset system.
//!
//! These tests use tempfile to create isolated test environments.

use std::io::Write;
use std::sync::Arc;
use std::thread;

use astrelis_assets::*;

// ============================================================================
// Test Asset Types
// ============================================================================

/// A simple test config asset.
#[derive(Debug, Clone, PartialEq)]
struct TestConfig {
    name: String,
    value: i32,
}

impl Asset for TestConfig {
    fn type_name() -> &'static str {
        "TestConfig"
    }
}

/// Loader for test config files (simple "name:value" format).
struct TestConfigLoader;

impl AssetLoader for TestConfigLoader {
    type Asset = TestConfig;

    fn extensions(&self) -> &[&str] {
        &["cfg", "config"]
    }

    fn load(&self, ctx: LoadContext<'_>) -> Result<Self::Asset, AssetError> {
        let text = std::str::from_utf8(ctx.bytes).map_err(|e| AssetError::LoaderError {
            path: ctx.source.display_path(),
            message: format!("Invalid UTF-8: {}", e),
        })?;

        let mut name = String::new();
        let mut value = 0;

        for line in text.lines() {
            let line = line.trim();
            if let Some((key, val)) = line.split_once(':') {
                match key.trim() {
                    "name" => name = val.trim().to_string(),
                    "value" => value = val.trim().parse().unwrap_or(0),
                    _ => {}
                }
            }
        }

        Ok(TestConfig { name, value })
    }
}

/// A binary data asset for testing.
#[derive(Debug, Clone, PartialEq)]
struct BinaryData {
    header: u32,
    payload: Vec<u8>,
}

impl Asset for BinaryData {
    fn type_name() -> &'static str {
        "BinaryData"
    }
}

/// Loader for binary data.
struct BinaryDataLoader;

impl AssetLoader for BinaryDataLoader {
    type Asset = BinaryData;

    fn extensions(&self) -> &[&str] {
        &["bin", "dat"]
    }

    fn load(&self, ctx: LoadContext<'_>) -> Result<Self::Asset, AssetError> {
        if ctx.bytes.len() < 4 {
            return Err(AssetError::LoaderError {
                path: ctx.source.display_path(),
                message: "Binary data too small".to_string(),
            });
        }

        let header = u32::from_le_bytes([ctx.bytes[0], ctx.bytes[1], ctx.bytes[2], ctx.bytes[3]]);
        let payload = ctx.bytes[4..].to_vec();

        Ok(BinaryData { header, payload })
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_server(path: &std::path::Path) -> AssetServer {
    let mut server = AssetServer::with_base_path(path);
    server.register_loader(TestConfigLoader);
    server.register_loader(TextLoader);
    // Note: We register BinaryDataLoader AFTER BytesLoader so it takes priority for .bin/.dat
    server.register_loader(BytesLoader);
    server.register_loader(BinaryDataLoader);
    server
}

fn write_config_file(path: &std::path::Path, name: &str, value: i32) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    writeln!(file, "name: {}", name)?;
    writeln!(file, "value: {}", value)?;
    Ok(())
}

fn write_binary_file(path: &std::path::Path, header: u32, payload: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(&header.to_le_bytes())?;
    file.write_all(payload)?;
    Ok(())
}

// ============================================================================
// Basic Loading Tests
// ============================================================================

#[test]
fn test_load_text_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    std::fs::write(&file_path, "Hello, World!").unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<String> = server.load_sync("test.txt").unwrap();

    assert!(server.is_ready(&handle));
    let text = server.get(&handle).unwrap();
    assert_eq!(**text, "Hello, World!");
}

#[test]
fn test_load_config_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("game.cfg");

    write_config_file(&file_path, "TestGame", 42).unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<TestConfig> = server.load_sync("game.cfg").unwrap();

    assert!(server.is_ready(&handle));
    let config = server.get(&handle).unwrap();
    assert_eq!(config.name, "TestGame");
    assert_eq!(config.value, 42);
}

#[test]
fn test_load_binary_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("data.bin");

    write_binary_file(&file_path, 0xDEADBEEF, &[1, 2, 3, 4, 5]).unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<BinaryData> = server.load_sync("data.bin").unwrap();

    assert!(server.is_ready(&handle));
    let data = server.get(&handle).unwrap();
    assert_eq!(data.header, 0xDEADBEEF);
    assert_eq!(data.payload, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_load_nonexistent_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut server = create_test_server(temp_dir.path());

    let result: Result<Handle<String>, _> = server.load_sync("nonexistent.txt");
    assert!(result.is_err());
}

// ============================================================================
// Handle and Storage Tests
// ============================================================================

#[test]
fn test_handle_deduplication() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    std::fs::write(&file_path, "Content").unwrap();

    let mut server = create_test_server(temp_dir.path());

    // Load the same file twice
    let handle1: Handle<String> = server.load_sync("test.txt").unwrap();
    let handle2: Handle<String> = server.load_sync("test.txt").unwrap();

    // Should return the same handle
    assert_eq!(handle1.id(), handle2.id());
}

#[test]
fn test_handle_copy() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    std::fs::write(&file_path, "Content").unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<String> = server.load_sync("test.txt").unwrap();

    // Handles should be Copy
    let handle_copy = handle;
    assert_eq!(handle.id(), handle_copy.id());

    // Both should work
    assert!(server.is_ready(&handle));
    assert!(server.is_ready(&handle_copy));
}

#[test]
fn test_untyped_handle_conversion() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    std::fs::write(&file_path, "Content").unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<String> = server.load_sync("test.txt").unwrap();

    // Convert to untyped
    let untyped = handle.untyped();
    assert_eq!(untyped.id(), handle.id());

    // Convert back to typed
    let typed: Option<Handle<String>> = untyped.typed();
    assert!(typed.is_some());
    assert_eq!(typed.unwrap().id(), handle.id());

    // Wrong type should return None
    let wrong: Option<Handle<TestConfig>> = untyped.typed();
    assert!(wrong.is_none());
}

#[test]
fn test_asset_removal() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    std::fs::write(&file_path, "Content").unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<String> = server.load_sync("test.txt").unwrap();

    assert!(server.is_ready(&handle));

    // Remove the asset
    server.remove(&handle);

    // Should no longer be accessible
    assert!(!server.is_ready(&handle));
    assert!(server.get(&handle).is_none());
}

// ============================================================================
// Version and Hot Reload Tests
// ============================================================================

#[test]
fn test_version_tracking() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.cfg");
    write_config_file(&file_path, "Initial", 1).unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<TestConfig> = server.load_sync("test.cfg").unwrap();

    let version1 = server.version(&handle).unwrap();
    assert!(version1 > 0);

    // Reload should increment version
    // For now, we simulate this by directly manipulating storage
    {
        let storage = server.assets_mut::<TestConfig>();
        storage.set_loaded(
            &handle,
            TestConfig {
                name: "Updated".to_string(),
                value: 2,
            },
        );
    }

    let version2 = server.version(&handle).unwrap();
    assert!(version2 > version1);
}

#[test]
fn test_hot_reload_simulation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("config.cfg");

    // Create initial file
    write_config_file(&file_path, "Version1", 100).unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<TestConfig> = server.load_sync("config.cfg").unwrap();

    // Verify initial load
    {
        let config = server.get(&handle).unwrap();
        assert_eq!(config.name, "Version1");
        assert_eq!(config.value, 100);
    }

    let initial_version = server.version(&handle).unwrap();

    // Simulate file change by modifying the file
    write_config_file(&file_path, "Version2", 200).unwrap();

    // In a real hot-reload scenario, a file watcher would detect this
    // and trigger a reload. Here we manually reload.

    // Read the new bytes and reload
    let new_bytes = std::fs::read(&file_path).unwrap();
    let source = AssetSource::disk("config.cfg");
    let ctx = LoadContext::new(&source, &new_bytes, Some("cfg"));
    let new_config = TestConfigLoader.load(ctx).unwrap();

    // Update the asset in storage
    {
        let storage = server.assets_mut::<TestConfig>();
        storage.set_loaded(&handle, new_config);
    }

    // Verify the update
    {
        let config = server.get(&handle).unwrap();
        assert_eq!(config.name, "Version2");
        assert_eq!(config.value, 200);
    }

    let new_version = server.version(&handle).unwrap();
    assert!(new_version > initial_version);
}

#[test]
fn test_tracked_handle_change_detection() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.cfg");
    write_config_file(&file_path, "Initial", 1).unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<TestConfig> = server.load_sync("test.cfg").unwrap();

    // Create a tracked handle
    let mut tracked = TrackedHandle::new(handle);

    // First check should show change (version > 0)
    let current_version = server.version(&handle).unwrap();
    assert!(tracked.check_changed(current_version));

    // Second check with same version should not show change
    assert!(!tracked.check_changed(current_version));

    // Update the asset
    {
        let storage = server.assets_mut::<TestConfig>();
        storage.set_loaded(
            &handle,
            TestConfig {
                name: "Updated".to_string(),
                value: 2,
            },
        );
    }

    // Now should show change again
    let new_version = server.version(&handle).unwrap();
    assert!(tracked.check_changed(new_version));
}

// ============================================================================
// Event System Tests
// ============================================================================

#[test]
fn test_creation_events() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    std::fs::write(&file_path, "Content").unwrap();

    let mut server = create_test_server(temp_dir.path());
    let _handle: Handle<String> = server.load_sync("test.txt").unwrap();

    // Should have a creation event
    let events: Vec<_> = server.drain_events().collect();
    assert!(!events.is_empty());

    let created_count = events.iter().filter(|e| e.is_created()).count();
    assert_eq!(created_count, 1);
}

#[test]
fn test_removal_events() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    std::fs::write(&file_path, "Content").unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<String> = server.load_sync("test.txt").unwrap();

    // Clear creation events
    let _ = server.drain_events().count();

    // Remove the asset
    server.remove(&handle);

    // Should have a removal event
    let events: Vec<_> = server.drain_events().collect();
    let removed_count = events.iter().filter(|e| e.is_removed()).count();
    assert_eq!(removed_count, 1);
}

#[test]
fn test_event_type_filtering() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create multiple files of different types
    std::fs::write(temp_dir.path().join("text.txt"), "Text").unwrap();
    write_config_file(&temp_dir.path().join("config.cfg"), "Config", 1).unwrap();

    let mut server = create_test_server(temp_dir.path());
    let _text: Handle<String> = server.load_sync("text.txt").unwrap();
    let _config: Handle<TestConfig> = server.load_sync("config.cfg").unwrap();

    let events: Vec<_> = server.drain_events().collect();

    // Filter by type
    let filter = TypedEventFilter::<String>::new();
    let string_events: Vec<_> = filter.filter(events.iter()).collect();
    assert_eq!(string_events.len(), 1);

    let filter = TypedEventFilter::<TestConfig>::new();
    let config_events: Vec<_> = filter.filter(events.iter()).collect();
    assert_eq!(config_events.len(), 1);
}

// ============================================================================
// Direct Insert Tests
// ============================================================================

#[test]
fn test_direct_insert() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut server = create_test_server(temp_dir.path());

    let config = TestConfig {
        name: "Inline".to_string(),
        value: 999,
    };

    let handle = server.insert(AssetSource::memory("inline://config"), config);

    assert!(server.is_ready(&handle));
    let stored = server.get(&handle).unwrap();
    assert_eq!(stored.name, "Inline");
    assert_eq!(stored.value, 999);
}

#[test]
fn test_insert_from_bytes() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut server = create_test_server(temp_dir.path());

    let data = BinaryData {
        header: 0x12345678,
        payload: vec![10, 20, 30],
    };

    let bytes: Vec<u8> = {
        let mut v = data.header.to_le_bytes().to_vec();
        v.extend(&data.payload);
        v
    };

    let bytes_arc: Arc<[u8]> = bytes.into();
    let handle = server.insert(AssetSource::bytes("test-data", bytes_arc), data.clone());

    assert!(server.is_ready(&handle));
    let stored = server.get(&handle).unwrap();
    assert_eq!(stored.header, data.header);
    assert_eq!(stored.payload, data.payload);
}

// ============================================================================
// Concurrency Tests
// ============================================================================

#[test]
fn test_concurrent_reads() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("shared.txt");
    std::fs::write(&file_path, "Shared Content").unwrap();

    let server = std::sync::Arc::new(std::sync::Mutex::new(create_test_server(temp_dir.path())));

    // Load the asset first
    let handle: Handle<String> = {
        let mut server = server.lock().unwrap();
        server.load_sync("shared.txt").unwrap()
    };

    // Spawn multiple reader threads
    let mut handles = vec![];
    for _ in 0..4 {
        let server = Arc::clone(&server);
        let handle = handle;
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let server = server.lock().unwrap();
                let text = server.get(&handle).unwrap();
                assert_eq!(**text, "Shared Content");
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn test_concurrent_loads() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create multiple files
    for i in 0..10 {
        let path = temp_dir.path().join(format!("file{}.txt", i));
        std::fs::write(&path, format!("Content {}", i)).unwrap();
    }

    let server = Arc::new(std::sync::Mutex::new(create_test_server(temp_dir.path())));

    // Spawn threads to load different files
    let mut handles = vec![];
    for i in 0..10 {
        let server = Arc::clone(&server);
        handles.push(thread::spawn(move || {
            let handle: Handle<String> = {
                let mut server = server.lock().unwrap();
                server.load_sync(&format!("file{}.txt", i)).unwrap()
            };

            let server = server.lock().unwrap();
            let text = server.get(&handle).unwrap();
            assert_eq!(**text, format!("Content {}", i));
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify all assets are loaded
    let server = server.lock().unwrap();
    let storage = server.assets::<String>().unwrap();
    assert_eq!(storage.len(), 10);
}

// ============================================================================
// Source Type Tests
// ============================================================================

#[test]
fn test_disk_source() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("disk.txt");
    std::fs::write(&file_path, "Disk Content").unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<String> = server
        .load_from_source_sync(AssetSource::disk(file_path))
        .unwrap();

    let text = server.get(&handle).unwrap();
    assert_eq!(**text, "Disk Content");
}

#[test]
fn test_memory_source() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut server = create_test_server(temp_dir.path());

    // Add data to memory reader
    server.add_embedded("embedded.txt", b"Embedded Content");

    // Memory source loading isn't directly supported via load_sync yet,
    // but we can test direct insert with memory source
    let handle = server.insert(
        AssetSource::memory("test://memory"),
        "Memory Content".to_string(),
    );

    let text = server.get(&handle).unwrap();
    assert_eq!(**text, "Memory Content");
}

#[test]
fn test_bytes_source() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut server = create_test_server(temp_dir.path());

    let data: Arc<[u8]> = b"Bytes Content".to_vec().into();
    let handle = server.insert(
        AssetSource::bytes("bytes-content", data),
        "Bytes Content".to_string(),
    );

    let text = server.get(&handle).unwrap();
    assert_eq!(**text, "Bytes Content");
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_loader_error_handling() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("invalid.bin");

    // Create a file that's too small for the binary loader
    std::fs::write(&file_path, &[1, 2]).unwrap();

    let mut server = create_test_server(temp_dir.path());
    let result: Result<Handle<BinaryData>, _> = server.load_sync("invalid.bin");

    assert!(result.is_err());
    match result.unwrap_err() {
        AssetError::LoaderError { message, .. } => {
            assert!(message.contains("too small"));
        }
        other => panic!("Expected LoaderError, got {:?}", other),
    }
}

#[test]
fn test_no_loader_for_extension() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.unknown");
    std::fs::write(&file_path, "Content").unwrap();

    let mut server = create_test_server(temp_dir.path());
    let result: Result<Handle<String>, _> = server.load_sync("test.unknown");

    assert!(result.is_err());
}

// ============================================================================
// Storage Statistics Tests
// ============================================================================

#[test]
fn test_storage_len() {
    let temp_dir = tempfile::tempdir().unwrap();

    for i in 0..5 {
        let path = temp_dir.path().join(format!("file{}.txt", i));
        std::fs::write(&path, format!("Content {}", i)).unwrap();
    }

    let mut server = create_test_server(temp_dir.path());

    for i in 0..5 {
        let _: Handle<String> = server.load_sync(&format!("file{}.txt", i)).unwrap();
    }

    let storage = server.assets::<String>().unwrap();
    assert_eq!(storage.len(), 5);
    assert!(!storage.is_empty());
}

#[test]
fn test_storage_iteration() {
    let temp_dir = tempfile::tempdir().unwrap();

    for i in 0..3 {
        let path = temp_dir.path().join(format!("item{}.txt", i));
        std::fs::write(&path, format!("Item {}", i)).unwrap();
    }

    let mut server = create_test_server(temp_dir.path());

    for i in 0..3 {
        let _: Handle<String> = server.load_sync(&format!("item{}.txt", i)).unwrap();
    }

    let storage = server.assets::<String>().unwrap();
    let entries: Vec<_> = storage.iter().collect();
    assert_eq!(entries.len(), 3);

    // All should be ready
    for entry in entries {
        assert!(entry.is_ready());
    }
}

// ============================================================================
// New Helper Methods Tests
// ============================================================================

#[test]
fn test_find_by_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("findme.txt");
    std::fs::write(&file_path, "Found!").unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<String> = server.load_sync("findme.txt").unwrap();

    // Should find the handle by path
    let found = server.find_by_path::<String>("findme.txt");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id(), handle.id());

    // Should not find non-existent path
    let not_found = server.find_by_path::<String>("nonexistent.txt");
    assert!(not_found.is_none());
}

#[test]
fn test_has_loader_for() {
    let temp_dir = tempfile::tempdir().unwrap();
    let server = create_test_server(temp_dir.path());

    // TextLoader handles txt files
    assert!(server.has_loader_for::<String>("txt"));
    assert!(server.has_loader_for_type::<String>());

    // No loader for unknown extension/type
    assert!(!server.has_loader_for::<String>("xyz"));
}

#[test]
fn test_handle_debug_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("debug_test.txt");
    std::fs::write(&file_path, "Debug!").unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<String> = server.load_sync("debug_test.txt").unwrap();

    // Debug output should include type name
    let debug_str = format!("{:?}", handle);
    assert!(debug_str.contains("Handle"));
    assert!(debug_str.contains("String")); // Type name
    assert!(debug_str.contains("index"));
    assert!(debug_str.contains("generation"));
}

#[test]
fn test_get_asset_state() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("state_test.txt");
    std::fs::write(&file_path, "State!").unwrap();

    let mut server = create_test_server(temp_dir.path());
    let handle: Handle<String> = server.load_sync("state_test.txt").unwrap();

    let state = server.state(&handle);
    assert!(state.is_some());
    assert!(state.unwrap().is_ready());
}

#[test]
fn test_drain_events_for_type() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create files for different types
    std::fs::write(temp_dir.path().join("text.txt"), "text").unwrap();
    let bin_data: Vec<u8> = vec![0, 0, 0, 10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    std::fs::write(temp_dir.path().join("data.bin"), &bin_data).unwrap();

    let mut server = create_test_server(temp_dir.path());

    // Load assets of different types
    let _text: Handle<String> = server.load_sync("text.txt").unwrap();
    let _data: Handle<BinaryData> = server.load_sync("data.bin").unwrap();

    // Drain only String events
    let string_events: Vec<_> = server.drain_events_for::<String>().collect();
    assert_eq!(string_events.len(), 1);

    // BinaryData events should still be in the buffer...
    // Actually, drain_events_for drains from the buffer filtering in place.
    // After draining for String, we should have BinaryData events remaining
}
