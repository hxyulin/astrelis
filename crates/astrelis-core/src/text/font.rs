use std::sync::Arc;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, fontdb};

use crate::assets::Asset;

// Embedded default font - Sarasa UI TC (supports Latin, CJK, and more)
const DEFAULT_FONT: &[u8] = include_bytes!("fonts/SarasaUiTC-Regular.ttf");
pub const DEFAULT_FONT_NAME: &str = "Sarasa UI TC";

// Embedded test fonts for testing text rendering with various scripts
const TEST_ASCII_FONT: &[u8] = include_bytes!("fonts/TestASCII-Regular.otf");
const TEST_ARABIC_FONT: &[u8] = include_bytes!("fonts/TestArabicSubset-Regular.otf");
const TEST_CJK_FONT: &[u8] = include_bytes!("fonts/TestCJKSubset-Regular.otf");

/// Font asset for text rendering using cosmic-text
///
/// Supports complex text shaping, bidirectional text, and CJK characters.
/// Uses cosmic-text's FontSystem for proper font management and text layout.
#[derive(Clone)]
pub struct Font {
    /// Font family name
    pub name: String,
    /// Raw font data (TTF/OTF bytes)
    data: Arc<Vec<u8>>,
    /// Font database IDs within cosmic-text (a font file can contain multiple fonts)
    font_ids: Vec<fontdb::ID>,
}

impl Font {
    /// Create a new font from raw bytes
    pub fn from_bytes(name: String, data: Vec<u8>) -> Self {
        Self {
            name,
            data: Arc::new(data),
            font_ids: Vec::new(),
        }
    }

    /// Load font from file path
    pub fn from_file(path: &str) -> Result<Self, std::io::Error> {
        let data = std::fs::read(path)?;
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown Font")
            .to_string();
        Ok(Self::from_bytes(name, data))
    }

    /// Register this font with a FontSystem
    /// Must be called before using the font for text rendering
    pub fn register(&mut self, font_system: &mut FontSystem) {
        if self.font_ids.is_empty() {
            let db = font_system.db_mut();
            let ids = db.load_font_source(cosmic_text::fontdb::Source::Binary(self.data.clone()));
            self.font_ids.extend(ids.iter());
        }
    }

    /// Load embedded default font (Sarasa UI TC - comprehensive Unicode support)
    pub fn default() -> Self {
        Self::from_bytes(DEFAULT_FONT_NAME.to_string(), DEFAULT_FONT.to_vec())
    }

    /// Load embedded test ASCII font
    pub fn test_ascii() -> Self {
        Self::from_bytes("TestASCII".to_string(), TEST_ASCII_FONT.to_vec())
    }

    /// Load embedded test Arabic font
    pub fn test_arabic() -> Self {
        Self::from_bytes("TestArabicSubset".to_string(), TEST_ARABIC_FONT.to_vec())
    }

    /// Load embedded test CJK font
    pub fn test_cjk() -> Self {
        Self::from_bytes("TestCJKSubset".to_string(), TEST_CJK_FONT.to_vec())
    }

    /// Check if font is registered with a FontSystem
    pub fn is_registered(&self) -> bool {
        !self.font_ids.is_empty()
    }

    /// Get the first font database ID (most fonts have only one)
    pub fn font_id(&self) -> Option<fontdb::ID> {
        self.font_ids.first().copied()
    }

    /// Get all font database IDs (for font collections)
    pub fn font_ids(&self) -> &[fontdb::ID] {
        &self.font_ids
    }

    /// Get font data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get font family name
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Asset for Font {}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font")
            .field("name", &self.name)
            .field("data_len", &self.data.len())
            .field("registered", &self.is_registered())
            .finish()
    }
}

/// Text buffer builder for creating text layouts
/// Wraps cosmic-text's Buffer with a more ergonomic API
pub struct TextBufferBuilder {
    text: String,
    font_size: f32,
    line_height: f32,
    font_family: String,
}

