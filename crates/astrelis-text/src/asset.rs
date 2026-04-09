//! Asset integration for font loading.
//!
//! Provides [`FontAsset`] and [`FontLoader`] for loading font files through
//! the `astrelis-assets` system.

use std::path::Path;
use std::sync::Arc;

use astrelis_assets::{Asset, AssetLoadError, AssetLoader};

/// A font asset containing raw font data.
///
/// Holds the raw bytes of a font file (`.ttf`, `.otf`, `.woff`, `.woff2`)
/// which can be loaded into a [`FontDatabase`](crate::FontDatabase).
#[derive(Debug, Clone)]
pub struct FontAsset {
    data: Arc<[u8]>,
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

    /// Get the font data as an `Arc` for efficient sharing.
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

    /// Load this font into a [`FontDatabase`](crate::FontDatabase).
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
    /// TrueType font (`.ttf`).
    TrueType,
    /// OpenType font (`.otf`).
    OpenType,
    /// Web Open Font Format (`.woff`).
    Woff,
    /// Web Open Font Format 2 (`.woff2`).
    Woff2,
    /// TrueType Collection (`.ttc`).
    TrueTypeCollection,
    /// OpenType Collection (`.otc`).
    OpenTypeCollection,
    /// Unknown format.
    Unknown,
}

impl FontFormat {
    /// Detect font format from magic bytes.
    pub fn detect(data: &[u8]) -> Self {
        if data.len() < 4 {
            return FontFormat::Unknown;
        }

        match &data[0..4] {
            [0x00, 0x01, 0x00, 0x00] | [b't', b'r', b'u', b'e'] => FontFormat::TrueType,
            [b'O', b'T', b'T', b'O'] => FontFormat::OpenType,
            [b'w', b'O', b'F', b'F'] => FontFormat::Woff,
            [b'w', b'O', b'F', b'2'] => FontFormat::Woff2,
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
pub struct FontLoader;

impl AssetLoader for FontLoader {
    type Asset = FontAsset;

    fn extensions(&self) -> &[&str] {
        &["ttf", "otf", "woff", "woff2", "ttc", "otc"]
    }

    fn load(&self, bytes: &[u8], path: &Path) -> Result<Self::Asset, AssetLoadError> {
        let format = FontFormat::detect(bytes);
        if format == FontFormat::Unknown && bytes.len() > 4 {
            tracing::warn!(
                "Font file '{}' has unrecognized format (magic: {:02x?}), loading anyway",
                path.display(),
                &bytes[..4.min(bytes.len())]
            );
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(FontAsset::new(bytes.to_vec(), name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_format_detection() {
        assert_eq!(
            FontFormat::detect(&[0x00, 0x01, 0x00, 0x00, 0x00, 0x00]),
            FontFormat::TrueType
        );
        assert_eq!(FontFormat::detect(b"true\x00\x00"), FontFormat::TrueType);
        assert_eq!(FontFormat::detect(b"OTTO\x00\x00"), FontFormat::OpenType);
        assert_eq!(FontFormat::detect(b"wOFF\x00\x00"), FontFormat::Woff);
        assert_eq!(FontFormat::detect(b"wOF2\x00\x00"), FontFormat::Woff2);
        assert_eq!(
            FontFormat::detect(b"ttcf\x00\x00"),
            FontFormat::TrueTypeCollection
        );
        assert_eq!(FontFormat::detect(b"????"), FontFormat::Unknown);
        assert_eq!(FontFormat::detect(&[0x00, 0x01]), FontFormat::Unknown);
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
        let loader = FontLoader;
        let data = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00];
        let path = Path::new("fonts/TestFont.ttf");

        let result = loader.load(&data, path);
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
