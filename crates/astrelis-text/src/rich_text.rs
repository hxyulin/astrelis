//! Rich text formatting with styled spans.
//!
//! Provides support for mixing multiple styles within a single text block.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_text::*;
//!
//! // Create rich text with spans
//! let mut rich = RichText::new();
//! rich.push("This is ", TextSpanStyle::default());
//! rich.push_bold("bold");
//! rich.push(" and ");
//! rich.push_italic("italic");
//! rich.push(" text.");
//!
//! // Or use the builder pattern
//! let rich = RichTextBuilder::new()
//!     .text("This is ")
//!     .bold("bold")
//!     .text(" and ")
//!     .italic("italic")
//!     .text(" text.")
//!     .build();
//!
//! // Parse from markup
//! let rich = RichText::from_markup("This is **bold** and *italic* text.");
//! ```

use crate::font::{FontAttributes, FontStyle, FontWeight};
use crate::text::{LineBreakConfig, Text, TextAlign, TextWrap, VerticalAlign};
use astrelis_render::Color;

/// A span of text with specific styling.
#[derive(Debug, Clone)]
pub struct TextSpan {
    /// The text content
    pub text: String,
    /// The style for this span
    pub style: TextSpanStyle,
}

impl TextSpan {
    /// Create a new text span.
    pub fn new(text: impl Into<String>, style: TextSpanStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// Style attributes for a text span.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct TextSpanStyle {
    /// Font size (None = inherit from parent)
    pub font_size: Option<f32>,
    /// Text color (None = inherit from parent)
    pub color: Option<Color>,
    /// Font weight (None = inherit from parent)
    pub weight: Option<FontWeight>,
    /// Font style (None = inherit from parent)
    pub style: Option<FontStyle>,
    /// Font family (None = inherit from parent)
    pub font_family: Option<String>,
    /// Underline flag
    pub underline: bool,
    /// Strikethrough flag
    pub strikethrough: bool,
    /// Background color (None = no background)
    pub background: Option<Color>,
    /// Scale factor relative to parent font size (1.0 = same size, 0.5 = half size, 2.0 = double)
    pub scale: Option<f32>,
}

impl TextSpanStyle {
    /// Create a new default text span style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set font size.
    pub fn with_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    /// Set color.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Set font weight.
    pub fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Make bold.
    pub fn bold(mut self) -> Self {
        self.weight = Some(FontWeight::Bold);
        self
    }

    /// Set font style.
    pub fn with_style(mut self, style: FontStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Make italic.
    pub fn italic(mut self) -> Self {
        self.style = Some(FontStyle::Italic);
        self
    }

    /// Set font family.
    pub fn with_family(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// Set underline.
    pub fn with_underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    /// Set strikethrough.
    pub fn with_strikethrough(mut self, strikethrough: bool) -> Self {
        self.strikethrough = strikethrough;
        self
    }

    /// Set background color.
    pub fn with_background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// Set scale factor.
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = Some(scale);
        self
    }
}


/// Rich text with multiple styled spans.
#[derive(Debug, Clone)]
pub struct RichText {
    /// The text spans
    spans: Vec<TextSpan>,
    /// Default font size for unspecified spans
    default_font_size: f32,
    /// Default color for unspecified spans
    default_color: Color,
    /// Default font attributes
    default_font_attrs: FontAttributes,
    /// Text alignment
    align: TextAlign,
    /// Vertical alignment
    vertical_align: VerticalAlign,
    /// Text wrapping mode
    wrap: TextWrap,
    /// Whether to allow breaks at hyphens
    break_at_hyphens: bool,
    /// Maximum width
    max_width: Option<f32>,
    /// Maximum height
    max_height: Option<f32>,
    /// Line height multiplier
    line_height: f32,
}

impl RichText {
    /// Create a new rich text instance.
    pub fn new() -> Self {
        Self {
            spans: Vec::new(),
            default_font_size: 16.0,
            default_color: Color::WHITE,
            default_font_attrs: FontAttributes::default(),
            align: TextAlign::Left,
            vertical_align: VerticalAlign::Top,
            wrap: TextWrap::Word,
            break_at_hyphens: true,
            max_width: None,
            max_height: None,
            line_height: 1.2,
        }
    }

