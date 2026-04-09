//! Text builder and styling types.
//!
//! The [`Text`] struct provides a builder-pattern API for creating styled text
//! with configurable font, size, color, alignment, wrapping, and decorations.

use astrelis_core::color::Color;
use astrelis_core::math::Vec2;

use crate::decoration::TextDecoration;
use crate::effects::TextEffect;
use crate::font::{FontAttributes, FontStretch, FontStyle, FontWeight};
use crate::sdf::TextRenderMode;

/// Text alignment (horizontal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Left-aligned (default).
    #[default]
    Left,
    /// Center-aligned.
    Center,
    /// Right-aligned.
    Right,
    /// Justified.
    Justified,
}

impl TextAlign {
    /// Convert to cosmic-text alignment.
    pub(crate) fn to_cosmic(self) -> cosmic_text::Align {
        match self {
            TextAlign::Left => cosmic_text::Align::Left,
            TextAlign::Center => cosmic_text::Align::Center,
            TextAlign::Right => cosmic_text::Align::Right,
            TextAlign::Justified => cosmic_text::Align::Justified,
        }
    }
}

/// Vertical alignment for text within a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalAlign {
    /// Align text to the top (default).
    #[default]
    Top,
    /// Center text vertically.
    Center,
    /// Align text to the bottom.
    Bottom,
}

/// Text wrapping mode controlling how text breaks across lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextWrap {
    /// No wrapping - text extends past boundaries on a single line.
    None,
    /// Word-based wrapping (default).
    #[default]
    Word,
    /// Character/glyph-based wrapping.
    Glyph,
    /// Word wrapping with glyph fallback for long words.
    WordOrGlyph,
}

/// Line break configuration for fine-grained control.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineBreakConfig {
    /// Wrapping mode.
    pub wrap: TextWrap,
    /// Whether to allow breaks at hyphens.
    pub break_at_hyphens: bool,
}

impl Default for LineBreakConfig {
    fn default() -> Self {
        Self {
            wrap: TextWrap::Word,
            break_at_hyphens: true,
        }
    }
}

/// Measured text metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    /// Text width in logical pixels.
    pub width: f32,
    /// Text height in logical pixels.
    pub height: f32,
    /// Distance from top to the first baseline.
    pub baseline_offset: f32,
    /// Number of lines after wrapping.
    pub line_count: u32,
}

/// Convert a [`Color`] to a cosmic-text [`cosmic_text::Color`].
/// Convert a [`Color`] to a cosmic-text color.
pub fn color_to_cosmic(color: Color) -> cosmic_text::Color {
    cosmic_text::Color::rgba(
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
        (color.a * 255.0) as u8,
    )
}

/// Styled text with builder-pattern configuration.
///
/// # Example
///
/// ```
/// use astrelis_text::Text;
/// use astrelis_core::color::Color;
///
/// let text = Text::new("Hello, World!")
///     .size(24.0)
///     .color(Color::WHITE)
///     .bold();
/// ```
#[derive(Debug, Clone)]
pub struct Text {
    /// The text content.
    pub content: String,
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Text color.
    pub text_color: Color,
    /// Horizontal alignment.
    pub align: TextAlign,
    /// Vertical alignment.
    pub vertical_align: VerticalAlign,
    /// Text wrapping mode.
    pub wrap: TextWrap,
    /// Line height multiplier (e.g. 1.2 = 120% of font size).
    pub line_height: f32,
    /// Maximum width for wrapping (None = no limit).
    pub max_width: Option<f32>,
    /// Maximum height for clipping (None = no limit).
    pub max_height: Option<f32>,
    /// Font weight.
    pub weight: FontWeight,
    /// Font style.
    pub font_style: FontStyle,
    /// Font stretch.
    pub stretch: FontStretch,
    /// Font family name (None = system default).
    pub font_family: Option<String>,
    /// Letter spacing in logical pixels.
    pub letter_spacing: f32,
    /// Word spacing in logical pixels.
    pub word_spacing: f32,
    /// Visual effects (shadow, outline, glow).
    pub effects: Vec<TextEffect>,
    /// Text decorations (underline, strikethrough, background).
    pub decorations: Vec<TextDecoration>,
    /// Forced render mode (None = auto).
    pub render_mode: Option<TextRenderMode>,
}

