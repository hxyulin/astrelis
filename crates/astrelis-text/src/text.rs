use cosmic_text::Color as CosmicColor;

use crate::effects::{TextEffect, TextEffects};
use crate::font::{FontAttributes, FontStretch, FontStyle, FontWeight};
use crate::sdf::TextRenderMode;
use astrelis_core::math::Vec2;
use astrelis_render::Color;

/// Text alignment (horizontal).
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

/// Vertical alignment for text within a container.
///
/// Controls how text is positioned vertically within its allocated space.
/// Position coordinates always represent the top-left corner of the text's bounding box,
/// and vertical alignment adjusts the text within that space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum VerticalAlign {
    /// Align text to the top of the container (default).
    #[default]
    Top,
    /// Center text vertically within the container.
    Center,
    /// Align text to the bottom of the container.
    Bottom,
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

/// Font metrics for text layout and positioning.
///
/// These metrics describe the vertical characteristics of a font at a given size,
/// useful for precise text layout and baseline alignment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    /// The ascent: distance from baseline to the top of the tallest glyph.
    pub ascent: f32,
    /// The descent: distance from baseline to the bottom of the lowest glyph (positive value).
    pub descent: f32,
    /// The line height: total vertical space for a line of text.
    pub line_height: f32,
    /// The baseline offset from the top of the bounding box.
    /// This is typically equal to ascent for top-left positioned text.
    pub baseline_offset: f32,
}

/// Text builder for creating styled text.
///
/// Text positioning uses a top-left coordinate system where (0, 0) is the top-left corner
/// and Y increases downward, consistent with UI layout systems like CSS and Flutter.
///
/// ## SDF Effects
///
/// Text supports effects like shadows, outlines, and glows via SDF (Signed Distance Field)
/// rendering. When effects are present, the text automatically uses SDF mode:
///
/// ```ignore
/// let text = Text::new("Hello")
///     .size(24.0)
///     .with_shadow(Vec2::new(2.0, 2.0), 2.0, Color::rgba(0.0, 0.0, 0.0, 0.5))
///     .with_outline(1.5, Color::BLACK);
/// ```
pub struct Text {
    content: String,
    font_size: f32,
    line_height: f32,
    font_attrs: FontAttributes,
    color: Color,
    align: TextAlign,
    vertical_align: VerticalAlign,
    wrap: TextWrap,
    max_width: Option<f32>,
    max_height: Option<f32>,
    letter_spacing: f32,
    word_spacing: f32,
    /// Optional text effects (shadows, outlines, glows)
    effects: Option<TextEffects>,
    /// Render mode (Bitmap or SDF) - auto-selected when effects are present
    render_mode: Option<TextRenderMode>,
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
            vertical_align: VerticalAlign::Top,
            wrap: TextWrap::Word,
            max_width: None,
            max_height: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            effects: None,
            render_mode: None,
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