    /// Add a text span.
    pub fn push(&mut self, text: impl Into<String>, style: TextSpanStyle) {
        self.spans.push(TextSpan::new(text, style));
    }

    /// Add plain text with default styling.
    pub fn push_str(&mut self, text: impl Into<String>) {
        self.spans
            .push(TextSpan::new(text, TextSpanStyle::default()));
    }

    /// Add bold text.
    pub fn push_bold(&mut self, text: impl Into<String>) {
        self.spans
            .push(TextSpan::new(text, TextSpanStyle::default().bold()));
    }

    /// Add italic text.
    pub fn push_italic(&mut self, text: impl Into<String>) {
        self.spans
            .push(TextSpan::new(text, TextSpanStyle::default().italic()));
    }

    /// Add colored text.
    pub fn push_colored(&mut self, text: impl Into<String>, color: Color) {
        self.spans.push(TextSpan::new(
            text,
            TextSpanStyle::default().with_color(color),
        ));
    }

    /// Add a span.
    pub fn push_span(&mut self, span: TextSpan) {
        self.spans.push(span);
    }

    /// Get all spans.
    pub fn spans(&self) -> &[TextSpan] {
        &self.spans
    }

    /// Set default font size.
    pub fn set_default_font_size(&mut self, size: f32) {
        self.default_font_size = size;
    }

    /// Set default color.
    pub fn set_default_color(&mut self, color: Color) {
        self.default_color = color;
    }

    /// Set default font attributes.
    pub fn set_default_font_attrs(&mut self, attrs: FontAttributes) {
        self.default_font_attrs = attrs;
    }

    /// Set text alignment.
    pub fn set_align(&mut self, align: TextAlign) {
        self.align = align;
    }

    /// Set vertical alignment.
    pub fn set_vertical_align(&mut self, align: VerticalAlign) {
        self.vertical_align = align;
    }

    /// Set text wrapping.
    pub fn set_wrap(&mut self, wrap: TextWrap) {
        self.wrap = wrap;
    }

    /// Set line breaking configuration.
    ///
    /// This provides more control than `set_wrap()` alone, allowing configuration
    /// of hyphen breaks and future UAX#14 options.
    pub fn set_line_break(&mut self, config: LineBreakConfig) {
        self.wrap = config.wrap;
        self.break_at_hyphens = config.break_at_hyphens;
    }

    /// Set maximum width.
    pub fn set_max_width(&mut self, width: Option<f32>) {
        self.max_width = width;
    }

    /// Set maximum height.
    pub fn set_max_height(&mut self, height: Option<f32>) {
        self.max_height = height;
    }

    /// Set line height multiplier.
    pub fn set_line_height(&mut self, height: f32) {
        self.line_height = height;
    }

    /// Get the full text content (concatenated spans).
    pub fn full_text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    /// Convert to a series of Text objects (one per span).
    ///
    /// This is used for rendering - each span becomes a separate Text that can be rendered.
    /// Note: This is a simplified conversion. For true rich text rendering, you'd want to
    /// integrate with cosmic-text's attributed string support.
    pub fn to_text_segments(&self) -> Vec<(Text, TextSpanStyle)> {
        let mut segments = Vec::new();

        for span in &self.spans {
            let mut text = Text::new(&span.text)
                .size(
                    span.style
                        .font_size
                        .or(span.style.scale.map(|s| self.default_font_size * s))
                        .unwrap_or(self.default_font_size),
                )
                .color(span.style.color.unwrap_or(self.default_color))
                .align(self.align)
                .vertical_align(self.vertical_align)
                .wrap(self.wrap)
                .line_height(self.line_height);

            if let Some(weight) = span.style.weight {
                text = text.weight(weight);
            } else {
                text = text.weight(self.default_font_attrs.weight);
            }

            if let Some(style) = span.style.style {
                text = text.style(style);
            } else {
                text = text.style(self.default_font_attrs.style);
            }

            if let Some(ref family) = span.style.font_family {
                text = text.font(family.clone());
            } else if !self.default_font_attrs.family.is_empty() {
                text = text.font(self.default_font_attrs.family.clone());
            }

            if let Some(width) = self.max_width {
                text = text.max_width(width);
            }

            if let Some(height) = self.max_height {
                text = text.max_height(height);
            }

            segments.push((text, span.style.clone()));
        }

        segments
    }

