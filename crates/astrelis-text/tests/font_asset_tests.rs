//! Integration tests for font asset loading.

use std::path::Path;
use std::sync::Arc;

use astrelis_assets::{AssetServer, AssetSource, Handle};
use astrelis_text::{FontAsset, FontDatabase, FontFormat, FontLoader};

/// Helper to create a test asset server with FontLoader registered.
fn create_font_server(base_path: impl AsRef<Path>) -> AssetServer {
    let mut server = AssetServer::with_base_path(base_path);
    server.register_loader(FontLoader);
    server
}

/// Create a minimal valid TrueType font file header.
/// This is just the header - not a complete font, but enough to test loading.
fn create_mock_ttf() -> Vec<u8> {
    // TrueType header: version (0x00010000) + numTables (1) + searchRange + entrySelector + rangeShift
    vec![
        0x00, 0x01, 0x00, 0x00, // sfntVersion
        0x00, 0x01, // numTables
        0x00, 0x10, // searchRange
        0x00, 0x00, // entrySelector
        0x00, 0x10, // rangeShift
        // Table record (placeholder)
        b'h', b'e', b'a', b'd', // tag
        0x00, 0x00, 0x00, 0x00, // checksum
        0x00, 0x00, 0x00, 0x1C, // offset
        0x00, 0x00, 0x00, 0x10, // length
    ]
}

/// Create a minimal OpenType font file header.
fn create_mock_otf() -> Vec<u8> {
    // OpenType header: 'OTTO' + numTables (1) + searchRange + entrySelector + rangeShift
    vec![
        b'O', b'T', b'T', b'O', // sfntVersion
        0x00, 0x01, // numTables
        0x00, 0x10, // searchRange
        0x00, 0x00, // entrySelector
        0x00, 0x10, // rangeShift
    ]
}

/// Create a minimal WOFF file header.
fn create_mock_woff() -> Vec<u8> {
    vec![
        b'w', b'O', b'F', b'F', // signature
        0x00, 0x01, 0x00, 0x00, // flavor (TrueType)
        0x00, 0x00, 0x00, 0x2C, // length
        0x00, 0x01, // numTables
        0x00, 0x00, // reserved
        0x00, 0x00, 0x00, 0x00, // totalSfntSize
        0x00, 0x01, // majorVersion
        0x00, 0x00, // minorVersion
        0x00, 0x00, 0x00, 0x00, // metaOffset
        0x00, 0x00, 0x00, 0x00, // metaLength
        0x00, 0x00, 0x00, 0x00, // metaOrigLength
        0x00, 0x00, 0x00, 0x00, // privOffset
        0x00, 0x00, 0x00, 0x00, // privLength
    ]
}

// ============================================================================
// Basic Loading Tests
// ============================================================================

#[test]
fn test_load_ttf_font() {
    let temp_dir = tempfile::tempdir().unwrap();
    let font_path = temp_dir.path().join("test.ttf");
    let mock_data = create_mock_ttf();
    std::fs::write(&font_path, &mock_data).unwrap();

    let mut server = create_font_server(temp_dir.path());
    let handle: Handle<FontAsset> = server.load_sync("test.ttf").unwrap();

    assert!(server.is_ready(&handle));

    let font = server.get(&handle).unwrap();
    assert_eq!(font.name(), "test.ttf");
    assert_eq!(font.format(), FontFormat::TrueType);
    assert_eq!(font.data(), &mock_data[..]);
}

#[test]
fn test_load_otf_font() {
    let temp_dir = tempfile::tempdir().unwrap();
    let font_path = temp_dir.path().join("test.otf");
    let mock_data = create_mock_otf();
    std::fs::write(&font_path, &mock_data).unwrap();

    let mut server = create_font_server(temp_dir.path());
    let handle: Handle<FontAsset> = server.load_sync("test.otf").unwrap();

    let font = server.get(&handle).unwrap();
    assert_eq!(font.name(), "test.otf");
    assert_eq!(font.format(), FontFormat::OpenType);
}

#[test]
fn test_load_woff_font() {
    let temp_dir = tempfile::tempdir().unwrap();
    let font_path = temp_dir.path().join("test.woff");
    let mock_data = create_mock_woff();
    std::fs::write(&font_path, &mock_data).unwrap();

    let mut server = create_font_server(temp_dir.path());
    let handle: Handle<FontAsset> = server.load_sync("test.woff").unwrap();

    let font = server.get(&handle).unwrap();
    assert_eq!(font.name(), "test.woff");
    assert_eq!(font.format(), FontFormat::Woff);
}

// ============================================================================
// Handle and Deduplication Tests
// ============================================================================

#[test]
fn test_font_handle_deduplication() {
    let temp_dir = tempfile::tempdir().unwrap();
    let font_path = temp_dir.path().join("font.ttf");
    std::fs::write(&font_path, create_mock_ttf()).unwrap();

    let mut server = create_font_server(temp_dir.path());

    let handle1: Handle<FontAsset> = server.load_sync("font.ttf").unwrap();
    let handle2: Handle<FontAsset> = server.load_sync("font.ttf").unwrap();

    // Same path should return same handle
    assert_eq!(handle1.id(), handle2.id());
}

#[test]
fn test_font_asset_arc_sharing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let font_path = temp_dir.path().join("shared.ttf");
    std::fs::write(&font_path, create_mock_ttf()).unwrap();

    let mut server = create_font_server(temp_dir.path());
    let handle: Handle<FontAsset> = server.load_sync("shared.ttf").unwrap();

    let font1 = server.get(&handle).unwrap();
    let font2 = server.get(&handle).unwrap();

    // Should be the same Arc
    assert!(Arc::ptr_eq(font1, font2));
}