impl Text {
    /// Create new text with the given content.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            font_size: 16.0,
            text_color: Color::WHITE,
            align: TextAlign::Left,
            vertical_align: VerticalAlign::Top,
            wrap: TextWrap::Word,
            line_height: 1.2,
            max_width: None,
            max_height: None,
            weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            stretch: FontStretch::Normal,
            font_family: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            effects: Vec::new(),
            decorations: Vec::new(),
            render_mode: None,
        }
    }

    /// Set font size in logical pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set text color.
    pub fn color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }

    /// Set horizontal alignment.
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Set vertical alignment.
    pub fn vertical_align(mut self, align: VerticalAlign) -> Self {
        self.vertical_align = align;
        self
    }

    /// Set text wrapping mode.
    pub fn wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        self
    }

    /// Set line height multiplier.
    pub fn line_height(mut self, multiplier: f32) -> Self {
        self.line_height = multiplier;
        self
    }

    /// Set maximum width for wrapping.
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Set maximum height for clipping.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Set font weight.
    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Set font style.
    pub fn style(mut self, style: FontStyle) -> Self {
        self.font_style = style;
        self
    }

    /// Set font stretch.
    pub fn stretch(mut self, stretch: FontStretch) -> Self {
        self.stretch = stretch;
        self
    }

    /// Set font family.
    pub fn font(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// Set letter spacing.
    pub fn letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = spacing;
        self
    }

    /// Set word spacing.
    pub fn word_spacing(mut self, spacing: f32) -> Self {
        self.word_spacing = spacing;
        self
    }

    /// Shorthand for bold weight.
    pub fn bold(self) -> Self {
        self.weight(FontWeight::Bold)
    }

    /// Shorthand for italic style.
    pub fn italic(self) -> Self {
        self.style(FontStyle::Italic)
    }

    /// Add a visual effect.
    pub fn with_effect(mut self, effect: TextEffect) -> Self {
        self.effects.push(effect);
        self
    }

    /// Add a drop shadow effect.
    pub fn with_shadow(self, offset: Vec2, color: Color) -> Self {
        self.with_effect(TextEffect::shadow(offset, color))
    }

    /// Add an outline effect.
    pub fn with_outline(self, width: f32, color: Color) -> Self {
        self.with_effect(TextEffect::outline(width, color))
    }

    /// Add a glow effect.
    pub fn with_glow(self, radius: f32, color: Color, intensity: f32) -> Self {
        self.with_effect(TextEffect::glow(radius, color, intensity))
    }

    /// Add a text decoration.
    pub fn decoration(mut self, decoration: TextDecoration) -> Self {
        self.decorations.push(decoration);
        self
    }

    /// Force SDF rendering mode.
    pub fn sdf(mut self) -> Self {
        self.render_mode = Some(TextRenderMode::SDF { spread: 4.0 });
        self
    }

    /// Force bitmap rendering mode.
    pub fn bitmap(mut self) -> Self {
        self.render_mode = Some(TextRenderMode::Bitmap);
        self
    }

    /// Get font attributes for this text.
    pub fn font_attributes(&self) -> FontAttributes {
        let mut attrs = FontAttributes::default();
        attrs.weight = self.weight;
        attrs.style = self.font_style;
        attrs.stretch = self.stretch;
        if let Some(ref family) = self.font_family {
            attrs.family = family.clone();
        }
        attrs
    }

    /// Check if this text has any effects that require SDF rendering.
    pub fn needs_sdf(&self) -> bool {
        !self.effects.is_empty()
            || matches!(self.render_mode, Some(TextRenderMode::SDF { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_defaults() {
        let text = Text::new("Hello");
        assert_eq!(text.content, "Hello");
        assert_eq!(text.font_size, 16.0);
        assert_eq!(text.text_color, Color::WHITE);
        assert_eq!(text.align, TextAlign::Left);
        assert_eq!(text.wrap, TextWrap::Word);
        assert_eq!(text.weight, FontWeight::Normal);
    }

    #[test]
    fn test_text_builder() {
        let text = Text::new("Test")
            .size(24.0)
            .color(Color::RED)
            .bold()
            .italic()
            .align(TextAlign::Center)
            .max_width(200.0);

        assert_eq!(text.font_size, 24.0);
        assert_eq!(text.text_color, Color::RED);
        assert_eq!(text.weight, FontWeight::Bold);
        assert_eq!(text.font_style, FontStyle::Italic);
        assert_eq!(text.align, TextAlign::Center);
        assert_eq!(text.max_width, Some(200.0));
    }

    #[test]
    fn test_text_sdf_mode() {
        let text = Text::new("SDF").sdf();
        assert!(text.needs_sdf());
        assert!(matches!(text.render_mode, Some(TextRenderMode::SDF { .. })));
    }

    #[test]
    fn test_text_bitmap_mode() {
        let text = Text::new("Bitmap").bitmap();
        assert!(!text.needs_sdf());
        assert!(matches!(text.render_mode, Some(TextRenderMode::Bitmap)));
    }

    #[test]
    fn test_text_with_effects() {
        let text = Text::new("Effects")
            .with_shadow(Vec2::new(2.0, 2.0), Color::BLACK)
            .with_outline(1.0, Color::WHITE);

        assert_eq!(text.effects.len(), 2);
        assert!(text.needs_sdf());
    }

    #[test]
    fn test_text_align_cosmic() {
        assert!(matches!(
            TextAlign::Left.to_cosmic(),
            cosmic_text::Align::Left
        ));
        assert!(matches!(
            TextAlign::Center.to_cosmic(),
            cosmic_text::Align::Center
        ));
        assert!(matches!(
            TextAlign::Right.to_cosmic(),
            cosmic_text::Align::Right
        ));
        assert!(matches!(
            TextAlign::Justified.to_cosmic(),
            cosmic_text::Align::Justified
        ));
    }

    #[test]
    fn test_color_to_cosmic() {
        let color = Color::new(1.0, 0.5, 0.0, 1.0);
        let cosmic = color_to_cosmic(color);
        // cosmic_text::Color stores as u32 RGBA
        assert_eq!(cosmic.r(), 255);
        assert_eq!(cosmic.g(), 127); // 0.5 * 255 = 127.5 -> 127
        assert_eq!(cosmic.b(), 0);
        assert_eq!(cosmic.a(), 255);
    }
}
