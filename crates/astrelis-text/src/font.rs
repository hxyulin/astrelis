//! Font database and font system management.
//!
//! Wraps [`cosmic_text::FontSystem`] with a thread-safe interface for font
//! management. Provides [`FontDatabase`] for loading system fonts and custom
//! font files, and [`FontSystem`] as the primary entry point for text shaping.

use cosmic_text::fontdb;
use std::sync::{Arc, RwLock};

/// A font database that manages available fonts.
///
/// Provides methods to load system fonts, custom font files, and query
/// available font families.
pub struct FontDatabase {
    pub(crate) inner: fontdb::Database,
}

impl FontDatabase {
    /// Create a new font database with system fonts loaded.
    pub fn new() -> Self {
        astrelis_profiling::profile_function!();
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        Self { inner: db }
    }

    /// Create an empty font database.
    pub fn empty() -> Self {
        Self {
            inner: fontdb::Database::new(),
        }
    }

    /// Load a font from raw bytes.
    pub fn load_font_data(&mut self, data: Vec<u8>) {
        astrelis_profiling::profile_function!();
        self.inner
            .load_font_source(fontdb::Source::Binary(Arc::new(data)));
    }

    /// Load a font from a `.ttf` or `.otf` file.
    pub fn load_font_file(&mut self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        astrelis_profiling::profile_function!();
        self.inner.load_font_file(path)?;
        Ok(())
    }

    /// Load fonts from a directory.
    pub fn load_fonts_dir(&mut self, path: impl AsRef<std::path::Path>) {
        astrelis_profiling::profile_function!();
        self.inner.load_fonts_dir(path);
    }

    /// Query a font by family name.
    ///
    /// Returns `true` if the font family is available.
    pub fn has_family(&self, family: &str) -> bool {
        self.inner
            .faces()
            .any(|face| face.families.iter().any(|(f, _)| f == family))
    }

    /// List all available font families.
    pub fn list_families(&self) -> Vec<String> {
        let mut families = std::collections::HashSet::new();
        for face in self.inner.faces() {
            for (family, _) in &face.families {
                families.insert(family.clone());
            }
        }
        let mut families: Vec<String> = families.into_iter().collect();
        families.sort();
        families
    }

    /// Get the number of font faces loaded.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the database is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for FontDatabase {
    fn default() -> Self {
        Self::new()
    }
}

/// Font management system.
///
/// Thread-safe wrapper around [`cosmic_text::FontSystem`] that manages font
/// loading, caching, and text shaping. Multiple renderers can share a single
/// `FontSystem` via internal `Arc<RwLock<...>>`.
pub struct FontSystem {
    inner: Arc<RwLock<cosmic_text::FontSystem>>,
}

impl FontSystem {
    /// Create a new font system with the given font database.
    pub fn new(db: FontDatabase) -> Self {
        astrelis_profiling::profile_function!();
        let cosmic_font_system = cosmic_text::FontSystem::new_with_locale_and_db(
            sys_locale::get_locale().unwrap_or_else(|| String::from("en-US")),
            db.inner,
        );
        Self {
            inner: Arc::new(RwLock::new(cosmic_font_system)),
        }
    }

    /// Create a new font system with system fonts.
    pub fn with_system_fonts() -> Self {
        astrelis_profiling::profile_function!();
        Self::new(FontDatabase::new())
    }

    /// Create a new font system with no fonts loaded.
    pub fn with_empty_fonts() -> Self {
        Self::new(FontDatabase::empty())
    }

    /// Get a reference to the inner font system.
    pub fn inner(&self) -> &Arc<RwLock<cosmic_text::FontSystem>> {
        &self.inner
    }
}

impl Default for FontSystem {
    fn default() -> Self {
        Self::with_system_fonts()
    }
}

impl Clone for FontSystem {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Font weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontWeight {
    /// Thin (100).
    Thin,
    /// Extra light (200).
    ExtraLight,
    /// Light (300).
    Light,
    /// Normal (400).
    #[default]
    Normal,
    /// Medium (500).
    Medium,
    /// Semi bold (600).
    SemiBold,
    /// Bold (700).
    Bold,
    /// Extra bold (800).
    ExtraBold,
    /// Black (900).
    Black,
}

impl FontWeight {
    /// Convert to cosmic-text weight.
    pub fn to_cosmic(self) -> cosmic_text::Weight {
        match self {
            FontWeight::Thin => cosmic_text::Weight::THIN,
            FontWeight::ExtraLight => cosmic_text::Weight::EXTRA_LIGHT,
            FontWeight::Light => cosmic_text::Weight::LIGHT,
            FontWeight::Normal => cosmic_text::Weight::NORMAL,
            FontWeight::Medium => cosmic_text::Weight::MEDIUM,
            FontWeight::SemiBold => cosmic_text::Weight::SEMIBOLD,
            FontWeight::Bold => cosmic_text::Weight::BOLD,
            FontWeight::ExtraBold => cosmic_text::Weight::EXTRA_BOLD,
            FontWeight::Black => cosmic_text::Weight::BLACK,
        }
    }
}

/// Font style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStyle {
    /// Normal upright style.
    #[default]
    Normal,
    /// Italic style.
    Italic,
    /// Oblique style.
    Oblique,
}

