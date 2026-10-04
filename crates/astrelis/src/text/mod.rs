//! GPU-independent font loading, advanced shaping, and retained paragraph layout.
//!
//! Keep one [`TextSystem`] per application and one [`TextBuffer`] per retained text
//! item. Font discovery is explicit. [`TextBuffer::layout`] evaluates dirty state
//! and returns an immutable, shared [`TextLayout`]; an unchanged buffer returns
//! the same snapshot without reshaping. Width/wrap/alignment changes retain shaped
//! runs. Snapshots own their source text and font references, so they survive later
//! edits, font loading, and dropping the system. No GPU initialization is required.
//!
//! All metrics use application-selected local units, X right/Y down. Layout does
//! not apply DPI, rasterize glyphs, or compute pixel ink bounds. Source clusters are
//! UTF-8 byte ranges, not character indices or caret boundaries. Glyphs can combine
//! multiple characters or share a source cluster. Missing glyphs are reported by
//! source range, while available fonts' `.notdef` glyphs remain in the layout.
//!
//! ```
//! use astrelis::text::{TextBuffer, TextStyle, TextSystem};
//! let mut text = TextSystem::new();
//! text.load_font(include_bytes!("../../tests/fonts/SourceSans3-Regular.otf"))?;
//! let mut label = TextBuffer::new();
//! label.set_text("Hello, office!", TextStyle::new().family("Source Sans 3"))?;
//! label.set_width(Some(200.0))?;
//! let layout = label.layout(&mut text)?;
//! assert!(!layout.glyphs().is_empty());
//! assert!(layout.size()[0] > 0.0);
//! assert!(std::sync::Arc::ptr_eq(&layout, &label.layout(&mut text)?));
//! # Ok::<(), astrelis::text::TextError>(())
//! ```
//!
//! The CPU backend is re-exported as [`cosmic`] for font inspection/custom consumers.
//! [`TextSystem::as_cosmic`] and [`TextFont::as_cosmic`] expose read-only backend
//! access. Buffer mutation goes through validated setters to preserve invalidation.
//! GPU text preparation, coverage/color atlases, and distance-field rendering are
//! separate milestones; this module does not introduce placeholder GPU APIs.

mod buffer;
mod layout;
mod style;
mod system;

pub use buffer::TextBuffer;
pub use cosmic_text as cosmic;
pub use layout::{TextFont, TextGlyph, TextLayout, TextLine};
pub use style::{FontFamily, FontSlant, FontStretch, TextAlign, TextStyle, TextWrap};
pub use system::{FontId, FontInfo, TextSystem};

/// Font loading or paragraph layout failure. Rejected setters leave the buffer unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextError {
    /// The provided data contains no faces usable by the shaping backend.
    InvalidFont,
    /// Font size/line height must be positive and finite, weight in `1..=1000`,
    /// tracking finite, and feature tags four printable ASCII bytes.
    InvalidStyle,
    /// Width must be finite and nonnegative, or `None` for unconstrained width.
    InvalidWidth,
    /// Nonempty content requires at least one usable font.
    NoFonts,
    /// Selected font data cannot be loaded or parsed.
    FontUnavailable,
    /// Computed positions or line metrics overflowed finite floating-point units.
    LayoutOverflow,
}
impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidFont => "font data contains no usable faces",
            Self::InvalidStyle => "text metrics must be positive and finite, weight in 1..=1000, tracking finite, and feature tags printable ASCII",
            Self::InvalidWidth => "text width must be finite and nonnegative",
            Self::NoFonts => "load a font before laying out nonempty text",
            Self::FontUnavailable => "selected font data is unavailable or invalid",
            Self::LayoutOverflow => "text layout positions or metrics exceed finite units",
        })
    }
}
impl std::error::Error for TextError {}

#[cfg(test)]
mod tests;