    /// Parse markdown-like markup into rich text.
    ///
    /// Supported syntax:
    /// - `**bold**` for bold text
    /// - `*italic*` for italic text
    /// - `__underline__` for underlined text
    /// - `~~strikethrough~~` for strikethrough text
    ///
    /// # Example
    ///
    /// ```ignore
    /// let rich = RichText::from_markup("This is **bold** and *italic* text.");
    /// ```
    pub fn from_markup(markup: &str) -> Self {
        let mut rich = RichText::new();
        let mut current = String::new();
        let mut chars = markup.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '*' => {
                    if chars.peek() == Some(&'*') {
                        // Bold: **text**
                        chars.next(); // consume second *

                        if !current.is_empty() {
                            rich.push_str(current.clone());
                            current.clear();
                        }

                        let mut bold_text = String::new();
                        let mut found_end = false;

                        while let Some(ch) = chars.next() {
                            if ch == '*' && chars.peek() == Some(&'*') {
                                chars.next(); // consume second *
                                found_end = true;
                                break;
                            }
                            bold_text.push(ch);
                        }

                        if found_end {
                            rich.push_bold(bold_text);
                        } else {
                            current.push_str("**");
                            current.push_str(&bold_text);
                        }
                    } else {
                        // Italic: *text*
                        if !current.is_empty() {
                            rich.push_str(current.clone());
                            current.clear();
                        }

                        let mut italic_text = String::new();
                        let mut found_end = false;

                        for ch in chars.by_ref() {
                            if ch == '*' {
                                found_end = true;
                                break;
                            }
                            italic_text.push(ch);
                        }

                        if found_end {
                            rich.push_italic(italic_text);
                        } else {
                            current.push('*');
                            current.push_str(&italic_text);
                        }
                    }
                }
                '_' => {
                    if chars.peek() == Some(&'_') {
                        // Underline: __text__
                        chars.next(); // consume second _

                        if !current.is_empty() {
                            rich.push_str(current.clone());
                            current.clear();
                        }

                        let mut underline_text = String::new();
                        let mut found_end = false;

                        while let Some(ch) = chars.next() {
                            if ch == '_' && chars.peek() == Some(&'_') {
                                chars.next(); // consume second _
                                found_end = true;
                                break;
                            }
                            underline_text.push(ch);
                        }

                        if found_end {
                            rich.push(
                                underline_text,
                                TextSpanStyle::default().with_underline(true),
                            );
                        } else {
                            current.push_str("__");
                            current.push_str(&underline_text);
                        }
                    } else {
                        current.push(ch);
                    }
                }
                '~' => {
                    if chars.peek() == Some(&'~') {
                        // Strikethrough: ~~text~~
                        chars.next(); // consume second ~

                        if !current.is_empty() {
                            rich.push_str(current.clone());
                            current.clear();
                        }

                        let mut strike_text = String::new();
                        let mut found_end = false;

                        while let Some(ch) = chars.next() {
                            if ch == '~' && chars.peek() == Some(&'~') {
                                chars.next(); // consume second ~
                                found_end = true;
                                break;
                            }
                            strike_text.push(ch);
                        }

                        if found_end {
                            rich.push(
                                strike_text,
                                TextSpanStyle::default().with_strikethrough(true),
                            );
                        } else {
                            current.push_str("~~");
                            current.push_str(&strike_text);
                        }
                    } else {
                        current.push(ch);
                    }
                }
                _ => {
                    current.push(ch);
                }
            }
        }

        if !current.is_empty() {
            rich.push_str(current);
        }

        rich
    }
}

impl Default for RichText {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating rich text with a fluent API.
pub struct RichTextBuilder {
    rich_text: RichText,
}

impl RichTextBuilder {
    /// Create a new rich text builder.
    pub fn new() -> Self {
        Self {
            rich_text: RichText::new(),
        }
    }

    /// Add plain text.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.rich_text.push_str(text);
        self
    }