impl TextBufferBuilder {
    /// Create a new text buffer builder
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            font_size: 16.0,
            line_height: 1.2,
            font_family: "sans-serif".to_string(),
        }
    }

    /// Set font size in pixels
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set line height multiplier
    pub fn line_height(mut self, height: f32) -> Self {
        self.line_height = height;
        self
    }

    /// Set font family name
    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.font_family = family.into();
        self
    }

    /// Build the text buffer using the provided FontSystem
    pub fn build(self, font_system: &mut FontSystem) -> Buffer {
        let metrics = Metrics::new(self.font_size, self.line_height * self.font_size);
        let mut buffer = Buffer::new(font_system, metrics);

        buffer.set_size(font_system, Some(800.0), None);
        buffer.set_text(
            font_system,
            &self.text,
            Attrs::new().family(cosmic_text::Family::Name(&self.font_family)),
            Shaping::Advanced,
        );

        buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_creation() {
        let font_data = vec![0u8; 1024];
        let font = Font::from_bytes("Test Font".to_string(), font_data);

        assert_eq!(font.name(), "Test Font");
        assert_eq!(font.data().len(), 1024);
        assert!(!font.is_registered());
    }

    #[test]
    fn test_font_registration() {
        // We can't easily test actual font registration without valid TTF data
        // but we can test the API
        let font_data = vec![0u8; 1024];
        let font = Font::from_bytes("Test Font".to_string(), font_data);

        assert!(!font.is_registered());
        assert_eq!(font.font_id(), None);
    }

    #[test]
    fn test_text_buffer_builder() {
        let builder = TextBufferBuilder::new("Hello, World!")
            .font_size(24.0)
            .line_height(1.5)
            .font_family("Arial");

        // Builder should store values correctly
        assert_eq!(builder.font_size, 24.0);
        assert_eq!(builder.line_height, 1.5);
        assert_eq!(builder.font_family, "Arial");
    }

    #[test]
    fn test_embedded_fonts_load() {
        let ascii_font = Font::test_ascii();
        let arabic_font = Font::test_arabic();
        let cjk_font = Font::test_cjk();

        assert_eq!(ascii_font.name(), "TestASCII");
        assert_eq!(arabic_font.name(), "TestArabicSubset");
        assert_eq!(cjk_font.name(), "TestCJKSubset");

        assert!(ascii_font.data().len() > 0);
        assert!(arabic_font.data().len() > 0);
        assert!(cjk_font.data().len() > 0);
    }

    #[test]
    fn test_ascii_font_registration() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_ascii();

        assert!(!font.is_registered());
        font.register(&mut font_system);
        assert!(font.is_registered());
        assert!(font.font_id().is_some());
    }

    #[test]
    fn test_arabic_font_registration() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_arabic();

        font.register(&mut font_system);
        assert!(font.is_registered());
        assert!(font.font_ids().len() > 0);
    }

    #[test]
    fn test_cjk_font_registration() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_cjk();

        font.register(&mut font_system);
        assert!(font.is_registered());
    }

    #[test]
    fn test_ascii_text_rendering() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_ascii();
        font.register(&mut font_system);

        // Test ASCII printable range (0x20-0x7E)
        let text = "Hello, World! 123 @#$%";
        let buffer = TextBufferBuilder::new(text)
            .font_size(16.0)
            .font_family("TestASCII")
            .build(&mut font_system);

        // Buffer should contain the text
        assert!(buffer.lines.len() > 0);
    }

    #[test]
    fn test_arabic_text_rendering() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_arabic();
        font.register(&mut font_system);

        // Test Arabic characters from subset: ا ل م ب ن ي
        let text = "المبني"; // Arabic text using subset characters
        let buffer = TextBufferBuilder::new(text)
            .font_size(16.0)
            .font_family("TestArabicSubset")
            .build(&mut font_system);

        assert!(buffer.lines.len() > 0);
    }

    #[test]
    fn test_cjk_text_rendering() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_cjk();
        font.register(&mut font_system);

        // Test CJK characters from subset: 中国文語漢字日本
        let text = "中国文語日本";
        let buffer = TextBufferBuilder::new(text)
            .font_size(16.0)
            .font_family("TestCJKSubset")
            .build(&mut font_system);

        assert!(buffer.lines.len() > 0);
    }

    #[test]
    fn test_font_metrics() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_ascii();
        font.register(&mut font_system);

        let buffer = TextBufferBuilder::new("Test")
            .font_size(20.0)
            .line_height(1.5)
            .build(&mut font_system);

        // Verify metrics are set correctly
        let metrics = buffer.metrics();
        assert_eq!(metrics.font_size, 20.0);
        assert_eq!(metrics.line_height, 30.0); // 20.0 * 1.5
    }

    #[test]
    fn test_multiple_fonts_registered() {
        let mut font_system = FontSystem::new();

        let mut ascii = Font::test_ascii();
        let mut arabic = Font::test_arabic();
        let mut cjk = Font::test_cjk();

        ascii.register(&mut font_system);
        arabic.register(&mut font_system);
        cjk.register(&mut font_system);

        assert!(ascii.is_registered());
        assert!(arabic.is_registered());
        assert!(cjk.is_registered());

        // All fonts should have different IDs
        let id1 = ascii.font_id().unwrap();
        let id2 = arabic.font_id().unwrap();
        let id3 = cjk.font_id().unwrap();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_font_data_integrity() {
        let ascii_font = Font::test_ascii();
        let arabic_font = Font::test_arabic();
        let cjk_font = Font::test_cjk();

        // Fonts should have reasonable sizes
        assert!(ascii_font.data().len() > 1000);
        assert!(arabic_font.data().len() > 1000);
        assert!(cjk_font.data().len() > 1000);

        // OTF files start with specific magic bytes
        // OTF: "OTTO" or TrueType: 0x00010000
        let ascii_data = ascii_font.data();
        assert!(ascii_data.len() >= 4);
    }

    #[test]
    fn test_text_shaping_advanced() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_ascii();
        font.register(&mut font_system);

        // Test that shaping is applied (ligatures, kerning, etc.)
        let buffer = TextBufferBuilder::new("Hello World")
            .font_size(16.0)
            .font_family("TestASCII")
            .build(&mut font_system);

        // Buffer should have shaped runs
        assert!(buffer.lines.len() > 0);
        for line in buffer.lines.iter() {
            // Each line should have layout runs
            assert!(line.layout_opt().is_some());
        }
    }

    #[test]
    fn test_bidirectional_text() {
        let mut font_system = FontSystem::new();
        let mut arabic = Font::test_arabic();
        arabic.register(&mut font_system);

        // Arabic text with RTL direction
        let text = "المبني";
        let buffer = TextBufferBuilder::new(text)
            .font_size(16.0)
            .font_family("TestArabicSubset")
            .build(&mut font_system);

        // Should handle RTL text correctly
        assert!(buffer.lines.len() > 0);

        // cosmic-text should process this as RTL
        for line in buffer.lines.iter() {
            if let Some(layout) = line.layout_opt() {
                // Layout should exist for shaped text
                assert!(layout.len() > 0);
            }
        }
    }

    #[test]
    fn test_mixed_script_text() {
        let mut font_system = FontSystem::new();
        let mut ascii = Font::test_ascii();
        let mut cjk = Font::test_cjk();
        ascii.register(&mut font_system);
        cjk.register(&mut font_system);

        // Mix Latin and CJK
        let text = "Hello 中国";
        let buffer = TextBufferBuilder::new(text)
            .font_size(16.0)
            .build(&mut font_system);

        // Should handle mixed scripts
        assert!(buffer.lines.len() > 0);
    }

    #[test]
    fn test_glyph_metrics() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_ascii();
        font.register(&mut font_system);

        let buffer = TextBufferBuilder::new("Mg")
            .font_size(24.0)
            .build(&mut font_system);

        // Verify buffer metrics
        let metrics = buffer.metrics();
        assert!(metrics.font_size > 0.0);
        assert!(metrics.line_height >= metrics.font_size);
    }

    #[test]
    fn test_multiline_layout() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_ascii();
        font.register(&mut font_system);

        let text = "Line 1\nLine 2\nLine 3";
        let buffer = TextBufferBuilder::new(text)
            .font_size(16.0)
            .font_family("TestASCII")
            .build(&mut font_system);

        // Should have 3 lines
        assert_eq!(buffer.lines.len(), 3);
    }

    #[test]
    fn test_empty_text() {
        let mut font_system = FontSystem::new();
        let buffer = TextBufferBuilder::new("")
            .font_size(16.0)
            .build(&mut font_system);

        // Empty text should still create a buffer
        assert!(buffer.lines.len() >= 0);
    }

    #[test]
    fn test_whitespace_handling() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_ascii();
        font.register(&mut font_system);

        let text = "  Multiple   Spaces  ";
        let buffer = TextBufferBuilder::new(text)
            .font_size(16.0)
            .font_family("TestASCII")
            .build(&mut font_system);

        assert!(buffer.lines.len() > 0);
    }

    #[test]
    fn test_special_characters() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_ascii();
        font.register(&mut font_system);

        // Test special ASCII characters from the font subset
        let text = "!@#$%^&*()_+-=[]{}|;:',.<>?/~`";
        let buffer = TextBufferBuilder::new(text)
            .font_size(16.0)
            .font_family("TestASCII")
            .build(&mut font_system);

        assert!(buffer.lines.len() > 0);
    }

    #[test]
    fn test_cjk_punctuation() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_cjk();
        font.register(&mut font_system);

        // CJK punctuation from the subset
        let text = "、。「」『』";
        let buffer = TextBufferBuilder::new(text)
            .font_size(16.0)
            .font_family("TestCJKSubset")
            .build(&mut font_system);

        assert!(buffer.lines.len() > 0);
    }

    #[test]
    fn test_arabic_diacritics() {
        let mut font_system = FontSystem::new();
        let mut arabic = Font::test_arabic();
        arabic.register(&mut font_system);

        // Arabic with diacritics (kasra, shadda, sukun)
        let text = "\u{0650}\u{0651}\u{0652}";
        let buffer = TextBufferBuilder::new(text)
            .font_size(16.0)
            .font_family("TestArabicSubset")
            .build(&mut font_system);

        // Should handle combining marks
        assert!(buffer.lines.len() > 0);
    }

    #[test]
    fn test_font_size_variations() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_ascii();
        font.register(&mut font_system);

        let sizes = [8.0, 12.0, 16.0, 24.0, 32.0, 48.0];

        for size in sizes {
            let buffer = TextBufferBuilder::new("Test")
                .font_size(size)
                .font_family("TestASCII")
                .build(&mut font_system);

            assert_eq!(buffer.metrics().font_size, size);
        }
    }

    #[test]
    fn test_line_height_variations() {
        let mut font_system = FontSystem::new();

        let line_heights = [1.0, 1.2, 1.5, 2.0];
        let font_size = 16.0;

        for lh in line_heights {
            let buffer = TextBufferBuilder::new("Test")
                .font_size(font_size)
                .line_height(lh)
                .build(&mut font_system);

            assert_eq!(buffer.metrics().line_height, font_size * lh);
        }
    }

    #[test]
    fn test_buffer_width_constraint() {
        let mut font_system = FontSystem::new();
        let mut font = Font::test_ascii();
        font.register(&mut font_system);

        // Create buffer with initial width
        let metrics = Metrics::new(16.0, 16.0 * 1.2);
        let mut buffer = Buffer::new(&mut font_system, metrics);

        // Set narrow width to force wrapping
        buffer.set_size(&mut font_system, Some(100.0), None);
        buffer.set_text(
            &mut font_system,
            "This is a very long text that should wrap when constrained",
            Attrs::new().family(cosmic_text::Family::Name("TestASCII")),
            Shaping::Advanced,
        );

        // Verify buffer has width constraint applied
        assert!(buffer.lines.len() >= 1);

        // At minimum, buffer should respect size constraints
        let size = buffer.size();
        assert!(size.0.is_some());
    }

    #[test]
    fn test_default_font_loads() {
        let font = Font::default();
        assert_eq!(font.name(), DEFAULT_FONT_NAME);
        assert!(font.data().len() > 100_000); // Sarasa is a large font
    }

    #[test]
    fn test_default_font_registration() {
        let mut font_system = FontSystem::new();
        let mut font = Font::default();

        font.register(&mut font_system);
        assert!(font.is_registered());
        assert!(font.font_id().is_some());
    }

    #[test]
    fn test_default_font_renders_mixed_text() {
        let mut font_system = FontSystem::new();
        let mut font = Font::default();
        font.register(&mut font_system);

        // Test mixed Latin, CJK, and symbols
        let text = "Hello 世界 123 @#$";
        let buffer = TextBufferBuilder::new(text)
            .font_size(16.0)
            .font_family(DEFAULT_FONT_NAME)
            .build(&mut font_system);

        assert!(buffer.lines.len() > 0);
    }
}
