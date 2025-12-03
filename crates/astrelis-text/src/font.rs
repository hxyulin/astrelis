use cosmic_text::{Attrs, fontdb};
use std::sync::{Arc, RwLock};

/// A font database that manages available fonts.
pub struct FontDatabase {
    inner: fontdb::Database,
}

impl FontDatabase {
    /// Create a new font database with system fonts loaded.
    pub fn new() -> Self {
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

    /// Load a font from bytes.
    pub fn load_font_data(&mut self, data: Vec<u8>) {
        self.inner
            .load_font_source(fontdb::Source::Binary(Arc::new(data)));
    }

    /// Load a font from a .ttf or .otf file.
    pub fn load_font_file(&mut self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        self.inner.load_font_file(path)?;
        Ok(())
    }

    /// Load fonts from a directory.
    pub fn load_fonts_dir(&mut self, path: impl AsRef<std::path::Path>) {
        self.inner.load_fonts_dir(path);
    }

    /// Query a font by family name.
    /// Returns true if the font family is available.
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

    /// Get the number of fonts loaded.
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
pub struct FontSystem {
    inner: Arc<RwLock<cosmic_text::FontSystem>>,
}

impl FontSystem {
    /// Create a new font system with the given font database.
    pub fn new(db: FontDatabase) -> Self {
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
        Self::new(FontDatabase::new())
    }

    pub(crate) fn inner(&self) -> Arc<RwLock<cosmic_text::FontSystem>> {
        self.inner.clone()
    }
}

impl Default for FontSystem {
    fn default() -> Self {
        Self::with_system_fonts()
    }
}

/// Font weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Thin,
    ExtraLight,
    Light,
    Normal,
    Medium,
    SemiBold,
    Bold,
    ExtraBold,
    Black,
}

impl FontWeight {
    pub(crate) fn to_cosmic(self) -> cosmic_text::Weight {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl FontStyle {
    pub(crate) fn to_cosmic(self) -> cosmic_text::Style {
        match self {
            FontStyle::Normal => cosmic_text::Style::Normal,
            FontStyle::Italic => cosmic_text::Style::Italic,
            FontStyle::Oblique => cosmic_text::Style::Oblique,
        }
    }
}

/// Font stretch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

impl FontStretch {
    pub(crate) fn to_cosmic(self) -> cosmic_text::Stretch {
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

/// Font attributes.
#[derive(Debug, Clone)]
pub struct FontAttributes {
    pub family: String,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub stretch: FontStretch,
}

impl FontAttributes {
    /// Create font attributes with a specific family name.
    /// Common system fonts:
    /// - "sans-serif", "serif", "monospace" (generic families)
    /// - "Arial", "Helvetica", "Times New Roman", "Courier New" (Windows)
    /// - "San Francisco", "Helvetica Neue" (macOS)
    /// - "Ubuntu", "Noto Sans", "DejaVu Sans" (Linux)
    pub fn new(family: impl Into<String>) -> Self {
        Self {
            family: family.into(),
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
            stretch: FontStretch::Normal,
        }
    }

    /// Create font attributes for a sans-serif font.
    pub fn sans_serif() -> Self {
        Self::new("sans-serif")
    }

    /// Create font attributes for a serif font.
    pub fn serif() -> Self {
        Self::new("serif")
    }

    /// Create font attributes for a monospace font.
    pub fn monospace() -> Self {
        Self::new("monospace")
    }

    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    pub fn style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }

    pub fn stretch(mut self, stretch: FontStretch) -> Self {
        self.stretch = stretch;
        self
    }

    pub(crate) fn to_cosmic(&self) -> Attrs<'_> {
        Attrs::new()
            .family(cosmic_text::Family::Name(&self.family))
            .weight(self.weight.to_cosmic())
            .style(self.style.to_cosmic())
            .stretch(self.stretch.to_cosmic())
    }
}

impl Default for FontAttributes {
    fn default() -> Self {
        Self {
            family: String::from("sans-serif"),
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
            stretch: FontStretch::Normal,
        }
    }
}
