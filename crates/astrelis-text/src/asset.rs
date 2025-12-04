//! Asset integration for font loading.
//!
//! This module provides integration with the `astrelis-assets` system,
//! allowing fonts to be loaded through the standard asset pipeline.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_assets::{AssetServer, Handle};
//! use astrelis_text::{FontAsset, FontLoader};
//!
//! let mut server = AssetServer::new();
//! server.register_loader(FontLoader);
//!
//! // Load a font file
//! let font: Handle<FontAsset> = server.load_sync("fonts/MyFont.ttf").unwrap();
//!
//! // Use the font data
//! if let Some(font_asset) = server.get(&font) {
//!     // font_asset.data() returns the raw font bytes
//!     // font_asset.name() returns the font file name
//! }
//! ```

use std::sync::Arc;

use astrelis_assets::{Asset, AssetLoader, AssetResult, LoadContext};

/// A font asset containing raw font data.
///
/// This asset holds the raw bytes of a font file (.ttf, .otf, .woff, .woff2)
/// which can be used to load the font into a `FontDatabase` or `FontSystem`.
///
/// # Usage
///
/// ```ignore
/// use astrelis_text::{FontAsset, FontDatabase};
///
/// // After loading the font asset
/// let font_asset: &FontAsset = server.get(&handle).unwrap();
///
/// // Load into a font database
/// let mut db = FontDatabase::empty();
/// db.load_font_data(font_asset.data().to_vec());
/// ```
#[derive(Debug, Clone)]
pub struct FontAsset {
    /// The raw font data bytes.
    data: Arc<[u8]>,
    /// The name/identifier of the font (usually the filename).
    name: String,
}

impl FontAsset {
    /// Create a new font asset from raw data.
    pub fn new(data: impl Into<Arc<[u8]>>, name: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            name: name.into(),
        }
    }

    /// Get the raw font data.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get the font data as an Arc for efficient sharing.
    pub fn data_arc(&self) -> Arc<[u8]> {
        self.data.clone()
    }

    /// Get the name/identifier of the font.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the size of the font data in bytes.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Detect the font format from the data.
    pub fn format(&self) -> FontFormat {
        FontFormat::detect(&self.data)
    }

    /// Load this font into a FontDatabase.
    ///
    /// This is a convenience method that adds the font data to the database.
    pub fn load_into(&self, db: &mut crate::FontDatabase) {
        db.load_font_data(self.data.to_vec());
    }
}

impl Asset for FontAsset {
    fn type_name() -> &'static str {
        "FontAsset"
    }
}

/// Detected font file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFormat {
    /// TrueType font (.ttf)
    TrueType,
    /// OpenType font (.otf)
    OpenType,
    /// Web Open Font Format (.woff)
    Woff,
    /// Web Open Font Format 2 (.woff2)
    Woff2,
    /// TrueType Collection (.ttc)
    TrueTypeCollection,
    /// OpenType Collection (.otc)
    OpenTypeCollection,
    /// Unknown format
    Unknown,
}

impl FontFormat {
    /// Detect font format from the magic bytes.
    pub fn detect(data: &[u8]) -> Self {
        if data.len() < 4 {
            return FontFormat::Unknown;
        }

        match &data[0..4] {
            // TrueType: 0x00010000 or 'true'
            [0x00, 0x01, 0x00, 0x00] | [b't', b'r', b'u', b'e'] => FontFormat::TrueType,
            // OpenType: 'OTTO'
            [b'O', b'T', b'T', b'O'] => FontFormat::OpenType,
            // WOFF: 'wOFF'
            [b'w', b'O', b'F', b'F'] => FontFormat::Woff,
            // WOFF2: 'wOF2'
            [b'w', b'O', b'F', b'2'] => FontFormat::Woff2,
            // TrueType Collection: 'ttcf'
            [b't', b't', b'c', b'f'] => FontFormat::TrueTypeCollection,
            _ => FontFormat::Unknown,
        }
    }

    /// Get the typical file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            FontFormat::TrueType => "ttf",
            FontFormat::OpenType => "otf",
            FontFormat::Woff => "woff",
            FontFormat::Woff2 => "woff2",
            FontFormat::TrueTypeCollection => "ttc",
            FontFormat::OpenTypeCollection => "otc",
            FontFormat::Unknown => "bin",
        }
    }
}

/// Asset loader for font files.
///
/// Supports loading `.ttf`, `.otf`, `.woff`, `.woff2`, `.ttc`, and `.otc` files.
///
/// # Example
///
/// ```ignore
/// use astrelis_assets::AssetServer;
/// use astrelis_text::FontLoader;
///
/// let mut server = AssetServer::new();
/// server.register_loader(FontLoader);
///
/// // Now you can load font files
/// let handle = server.load_sync::<FontAsset>("fonts/Roboto-Regular.ttf");
/// ```
pub struct FontLoader;