// ============================================================================
// BytesSource Tests
// ============================================================================

#[test]
fn test_load_font_from_bytes() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut server = create_font_server(temp_dir.path());

    let mock_data = create_mock_ttf();
    let data_arc: Arc<[u8]> = mock_data.clone().into();

    let handle = server.insert(
        AssetSource::bytes("embedded-font.ttf", data_arc),
        FontAsset::new(mock_data.clone(), "embedded-font.ttf"),
    );

    assert!(server.is_ready(&handle));
    let font = server.get(&handle).unwrap();
    assert_eq!(font.name(), "embedded-font.ttf");
    assert_eq!(font.format(), FontFormat::TrueType);
}

// ============================================================================
// FontDatabase Integration Tests
// ============================================================================

#[test]
fn test_load_font_into_database() {
    let temp_dir = tempfile::tempdir().unwrap();
    let font_path = temp_dir.path().join("loadable.ttf");
    std::fs::write(&font_path, create_mock_ttf()).unwrap();

    let mut server = create_font_server(temp_dir.path());
    let handle: Handle<FontAsset> = server.load_sync("loadable.ttf").unwrap();

    let font = server.get(&handle).unwrap();

    // Create an empty database and load the font into it
    let mut db = FontDatabase::empty();
    assert!(db.is_empty());

    font.load_into(&mut db);

    // Note: The mock font won't actually register as a valid font,
    // but the API should work. With a real font file, db.len() would increase.
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_load_nonexistent_font() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut server = create_font_server(temp_dir.path());

    let result: Result<Handle<FontAsset>, _> = server.load_sync("nonexistent.ttf");
    assert!(result.is_err());
}

#[test]
fn test_no_font_loader_for_unknown_extension() {
    let temp_dir = tempfile::tempdir().unwrap();
    let unknown_path = temp_dir.path().join("font.xyz");
    std::fs::write(&unknown_path, b"not a font").unwrap();

    let mut server = create_font_server(temp_dir.path());

    // Should fail because .xyz is not a registered font extension
    let result: Result<Handle<FontAsset>, _> = server.load_sync("font.xyz");
    assert!(result.is_err());
}

// ============================================================================
// Multiple Font Loading Tests
// ============================================================================

#[test]
fn test_load_multiple_fonts() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create multiple font files
    std::fs::write(temp_dir.path().join("font1.ttf"), create_mock_ttf()).unwrap();
    std::fs::write(temp_dir.path().join("font2.otf"), create_mock_otf()).unwrap();
    std::fs::write(temp_dir.path().join("font3.woff"), create_mock_woff()).unwrap();

    let mut server = create_font_server(temp_dir.path());

    let h1: Handle<FontAsset> = server.load_sync("font1.ttf").unwrap();
    let h2: Handle<FontAsset> = server.load_sync("font2.otf").unwrap();
    let h3: Handle<FontAsset> = server.load_sync("font3.woff").unwrap();

    // All should be loaded and distinct
    assert!(server.is_ready(&h1));
    assert!(server.is_ready(&h2));
    assert!(server.is_ready(&h3));

    assert_ne!(h1.id(), h2.id());
    assert_ne!(h2.id(), h3.id());
    assert_ne!(h1.id(), h3.id());

    let f1 = server.get(&h1).unwrap();
    let f2 = server.get(&h2).unwrap();
    let f3 = server.get(&h3).unwrap();

    assert_eq!(f1.format(), FontFormat::TrueType);
    assert_eq!(f2.format(), FontFormat::OpenType);
    assert_eq!(f3.format(), FontFormat::Woff);
}

// ============================================================================
// Event Tests
// ============================================================================

#[test]
fn test_font_load_creates_event() {
    use astrelis_assets::AssetEvent;

    let temp_dir = tempfile::tempdir().unwrap();
    let font_path = temp_dir.path().join("event_test.ttf");
    std::fs::write(&font_path, create_mock_ttf()).unwrap();

    let mut server = create_font_server(temp_dir.path());
    let _handle: Handle<FontAsset> = server.load_sync("event_test.ttf").unwrap();

    let events: Vec<_> = server.drain_events().collect();

    // Should have a Created event
    assert!(!events.is_empty());
    assert!(events.iter().any(|e| matches!(e, AssetEvent::Created { .. })));
}

// ============================================================================
// Version Tracking Tests
// ============================================================================

#[test]
fn test_font_version_tracking() {
    let temp_dir = tempfile::tempdir().unwrap();
    let font_path = temp_dir.path().join("versioned.ttf");
    std::fs::write(&font_path, create_mock_ttf()).unwrap();

    let mut server = create_font_server(temp_dir.path());
    let handle: Handle<FontAsset> = server.load_sync("versioned.ttf").unwrap();

    let version = server.version(&handle);
    assert!(version.is_some());
    assert!(version.unwrap() > 0);
}

// ============================================================================
// Find By Path Tests
// ============================================================================

#[test]
fn test_find_font_by_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    let font_path = temp_dir.path().join("findable.ttf");
    std::fs::write(&font_path, create_mock_ttf()).unwrap();

    let mut server = create_font_server(temp_dir.path());
    let handle: Handle<FontAsset> = server.load_sync("findable.ttf").unwrap();

    let found = server.find_by_path::<FontAsset>("findable.ttf");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id(), handle.id());

    // Non-existent path should return None
    let not_found = server.find_by_path::<FontAsset>("other.ttf");
    assert!(not_found.is_none());
}