impl FontStyle {
    /// Convert to cosmic-text style.
    pub fn to_cosmic(self) -> cosmic_text::Style {
        match self {
            FontStyle::Normal => cosmic_text::Style::Normal,
            FontStyle::Italic => cosmic_text::Style::Italic,
            FontStyle::Oblique => cosmic_text::Style::Oblique,
        }
    }
}

/// Font stretch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStretch {
    /// Ultra condensed.
    UltraCondensed,
    /// Extra condensed.
    ExtraCondensed,
    /// Condensed.
    Condensed,
    /// Semi condensed.
    SemiCondensed,
    /// Normal width.
    #[default]
    Normal,
    /// Semi expanded.
    SemiExpanded,
    /// Expanded.
    Expanded,
    /// Extra expanded.
    ExtraExpanded,
    /// Ultra expanded.
    UltraExpanded,
}

impl FontStretch {
    /// Convert to cosmic-text stretch.
    pub fn to_cosmic(self) -> cosmic_text::Stretch {
        match self {
            FontStretch::UltraCondensed => cosmic_text::Stretch::UltraCondensed,
            FontStretch::ExtraCondensed => cosmic_text::Stretch::ExtraCondensed,
            FontStretch::Condensed => cosmic_text::Stretch::Condensed,
            FontStretch::SemiCondensed => cosmic_text::Stretch::SemiCondensed,
            FontStretch::Normal => cosmic_text::Stretch::Normal,
            FontStretch::SemiExpanded => cosmic_text::Stretch::SemiExpanded,
            FontStretch::Expanded => cosmic_text::Stretch::Expanded,
            FontStretch::ExtraExpanded => cosmic_text::Stretch::ExtraExpanded,
            FontStretch::UltraExpanded => cosmic_text::Stretch::UltraExpanded,
        }
    }
}

/// Font attributes describing a font face.
#[derive(Debug, Clone)]
pub struct FontAttributes {
    /// Font family name.
    pub family: String,
    /// Font weight.
    pub weight: FontWeight,
    /// Font style.
    pub style: FontStyle,
    /// Font stretch.
    pub stretch: FontStretch,
}

impl FontAttributes {
    /// Create font attributes with a specific family name.
    pub fn new(family: impl Into<String>) -> Self {
        Self {
            family: family.into(),
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
            stretch: FontStretch::Normal,
        }
    }

    /// Set font weight.
    pub fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Set font style.
    pub fn with_style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }

    /// Set font stretch.
    pub fn with_stretch(mut self, stretch: FontStretch) -> Self {
        self.stretch = stretch;
        self
    }
}

impl Default for FontAttributes {
    fn default() -> Self {
        Self {
            family: String::new(),
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
            stretch: FontStretch::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_database_empty() {
        let db = FontDatabase::empty();
        assert!(db.is_empty());
        assert_eq!(db.len(), 0);
    }

    #[test]
    fn test_font_database_system() {
        let db = FontDatabase::new();
        // System fonts should be loaded (may vary by platform)
        assert!(!db.list_families().is_empty() || db.is_empty());
    }

    #[test]
    fn test_font_database_load_data() {
        let mut db = FontDatabase::empty();
        // Loading invalid data shouldn't crash
        db.load_font_data(vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_font_system_creation() {
        let _fs = FontSystem::with_system_fonts();
    }

    #[test]
    fn test_font_system_empty() {
        let _fs = FontSystem::with_empty_fonts();
    }

    #[test]
    fn test_font_system_clone() {
        let fs1 = FontSystem::with_empty_fonts();
        let fs2 = fs1.clone();
        // Should share the same inner Arc
        assert!(Arc::ptr_eq(fs1.inner(), fs2.inner()));
    }

    #[test]
    fn test_font_weight_default() {
        assert_eq!(FontWeight::default(), FontWeight::Normal);
    }

    #[test]
    fn test_font_style_default() {
        assert_eq!(FontStyle::default(), FontStyle::Normal);
    }

    #[test]
    fn test_font_attributes_builder() {
        let attrs = FontAttributes::new("Arial")
            .with_weight(FontWeight::Bold)
            .with_style(FontStyle::Italic);

        assert_eq!(attrs.family, "Arial");
        assert_eq!(attrs.weight, FontWeight::Bold);
        assert_eq!(attrs.style, FontStyle::Italic);
    }
}
