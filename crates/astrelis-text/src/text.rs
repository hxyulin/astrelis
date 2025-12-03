use cosmic_text::Color as CosmicColor;

use crate::font::{FontAttributes, FontStretch, FontStyle, FontWeight};
use astrelis_render::Color;

/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    Justified,
}

impl TextAlign {
    pub(crate) fn to_cosmic(self) -> cosmic_text::Align {
        match self {
            TextAlign::Left => cosmic_text::Align::Left,
            TextAlign::Center => cosmic_text::Align::Center,
            TextAlign::Right => cosmic_text::Align::Right,
            TextAlign::Justified => cosmic_text::Align::Justified,
        }
    }
}

/// Text wrapping mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextWrap {
    None,
    Word,
    Glyph,
}

impl TextWrap {
    pub(crate) fn to_cosmic(self) -> cosmic_text::Wrap {
        match self {
            TextWrap::None => cosmic_text::Wrap::None,
            TextWrap::Word => cosmic_text::Wrap::Word,
            TextWrap::Glyph => cosmic_text::Wrap::Glyph,
        }
    }
}

/// Convert Color to cosmic-text color.
pub(crate) fn color_to_cosmic(color: Color) -> CosmicColor {
    CosmicColor::rgba(
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
        (color.a * 255.0) as u8,
    )
}

/// Text builder for creating styled text.
pub struct Text {
    content: String,
    font_size: f32,
    line_height: f32,
    font_attrs: FontAttributes,
    color: Color,
    align: TextAlign,
    wrap: TextWrap,
    max_width: Option<f32>,
    max_height: Option<f32>,
    letter_spacing: f32,
    word_spacing: f32,
}

impl Text {
    /// Create a new text instance.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            font_size: 16.0,
            line_height: 1.2,
            font_attrs: FontAttributes::default(),
            color: Color::WHITE,
            align: TextAlign::Left,
            wrap: TextWrap::Word,
            max_width: None,
            max_height: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
        }
    }

    /// Set the font size in pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the line height multiplier.
    pub fn line_height(mut self, height: f32) -> Self {
        self.line_height = height;
        self
    }

    /// Set the font family.
    pub fn font(mut self, family: impl Into<String>) -> Self {
        self.font_attrs.family = family.into();
        self
    }

    /// Set the font weight.
    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.font_attrs.weight = weight;
        self
    }

    /// Set the font style.
    pub fn style(mut self, style: FontStyle) -> Self {
        self.font_attrs.style = style;
        self
    }

    /// Set the font stretch.
    pub fn stretch(mut self, stretch: FontStretch) -> Self {
        self.font_attrs.stretch = stretch;
        self
    }

    /// Set font attributes.
    pub fn font_attrs(mut self, attrs: FontAttributes) -> Self {
        self.font_attrs = attrs;
        self
    }

    /// Set the text color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set the text alignment.
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Set the text wrapping mode.
    pub fn wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        self
    }

    /// Set the maximum width for text wrapping.
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Set the maximum height for text.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Set letter spacing in pixels.
    pub fn letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = spacing;
        self
    }

    /// Set word spacing in pixels.
    pub fn word_spacing(mut self, spacing: f32) -> Self {
        self.word_spacing = spacing;
        self
    }

    /// Make the text bold.
    pub fn bold(self) -> Self {
        self.weight(FontWeight::Bold)
    }

    /// Make the text italic.
    pub fn italic(self) -> Self {
        self.style(FontStyle::Italic)
    }

    // Getters

    pub fn get_content(&self) -> &str {
        &self.content
    }

    pub fn get_font_size(&self) -> f32 {
        self.font_size
    }

    pub fn get_line_height(&self) -> f32 {
        self.line_height
    }

    pub fn get_font_attrs(&self) -> &FontAttributes {
        &self.font_attrs
    }

    pub fn get_color(&self) -> Color {
        self.color
    }

    pub fn get_align(&self) -> TextAlign {
        self.align
    }

    pub fn get_wrap(&self) -> TextWrap {
        self.wrap
    }

    pub fn get_max_width(&self) -> Option<f32> {
        self.max_width
    }

    pub fn get_max_height(&self) -> Option<f32> {
        self.max_height
    }

    pub fn get_letter_spacing(&self) -> f32 {
        self.letter_spacing
    }

    pub fn get_word_spacing(&self) -> f32 {
        self.word_spacing
    }
}

impl Default for Text {
    fn default() -> Self {
        Self::new("")
    }
}