    /// Set the text alignment (horizontal).
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Set the vertical alignment.
    pub fn vertical_align(mut self, vertical_align: VerticalAlign) -> Self {
        self.vertical_align = vertical_align;
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

    pub fn get_vertical_align(&self) -> VerticalAlign {
        self.vertical_align
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

    // ========== Effects Builder Methods ==========

    /// Add a single text effect.
    ///
    /// Effects are rendered using SDF (Signed Distance Field) rendering, which
    /// enables high-quality shadows, outlines, and glows at any scale.
    ///
    /// Multiple effects can be combined by chaining calls. Effects are rendered
    /// in priority order: shadows first (background), then outlines (foreground).
    ///
    /// # Arguments
    ///
    /// * `effect` - The text effect to add
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::{Text, TextEffect, Color};
    /// use astrelis_core::math::Vec2;
    ///
    /// // Single shadow effect
    /// let text = Text::new("Hello")
    ///     .size(32.0)
    ///     .with_effect(TextEffect::shadow(
    ///         Vec2::new(2.0, 2.0),
    ///         Color::rgba(0.0, 0.0, 0.0, 0.5)
    ///     ));
    ///
    /// // Combine shadow and outline
    /// let text = Text::new("Bold")
    ///     .size(48.0)
    ///     .with_effect(TextEffect::shadow(
    ///         Vec2::new(2.0, 2.0),
    ///         Color::BLACK
    ///     ))
    ///     .with_effect(TextEffect::outline(
    ///         2.0,
    ///         Color::WHITE
    ///     ));
    /// ```
    pub fn with_effect(mut self, effect: TextEffect) -> Self {
        let effects = self.effects.get_or_insert_with(TextEffects::new);
        effects.add(effect);
        self
    }

    /// Add multiple text effects at once.
    pub fn with_effects(mut self, effects: TextEffects) -> Self {
        self.effects = Some(effects);
        self
    }

    /// Add a shadow effect.
    ///
    /// Creates a drop shadow behind the text. This is the most commonly used effect
    /// for improving text readability on varied backgrounds.
    ///
    /// # Arguments
    ///
    /// * `offset` - Shadow offset in pixels (x, y). Positive values offset down and right.
    /// * `color` - Shadow color (typically semi-transparent black)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::{Text, Color};
    /// use astrelis_core::math::Vec2;
    ///
    /// // Standard drop shadow (2px right and down)
    /// let text = Text::new("Readable")
    ///     .size(24.0)
    ///     .with_shadow(Vec2::new(2.0, 2.0), Color::rgba(0.0, 0.0, 0.0, 0.5));
    /// ```
    pub fn with_shadow(self, offset: Vec2, color: Color) -> Self {
        self.with_effect(TextEffect::shadow(offset, color))
    }

    /// Add a blurred shadow effect for softer appearance.
    ///
    /// Creates a drop shadow with a blur radius, producing a softer, more natural
    /// shadow that's useful for headings and titles.
    ///
    /// # Arguments
    ///
    /// * `offset` - Shadow offset in pixels (x, y)
    /// * `blur_radius` - Blur radius in pixels (0 = hard edge, 2-5 = soft shadow)
    /// * `color` - Shadow color (typically semi-transparent)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::{Text, Color};
    /// use astrelis_core::math::Vec2;
    ///
    /// // Soft shadow for a heading
    /// let text = Text::new("Title")
    ///     .size(48.0)
    ///     .with_shadow_blurred(
    ///         Vec2::new(3.0, 3.0),
    ///         4.0,  // 4px blur radius
    ///         Color::rgba(0.0, 0.0, 0.0, 0.6)
    ///     );
    /// ```
    pub fn with_shadow_blurred(self, offset: Vec2, blur_radius: f32, color: Color) -> Self {
        self.with_effect(TextEffect::shadow_blurred(offset, blur_radius, color))
    }

    /// Add an outline effect around the text.
    ///
    /// Creates a stroke around text characters, useful for making text stand out
    /// against complex backgrounds or creating stylized text.
    ///
    /// # Arguments
    ///
    /// * `width` - Outline width in pixels (typically 1-3px)
    /// * `color` - Outline color (often contrasting with text color)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::{Text, Color};
    ///
    /// // White text with black outline (classic game text style)
    /// let text = Text::new("Game Text")
    ///     .size(32.0)
    ///     .color(Color::WHITE)
    ///     .with_outline(2.0, Color::BLACK);
    ///
    /// // Bold outline for emphasis
    /// let text = Text::new("Important!")
    ///     .size(40.0)
    ///     .color(Color::YELLOW)
    ///     .with_outline(3.0, Color::RED);
    /// ```
    pub fn with_outline(self, width: f32, color: Color) -> Self {
        self.with_effect(TextEffect::outline(width, color))
    }

    /// Add a glow effect around the text.
    ///
    /// Creates a soft luminous halo around text, useful for magical, sci-fi,
    /// or neon-style text effects.
    ///
    /// # Arguments
    ///
    /// * `radius` - Glow radius in pixels (typically 3-10px)
    /// * `color` - Glow color (often bright, saturated colors)
    /// * `intensity` - Glow intensity multiplier (0.5 to 1.0 for subtle, > 1.0 for intense)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::{Text, Color};
    ///
    /// // Neon blue glow
    /// let text = Text::new("Cyber")
    ///     .size(36.0)
    ///     .color(Color::CYAN)
    ///     .with_glow(6.0, Color::BLUE, 0.8);
    ///
    /// // Intense magical glow
    /// let text = Text::new("Magic")
    ///     .size(40.0)
    ///     .color(Color::WHITE)
    ///     .with_glow(8.0, Color::rgba(1.0, 0.0, 1.0, 1.0), 1.2);
    /// ```
    pub fn with_glow(self, radius: f32, color: Color, intensity: f32) -> Self {
        self.with_effect(TextEffect::glow(radius, color, intensity))
    }

    /// Set the render mode (Bitmap or SDF).
    ///
    /// By default, render mode is auto-selected based on font size and effects:
    /// - Bitmap for small text (< 24px) without effects - sharper at small sizes
    /// - SDF for large text (>= 24px) or text with effects - scalable and smooth
    ///
    /// Use this method to override the automatic selection.
    ///
    /// # Arguments
    ///
    /// * `mode` - The render mode to use:
    ///   - `TextRenderMode::Bitmap` - Traditional rasterized glyphs
    ///   - `TextRenderMode::SDF { spread }` - Distance field rendering
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::{Text, TextRenderMode};
    ///
    /// // Force bitmap even for large text
    /// let text = Text::new("Large but Sharp")
    ///     .size(48.0)
    ///     .render_mode(TextRenderMode::Bitmap);
    ///
    /// // Force SDF with custom spread
    /// let text = Text::new("Custom SDF")
    ///     .size(20.0)
    ///     .render_mode(TextRenderMode::SDF { spread: 6.0 });
    /// ```
    pub fn render_mode(mut self, mode: TextRenderMode) -> Self {
        self.render_mode = Some(mode);
        self
    }

    /// Force SDF rendering mode with default spread.
    ///
    /// Useful for text that needs to scale smoothly or maintain quality at various
    /// sizes. Equivalent to `.render_mode(TextRenderMode::SDF { spread: 4.0 })`.
    ///
    /// # When to Use
    ///
    /// - Text that will be animated or scaled
    /// - Text in UI elements that change size
    /// - High-DPI displays where extra sharpness helps
    /// - When preparing text for future effects
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::Text;
    ///
    /// // Small text that will be scaled up smoothly
    /// let text = Text::new("UI Label")
    ///     .size(14.0)
    ///     .sdf();  // Force SDF for smooth scaling
    /// ```
    pub fn sdf(self) -> Self {
        self.render_mode(TextRenderMode::SDF { spread: 4.0 })
    }

    /// Get the text effects, if any.
    pub fn get_effects(&self) -> Option<&TextEffects> {
        self.effects.as_ref()
    }

    /// Get the render mode, if explicitly set.
    pub fn get_render_mode(&self) -> Option<TextRenderMode> {
        self.render_mode
    }

    /// Check if this text has any effects configured.
    pub fn has_effects(&self) -> bool {
        self.effects
            .as_ref()
            .map(|e| e.has_enabled_effects())
            .unwrap_or(false)
    }

    /// Determine the appropriate render mode for this text.
    ///
    /// Returns the explicitly set mode via `.render_mode()` or `.sdf()`, or auto-selects
    /// based on font size and effects using the hybrid rendering strategy.
    ///
    /// # Auto-Selection Logic
    ///
    /// If no explicit mode is set:
    /// - Font size >= 24px → SDF (better scaling for large text)
    /// - Has effects → SDF (required for shadows, outlines, glows)
    /// - Otherwise → Bitmap (sharper for small UI text)
    ///
    /// # Returns
    ///
    /// The render mode that will be used when this text is rendered
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::{Text, TextRenderMode};
    ///
    /// let text = Text::new("Hello").size(32.0);
    /// assert!(text.effective_render_mode().is_sdf());  // Auto-selected SDF for 32px
    ///
    /// let text = Text::new("Small").size(14.0);
    /// assert!(!text.effective_render_mode().is_sdf());  // Auto-selected Bitmap for 14px
    ///
    /// let text = Text::new("Effects").size(16.0).with_shadow(...);
    /// assert!(text.effective_render_mode().is_sdf());  // Auto-selected SDF for effects
    /// ```
    pub fn effective_render_mode(&self) -> TextRenderMode {
        // If explicitly set, use that
        if let Some(mode) = self.render_mode {
            return mode;
        }

        // Auto-select: SDF for effects or large text, bitmap otherwise
        if self.has_effects() || self.font_size >= 24.0 {
            TextRenderMode::SDF { spread: 4.0 }
        } else {
            TextRenderMode::Bitmap
        }
    }
}

impl Default for Text {
    fn default() -> Self {
        Self::new("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_with_effect() {
        use crate::effects::TextEffect;

        let text = Text::new("Hello")
            .with_effect(TextEffect::shadow(Vec2::new(1.0, 1.0), Color::BLACK));

        assert!(text.has_effects());
        assert_eq!(text.get_effects().unwrap().effects().len(), 1);
    }

    #[test]
    fn test_text_with_shadow() {
        let text = Text::new("Hello")
            .with_shadow(Vec2::new(2.0, 2.0), Color::BLACK);

        assert!(text.has_effects());
        let effects = text.get_effects().unwrap();
        assert_eq!(effects.effects().len(), 1);
    }

    #[test]
    fn test_text_with_shadow_blurred() {
        let text = Text::new("Hello")
            .with_shadow_blurred(Vec2::new(2.0, 2.0), 1.5, Color::BLACK);

        assert!(text.has_effects());
    }

    #[test]
    fn test_text_with_outline() {
        let text = Text::new("Hello")
            .with_outline(1.0, Color::WHITE);

        assert!(text.has_effects());
    }

    #[test]
    fn test_text_with_glow() {
        let text = Text::new("Hello")
            .with_glow(5.0, Color::BLUE, 0.8);

        assert!(text.has_effects());
    }

    #[test]
    fn test_text_with_multiple_effects() {
        let text = Text::new("Hello")
            .with_shadow(Vec2::new(1.0, 1.0), Color::BLACK)
            .with_outline(1.0, Color::WHITE)
            .with_glow(3.0, Color::BLUE, 0.5);

        assert!(text.has_effects());
        let effects = text.get_effects().unwrap();
        assert_eq!(effects.effects().len(), 3);
    }

    #[test]
    fn test_text_render_mode_explicit() {
        let text = Text::new("Hello")
            .render_mode(TextRenderMode::SDF { spread: 6.0 });

        assert_eq!(text.get_render_mode(), Some(TextRenderMode::SDF { spread: 6.0 }));
    }

    #[test]
    fn test_text_sdf() {
        let text = Text::new("Hello").sdf();

        assert!(text.get_render_mode().is_some());
        assert!(text.get_render_mode().unwrap().is_sdf());
    }

    #[test]
    fn test_text_effective_render_mode_small_no_effects() {
        let text = Text::new("Hello").size(12.0);

        let mode = text.effective_render_mode();
        assert_eq!(mode, TextRenderMode::Bitmap);
    }

    #[test]
    fn test_text_effective_render_mode_large_no_effects() {
        let text = Text::new("Hello").size(32.0);

        let mode = text.effective_render_mode();
        assert!(mode.is_sdf());
    }

    #[test]
    fn test_text_effective_render_mode_small_with_effects() {
        let text = Text::new("Hello")
            .size(12.0)
            .with_shadow(Vec2::new(1.0, 1.0), Color::BLACK);

        let mode = text.effective_render_mode();
        assert!(mode.is_sdf());
    }

    #[test]
    fn test_text_effective_render_mode_explicit_overrides() {
        // Explicit mode should override auto-selection
        let text = Text::new("Hello")
            .size(12.0)
            .with_shadow(Vec2::new(1.0, 1.0), Color::BLACK)
            .render_mode(TextRenderMode::Bitmap);

        let mode = text.effective_render_mode();
        assert_eq!(mode, TextRenderMode::Bitmap);
    }

    #[test]
    fn test_text_has_effects_false() {
        let text = Text::new("Hello");

        assert!(!text.has_effects());
    }

    #[test]
    fn test_text_has_effects_true() {
        let text = Text::new("Hello")
            .with_shadow(Vec2::new(1.0, 1.0), Color::BLACK);

        assert!(text.has_effects());
    }

    #[test]
    fn test_text_has_effects_disabled() {
        use crate::effects::{TextEffect, TextEffects};

        let mut effects = TextEffects::new();
        let mut effect = TextEffect::shadow(Vec2::new(1.0, 1.0), Color::BLACK);
        effect.set_enabled(false);
        effects.add(effect);

        let text = Text::new("Hello").with_effects(effects);

        assert!(!text.has_effects());
    }

    #[test]
    fn test_text_builder_chaining() {
        let text = Text::new("Hello World")
            .size(24.0)
            .color(Color::RED)
            .bold()
            .with_shadow(Vec2::new(2.0, 2.0), Color::BLACK)
            .with_outline(1.0, Color::WHITE)
            .sdf();

        assert_eq!(text.get_font_size(), 24.0);
        assert_eq!(text.get_color(), Color::RED);
        assert!(text.has_effects());
        assert!(text.get_render_mode().unwrap().is_sdf());
    }

    #[test]
    fn test_text_effective_render_mode_boundary() {
        // At 24px boundary
        let text_at_boundary = Text::new("Hello").size(24.0);
        assert!(text_at_boundary.effective_render_mode().is_sdf());

        // Just below boundary
        let text_below = Text::new("Hello").size(23.9);
        assert!(!text_below.effective_render_mode().is_sdf());

        // Just above boundary
        let text_above = Text::new("Hello").size(24.1);
        assert!(text_above.effective_render_mode().is_sdf());
    }
}