    /// Add bold text.
    pub fn bold(mut self, text: impl Into<String>) -> Self {
        self.rich_text.push_bold(text);
        self
    }

    /// Add italic text.
    pub fn italic(mut self, text: impl Into<String>) -> Self {
        self.rich_text.push_italic(text);
        self
    }

    /// Add colored text.
    pub fn colored(mut self, text: impl Into<String>, color: Color) -> Self {
        self.rich_text.push_colored(text, color);
        self
    }

    /// Add a custom styled span.
    pub fn span(mut self, text: impl Into<String>, style: TextSpanStyle) -> Self {
        self.rich_text.push(text, style);
        self
    }

    /// Set default font size.
    pub fn default_size(mut self, size: f32) -> Self {
        self.rich_text.set_default_font_size(size);
        self
    }

    /// Set default color.
    pub fn default_color(mut self, color: Color) -> Self {
        self.rich_text.set_default_color(color);
        self
    }

    /// Set text alignment.
    pub fn align(mut self, align: TextAlign) -> Self {
        self.rich_text.set_align(align);
        self
    }

    /// Set vertical alignment.
    pub fn vertical_align(mut self, align: VerticalAlign) -> Self {
        self.rich_text.set_vertical_align(align);
        self
    }

    /// Set text wrapping.
    pub fn wrap(mut self, wrap: TextWrap) -> Self {
        self.rich_text.set_wrap(wrap);
        self
    }

    /// Set line breaking configuration.
    ///
    /// This provides more control than `.wrap()` alone, allowing configuration
    /// of hyphen breaks and future UAX#14 options.
    pub fn line_break(mut self, config: LineBreakConfig) -> Self {
        self.rich_text.set_line_break(config);
        self
    }

    /// Set maximum width.
    pub fn max_width(mut self, width: f32) -> Self {
        self.rich_text.set_max_width(Some(width));
        self
    }

    /// Set maximum height.
    pub fn max_height(mut self, height: f32) -> Self {
        self.rich_text.set_max_height(Some(height));
        self
    }

    /// Set line height multiplier.
    pub fn line_height(mut self, height: f32) -> Self {
        self.rich_text.set_line_height(height);
        self
    }

    /// Build the rich text.
    pub fn build(self) -> RichText {
        self.rich_text
    }
}

impl Default for RichTextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rich_text_builder() {
        let rich = RichTextBuilder::new()
            .text("This is ")
            .bold("bold")
            .text(" and ")
            .italic("italic")
            .text(" text.")
            .build();

        assert_eq!(rich.spans().len(), 5);
        assert_eq!(rich.full_text(), "This is bold and italic text.");
    }

    #[test]
    fn test_markup_parsing_bold() {
        let rich = RichText::from_markup("This is **bold** text.");
        assert_eq!(rich.spans().len(), 3);
        assert_eq!(rich.full_text(), "This is bold text.");

        assert!(rich.spans()[1].style.weight == Some(FontWeight::Bold));
    }

    #[test]
    fn test_markup_parsing_italic() {
        let rich = RichText::from_markup("This is *italic* text.");
        assert_eq!(rich.spans().len(), 3);
        assert_eq!(rich.full_text(), "This is italic text.");

        assert!(rich.spans()[1].style.style == Some(FontStyle::Italic));
    }

    #[test]
    fn test_markup_parsing_underline() {
        let rich = RichText::from_markup("This is __underlined__ text.");
        assert_eq!(rich.spans().len(), 3);
        assert_eq!(rich.full_text(), "This is underlined text.");

        assert!(rich.spans()[1].style.underline);
    }

    #[test]
    fn test_markup_parsing_strikethrough() {
        let rich = RichText::from_markup("This is ~~strikethrough~~ text.");
        assert_eq!(rich.spans().len(), 3);
        assert_eq!(rich.full_text(), "This is strikethrough text.");

        assert!(rich.spans()[1].style.strikethrough);
    }

    #[test]
    fn test_markup_parsing_mixed() {
        let rich = RichText::from_markup("This is **bold** and *italic* and __underlined__ text.");
        assert_eq!(rich.spans().len(), 7);
    }
}