impl AssetLoader for FontLoader {
    type Asset = FontAsset;

    fn extensions(&self) -> &[&str] {
        &["ttf", "otf", "woff", "woff2", "ttc", "otc"]
    }

    fn load(&self, ctx: LoadContext<'_>) -> AssetResult<Self::Asset> {
        // Validate that the data looks like a font
        let format = FontFormat::detect(ctx.bytes);
        if format == FontFormat::Unknown && ctx.bytes.len() > 4 {
            // Only warn for non-empty files that don't have recognized magic bytes
            tracing::warn!(
                "Font file '{}' has unrecognized format (magic: {:02x?}), loading anyway",
                ctx.source.display_path(),
                &ctx.bytes[..4.min(ctx.bytes.len())]
            );
        }

        // Extract name from source path
        let name = ctx
            .source
            .path()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(String::from)
            .unwrap_or_else(|| ctx.source.display_path());

        Ok(FontAsset::new(ctx.bytes.to_vec(), name))
    }

    fn priority(&self) -> i32 {
        // Default priority
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_format_detection() {
        // TrueType
        let ttf_data = [0x00, 0x01, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(FontFormat::detect(&ttf_data), FontFormat::TrueType);

        // TrueType (alternate magic)
        let ttf_true = b"true\x00\x00";
        assert_eq!(FontFormat::detect(ttf_true), FontFormat::TrueType);

        // OpenType
        let otf_data = b"OTTO\x00\x00";
        assert_eq!(FontFormat::detect(otf_data), FontFormat::OpenType);

        // WOFF
        let woff_data = b"wOFF\x00\x00";
        assert_eq!(FontFormat::detect(woff_data), FontFormat::Woff);

        // WOFF2
        let woff2_data = b"wOF2\x00\x00";
        assert_eq!(FontFormat::detect(woff2_data), FontFormat::Woff2);

        // TTC
        let ttc_data = b"ttcf\x00\x00";
        assert_eq!(FontFormat::detect(ttc_data), FontFormat::TrueTypeCollection);

        // Unknown
        let unknown = b"????";
        assert_eq!(FontFormat::detect(unknown), FontFormat::Unknown);

        // Too short
        let short = [0x00, 0x01];
        assert_eq!(FontFormat::detect(&short), FontFormat::Unknown);
    }

    #[test]
    fn test_font_asset_creation() {
        let data: Vec<u8> = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x10];
        let asset = FontAsset::new(data.clone(), "test.ttf");

        assert_eq!(asset.name(), "test.ttf");
        assert_eq!(asset.data(), &data[..]);
        assert_eq!(asset.size(), 6);
        assert_eq!(asset.format(), FontFormat::TrueType);
    }

    #[test]
    fn test_font_asset_clone() {
        let data: Vec<u8> = vec![0x00, 0x01, 0x00, 0x00];
        let asset1 = FontAsset::new(data, "font.ttf");
        let asset2 = asset1.clone();

        assert_eq!(asset1.name(), asset2.name());
        assert_eq!(asset1.data(), asset2.data());
        // Data should be shared via Arc
        assert!(Arc::ptr_eq(&asset1.data, &asset2.data));
    }

    #[test]
    fn test_font_loader_extensions() {
        let loader = FontLoader;
        let exts = loader.extensions();

        assert!(exts.contains(&"ttf"));
        assert!(exts.contains(&"otf"));
        assert!(exts.contains(&"woff"));
        assert!(exts.contains(&"woff2"));
        assert!(exts.contains(&"ttc"));
        assert!(exts.contains(&"otc"));
    }

    #[test]
    fn test_font_loader_load() {
        use astrelis_assets::AssetSource;

        let loader = FontLoader;
        let data = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00];
        let source = AssetSource::disk("fonts/TestFont.ttf");
        let ctx = LoadContext::new(&source, &data, Some("ttf"));

        let result = loader.load(ctx);
        assert!(result.is_ok());

        let asset = result.unwrap();
        assert_eq!(asset.name(), "TestFont.ttf");
        assert_eq!(asset.data(), &data[..]);
        assert_eq!(asset.format(), FontFormat::TrueType);
    }

    #[test]
    fn test_font_format_extensions() {
        assert_eq!(FontFormat::TrueType.extension(), "ttf");
        assert_eq!(FontFormat::OpenType.extension(), "otf");
        assert_eq!(FontFormat::Woff.extension(), "woff");
        assert_eq!(FontFormat::Woff2.extension(), "woff2");
        assert_eq!(FontFormat::TrueTypeCollection.extension(), "ttc");
        assert_eq!(FontFormat::OpenTypeCollection.extension(), "otc");
        assert_eq!(FontFormat::Unknown.extension(), "bin");
    }
}
