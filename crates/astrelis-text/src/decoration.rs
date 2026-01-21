//! Text decoration - underline, strikethrough, and background highlighting.
//!
//! This module provides text decoration capabilities for rich text rendering:
//! - Underlines (solid, dashed, dotted, wavy)
//! - Strikethrough
//! - Background highlighting
//!
//! # Example
//!
//! ```ignore
//! use astrelis_text::*;
//!
//! let decoration = TextDecoration::new()
//!     .underline(UnderlineStyle::solid(Color::BLUE, 1.0))
//!     .background(Color::YELLOW);
//!
//! let text = Text::new("Important text")
//!     .decoration(decoration);
//! ```

use astrelis_render::Color;

/// Line style for underlines and strikethrough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    /// Solid line
    Solid,
    /// Dashed line
    Dashed,
    /// Dotted line
    Dotted,
    /// Wavy line (sine wave)
    Wavy,
}

impl Default for LineStyle {
    fn default() -> Self {
        Self::Solid
    }
}

/// Underline style configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnderlineStyle {
    /// Line color
    pub color: Color,
    /// Line thickness in pixels
    pub thickness: f32,
    /// Line style (solid, dashed, dotted, wavy)
    pub style: LineStyle,
    /// Offset below baseline in pixels (positive = below)
    pub offset: f32,
}

impl UnderlineStyle {
    /// Create a solid underline.
    pub fn solid(color: Color, thickness: f32) -> Self {
        Self {
            color,
            thickness,
            style: LineStyle::Solid,
            offset: 2.0,
        }
    }

    /// Create a dashed underline.
    pub fn dashed(color: Color, thickness: f32) -> Self {
        Self {
            color,
            thickness,
            style: LineStyle::Dashed,
            offset: 2.0,
        }
    }

    /// Create a dotted underline.
    pub fn dotted(color: Color, thickness: f32) -> Self {
        Self {
            color,
            thickness,
            style: LineStyle::Dotted,
            offset: 2.0,
        }
    }

    /// Create a wavy underline.
    pub fn wavy(color: Color, thickness: f32) -> Self {
        Self {
            color,
            thickness,
            style: LineStyle::Wavy,
            offset: 2.0,
        }
    }

    /// Set the offset below baseline.
    pub fn with_offset(mut self, offset: f32) -> Self {
        self.offset = offset;
        self
    }
}

/// Strikethrough style configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrikethroughStyle {
    /// Line color
    pub color: Color,
    /// Line thickness in pixels
    pub thickness: f32,
    /// Line style (solid, dashed, dotted)
    pub style: LineStyle,
    /// Offset from baseline in pixels (0 = centered on text)
    pub offset: f32,
}

impl StrikethroughStyle {
    /// Create a solid strikethrough.
    pub fn solid(color: Color, thickness: f32) -> Self {
        Self {
            color,
            thickness,
            style: LineStyle::Solid,
            offset: 0.0,
        }
    }

    /// Create a dashed strikethrough.
    pub fn dashed(color: Color, thickness: f32) -> Self {
        Self {
            color,
            thickness,
            style: LineStyle::Dashed,
            offset: 0.0,
        }
    }

    /// Create a dotted strikethrough.
    pub fn dotted(color: Color, thickness: f32) -> Self {
        Self {
            color,
            thickness,
            style: LineStyle::Dotted,
            offset: 0.0,
        }
    }

    /// Set the offset from baseline.
    pub fn with_offset(mut self, offset: f32) -> Self {
        self.offset = offset;
        self
    }
}

/// Text decoration configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct TextDecoration {
    /// Underline style
    pub underline: Option<UnderlineStyle>,
    /// Strikethrough style
    pub strikethrough: Option<StrikethroughStyle>,
    /// Background highlight color
    pub background: Option<Color>,
    /// Background padding (left, top, right, bottom)
    pub background_padding: [f32; 4],
}

impl Default for TextDecoration {
    fn default() -> Self {
        Self {
            underline: None,
            strikethrough: None,
            background: None,
            background_padding: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

impl TextDecoration {
    /// Create a new empty decoration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set underline style.
    pub fn underline(mut self, style: UnderlineStyle) -> Self {
        self.underline = Some(style);
        self
    }

    /// Set strikethrough style.
    pub fn strikethrough(mut self, style: StrikethroughStyle) -> Self {
        self.strikethrough = Some(style);
        self
    }

    /// Set background highlight color.
    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// Set background padding (uniform).
    pub fn background_padding_uniform(mut self, padding: f32) -> Self {
        self.background_padding = [padding; 4];
        self
    }

    /// Set background padding (left, top, right, bottom).
    pub fn background_padding_ltrb(mut self, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        self.background_padding = [left, top, right, bottom];
        self
    }

    /// Check if any decoration is set.
    pub fn has_decoration(&self) -> bool {
        self.underline.is_some() || self.strikethrough.is_some() || self.background.is_some()
    }

    /// Check if underline is set.
    pub fn has_underline(&self) -> bool {
        self.underline.is_some()
    }

    /// Check if strikethrough is set.
    pub fn has_strikethrough(&self) -> bool {
        self.strikethrough.is_some()
    }

    /// Check if background is set.
    pub fn has_background(&self) -> bool {
        self.background.is_some()
    }
}

/// Geometry for rendering decorations.
///
/// This is typically generated per line or per text span.
#[derive(Debug, Clone, PartialEq)]
pub struct DecorationGeometry {
    /// Line start position (x, y)
    pub start: (f32, f32),
    /// Line end position (x, y)
    pub end: (f32, f32),
    /// Line thickness
    pub thickness: f32,
    /// Line color
    pub color: Color,
    /// Line style
    pub style: LineStyle,
}

impl DecorationGeometry {
    /// Create a new decoration geometry.
    pub fn new(start: (f32, f32), end: (f32, f32), thickness: f32, color: Color, style: LineStyle) -> Self {
        Self {
            start,
            end,
            thickness,
            color,
            style,
        }
    }

    /// Get the line length.
    pub fn length(&self) -> f32 {
        let dx = self.end.0 - self.start.0;
        let dy = self.end.1 - self.start.1;
        (dx * dx + dy * dy).sqrt()
    }

    /// Get the center point.
    pub fn center(&self) -> (f32, f32) {
        ((self.start.0 + self.end.0) / 2.0, (self.start.1 + self.end.1) / 2.0)
    }
}

/// Background highlight geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct BackgroundGeometry {
    /// Rectangle bounds (x, y, width, height)
    pub rect: (f32, f32, f32, f32),
    /// Background color
    pub color: Color,
}

impl BackgroundGeometry {
    /// Create a new background geometry.
    pub fn new(x: f32, y: f32, width: f32, height: f32, color: Color) -> Self {
        Self {
            rect: (x, y, width, height),
            color,
        }
    }

    /// Get the rectangle as (x, y, width, height).
    pub fn as_rect(&self) -> (f32, f32, f32, f32) {
        self.rect
    }
}

/// Generate decoration geometry for a line of text.
///
/// # Arguments
///
/// * `decoration` - The decoration configuration
/// * `baseline_y` - Y coordinate of the text baseline
/// * `line_start_x` - X coordinate where the line starts
/// * `line_end_x` - X coordinate where the line ends
/// * `line_height` - Height of the line
///
/// # Returns
///
/// Tuple of (background, underlines, strikethroughs)
pub fn generate_decoration_geometry(
    decoration: &TextDecoration,
    baseline_y: f32,
    line_start_x: f32,
    line_end_x: f32,
    line_height: f32,
) -> (
    Option<BackgroundGeometry>,
    Option<DecorationGeometry>,
    Option<DecorationGeometry>,
) {
    let mut background = None;
    let mut underline = None;
    let mut strikethrough = None;

    // Background
    if let Some(bg_color) = decoration.background {
        let padding = &decoration.background_padding;
        let x = line_start_x - padding[0];
        let y = baseline_y - line_height + padding[1];
        let width = (line_end_x - line_start_x) + padding[0] + padding[2];
        let height = line_height + padding[1] + padding[3];

        background = Some(BackgroundGeometry::new(x, y, width, height, bg_color));
    }

    // Underline
    if let Some(ul_style) = decoration.underline {
        let y = baseline_y + ul_style.offset;
        underline = Some(DecorationGeometry::new(
            (line_start_x, y),
            (line_end_x, y),
            ul_style.thickness,
            ul_style.color,
            ul_style.style,
        ));
    }

    // Strikethrough
    if let Some(st_style) = decoration.strikethrough {
        let y = baseline_y - (line_height / 2.0) + st_style.offset;
        strikethrough = Some(DecorationGeometry::new(
            (line_start_x, y),
            (line_end_x, y),
            st_style.thickness,
            st_style.color,
            st_style.style,
        ));
    }

    (background, underline, strikethrough)
}

/// Type of decoration quad for rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecorationQuadType {
    /// Background highlight quad.
    Background,
    /// Underline quad.
    Underline {
        /// Line thickness in pixels.
        thickness: f32,
    },
    /// Strikethrough quad.
    Strikethrough {
        /// Line thickness in pixels.
        thickness: f32,
    },
}

/// A quad for rendering text decorations.
///
/// This is the unified output format for all decoration types.
/// The renderer generates these quads and submits them for rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct DecorationQuad {
    /// Quad bounds (x, y, width, height) in logical pixels.
    pub bounds: (f32, f32, f32, f32),
    /// Quad color.
    pub color: Color,
    /// Type of decoration this quad represents.
    pub quad_type: DecorationQuadType,
}

impl DecorationQuad {
    /// Create a new decoration quad.
    pub fn new(x: f32, y: f32, width: f32, height: f32, color: Color, quad_type: DecorationQuadType) -> Self {
        Self {
            bounds: (x, y, width, height),
            color,
            quad_type,
        }
    }

    /// Create a background quad.
    pub fn background(x: f32, y: f32, width: f32, height: f32, color: Color) -> Self {
        Self::new(x, y, width, height, color, DecorationQuadType::Background)
    }

    /// Create an underline quad.
    pub fn underline(x: f32, y: f32, width: f32, thickness: f32, color: Color) -> Self {
        Self::new(x, y, width, thickness, color, DecorationQuadType::Underline { thickness })
    }

    /// Create a strikethrough quad.
    pub fn strikethrough(x: f32, y: f32, width: f32, thickness: f32, color: Color) -> Self {
        Self::new(x, y, width, thickness, color, DecorationQuadType::Strikethrough { thickness })
    }

    /// Get the bounds as (x, y, width, height).
    pub fn as_rect(&self) -> (f32, f32, f32, f32) {
        self.bounds
    }

    /// Check if this is a background quad.
    pub fn is_background(&self) -> bool {
        matches!(self.quad_type, DecorationQuadType::Background)
    }

    /// Check if this is an underline quad.
    pub fn is_underline(&self) -> bool {
        matches!(self.quad_type, DecorationQuadType::Underline { .. })
    }

    /// Check if this is a strikethrough quad.
    pub fn is_strikethrough(&self) -> bool {
        matches!(self.quad_type, DecorationQuadType::Strikethrough { .. })
    }
}

/// Text bounds information needed for decoration geometry generation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextBounds {
    /// X position of text (left edge).
    pub x: f32,
    /// Y position of text (top edge).
    pub y: f32,
    /// Width of text.
    pub width: f32,
    /// Height of text (line height).
    pub height: f32,
    /// Baseline Y offset from top (ascent).
    pub baseline_offset: f32,
}

impl TextBounds {
    /// Create new text bounds.
    pub fn new(x: f32, y: f32, width: f32, height: f32, baseline_offset: f32) -> Self {
        Self { x, y, width, height, baseline_offset }
    }
}

/// Generate line quads for a given line style.
///
/// This helper function generates the appropriate quads for different line styles:
/// - Solid: Single rectangular quad
/// - Dashed: Multiple rectangular quads with gaps
/// - Dotted: Multiple small square quads
/// - Wavy: Multiple rectangular quads forming a sine wave pattern
///
/// # Arguments
///
/// * `quads` - Output vector to append generated quads to
/// * `x` - Starting X position of the line
/// * `y` - Y position (center) of the line
/// * `width` - Total width of the line
/// * `thickness` - Line thickness in pixels
/// * `color` - Line color
/// * `style` - Line style (Solid, Dashed, Dotted, Wavy)
/// * `quad_type` - Type of decoration quad (Underline or Strikethrough)
fn generate_line_quads(
    quads: &mut Vec<DecorationQuad>,
    x: f32,
    y: f32,
    width: f32,
    thickness: f32,
    color: Color,
    style: LineStyle,
    quad_type: DecorationQuadType,
) {
    match style {
        LineStyle::Solid => {
            // Single solid quad
            quads.push(DecorationQuad::new(x, y, width, thickness, color, quad_type));
        }
        LineStyle::Dashed => {
            // Dashed line: dash_length = 4 * thickness, gap_length = 2 * thickness
            let dash_length = (4.0 * thickness).max(3.0);
            let gap_length = (2.0 * thickness).max(2.0);
            let segment_length = dash_length + gap_length;

            let mut current_x = x;
            while current_x < x + width {
                let remaining = (x + width) - current_x;
                let dash_width = dash_length.min(remaining);

                if dash_width > 0.5 {
                    quads.push(DecorationQuad::new(
                        current_x,
                        y,
                        dash_width,
                        thickness,
                        color,
                        quad_type,
                    ));
                }

                current_x += segment_length;
            }
        }
        LineStyle::Dotted => {
            // Dotted line: dots are squares with size = thickness, spaced by 2 * thickness
            let dot_size = thickness;
            let dot_spacing = (2.0 * thickness).max(2.0);
            let segment_length = dot_size + dot_spacing;

            let mut current_x = x;
            while current_x < x + width {
                let remaining = (x + width) - current_x;
                let dot_width = dot_size.min(remaining);

                if dot_width > 0.5 {
                    quads.push(DecorationQuad::new(
                        current_x,
                        y,
                        dot_width,
                        thickness,
                        color,
                        quad_type,
                    ));
                }

                current_x += segment_length;
            }
        }
        LineStyle::Wavy => {
            // Wavy line: sine wave pattern
            // Wave parameters
            let wave_height = (thickness * 1.5).max(2.0); // Amplitude of the wave
            let wave_length = (thickness * 8.0).max(8.0); // One complete wave cycle
            let segment_width = wave_length / 8.0; // Divide wave into segments for smooth curve

            let mut current_x = x;
            let mut segment_index = 0;

            while current_x < x + width {
                let remaining = (x + width) - current_x;
                let seg_width = segment_width.min(remaining);

                if seg_width > 0.5 {
                    // Calculate Y offset based on sine wave
                    let phase = segment_index as f32 * segment_width / wave_length * 2.0 * std::f32::consts::PI;
                    let y_offset = phase.sin() * wave_height * 0.5;

                    quads.push(DecorationQuad::new(
                        current_x,
                        y + y_offset,
                        seg_width,
                        thickness,
                        color,
                        quad_type,
                    ));
                }

                current_x += segment_width;
                segment_index += 1;
            }
        }
    }
}

/// Generate decoration quads from text bounds and decoration configuration.
///
/// This function generates all the quads needed to render decorations for a piece of text.
/// It returns a Vec of DecorationQuad that can be rendered using the decoration pipeline.
///
/// The order of quads in the returned Vec is:
/// 1. Background quads (rendered first, behind text)
/// 2. Underline quads (rendered after text)
/// 3. Strikethrough quads (rendered after text)
///
/// Supports all line styles:
/// - **Solid**: Continuous line
/// - **Dashed**: Alternating dashes and gaps
/// - **Dotted**: Series of dots
/// - **Wavy**: Sine wave pattern
///
/// # Arguments
///
/// * `bounds` - The text bounds (position, size, baseline)
/// * `decoration` - The decoration configuration
///
/// # Returns
///
/// A Vec of DecorationQuad to render
///
/// # Example
///
/// ```ignore
/// use astrelis_text::{TextDecoration, UnderlineStyle, TextBounds, generate_decoration_quads, LineStyle};
///
/// let bounds = TextBounds::new(10.0, 20.0, 100.0, 24.0, 18.0);
///
/// // Solid underline
/// let decoration = TextDecoration::new()
///     .underline(UnderlineStyle::solid(Color::BLUE, 1.0));
///
/// // Wavy underline
/// let decoration = TextDecoration::new()
///     .underline(UnderlineStyle::wavy(Color::RED, 1.5));
///
/// let quads = generate_decoration_quads(&bounds, &decoration);
/// ```
pub fn generate_decoration_quads(bounds: &TextBounds, decoration: &TextDecoration) -> Vec<DecorationQuad> {
    let mut quads = Vec::new();

    // Background (rendered first, behind text)
    if let Some(bg_color) = decoration.background {
        let padding = &decoration.background_padding;
        let x = bounds.x - padding[0];
        let y = bounds.y - padding[1];
        let width = bounds.width + padding[0] + padding[2];
        let height = bounds.height + padding[1] + padding[3];

        quads.push(DecorationQuad::background(x, y, width, height, bg_color));
    }

    // Underline (rendered after text)
    if let Some(ul_style) = decoration.underline {
        let baseline_y = bounds.y + bounds.baseline_offset;
        let y = baseline_y + ul_style.offset;
        let x = bounds.x;
        let width = bounds.width;
        let thickness = ul_style.thickness;

        generate_line_quads(
            &mut quads,
            x,
            y,
            width,
            thickness,
            ul_style.color,
            ul_style.style,
            DecorationQuadType::Underline { thickness },
        );
    }

    // Strikethrough (rendered after text)
    if let Some(st_style) = decoration.strikethrough {
        // Strikethrough at ~40% of line height from baseline (approximately middle of x-height)
        let baseline_y = bounds.y + bounds.baseline_offset;
        let y = baseline_y - (bounds.height * 0.35) + st_style.offset;
        let x = bounds.x;
        let width = bounds.width;
        let thickness = st_style.thickness;

        generate_line_quads(
            &mut quads,
            x,
            y,
            width,
            thickness,
            st_style.color,
            st_style.style,
            DecorationQuadType::Strikethrough { thickness },
        );
    }

    quads
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_style_default() {
        assert_eq!(LineStyle::default(), LineStyle::Solid);
    }

    #[test]
    fn test_underline_style_solid() {
        let style = UnderlineStyle::solid(Color::RED, 1.0);
        assert_eq!(style.color, Color::RED);
        assert_eq!(style.thickness, 1.0);
        assert_eq!(style.style, LineStyle::Solid);
        assert_eq!(style.offset, 2.0);
    }

    #[test]
    fn test_underline_style_wavy() {
        let style = UnderlineStyle::wavy(Color::BLUE, 2.0).with_offset(3.0);
        assert_eq!(style.color, Color::BLUE);
        assert_eq!(style.thickness, 2.0);
        assert_eq!(style.style, LineStyle::Wavy);
        assert_eq!(style.offset, 3.0);
    }

    #[test]
    fn test_strikethrough_style_solid() {
        let style = StrikethroughStyle::solid(Color::BLACK, 1.5);
        assert_eq!(style.color, Color::BLACK);
        assert_eq!(style.thickness, 1.5);
        assert_eq!(style.style, LineStyle::Solid);
        assert_eq!(style.offset, 0.0);
    }

    #[test]
    fn test_text_decoration_default() {
        let decoration = TextDecoration::default();
        assert!(!decoration.has_decoration());
        assert!(!decoration.has_underline());
        assert!(!decoration.has_strikethrough());
        assert!(!decoration.has_background());
    }

    #[test]
    fn test_text_decoration_builder() {
        let decoration = TextDecoration::new()
            .underline(UnderlineStyle::solid(Color::RED, 1.0))
            .strikethrough(StrikethroughStyle::solid(Color::BLACK, 1.0))
            .background(Color::YELLOW);

        assert!(decoration.has_decoration());
        assert!(decoration.has_underline());
        assert!(decoration.has_strikethrough());
        assert!(decoration.has_background());
    }

    #[test]
    fn test_decoration_geometry() {
        let geom = DecorationGeometry::new((0.0, 0.0), (100.0, 0.0), 1.0, Color::RED, LineStyle::Solid);
        assert_eq!(geom.length(), 100.0);
        assert_eq!(geom.center(), (50.0, 0.0));
    }

    #[test]
    fn test_background_geometry() {
        let geom = BackgroundGeometry::new(10.0, 20.0, 100.0, 50.0, Color::YELLOW);
        assert_eq!(geom.as_rect(), (10.0, 20.0, 100.0, 50.0));
        assert_eq!(geom.color, Color::YELLOW);
    }

    #[test]
    fn test_generate_decoration_geometry() {
        let decoration = TextDecoration::new()
            .underline(UnderlineStyle::solid(Color::RED, 1.0))
            .strikethrough(StrikethroughStyle::solid(Color::BLACK, 1.0))
            .background(Color::YELLOW);

        let (bg, ul, st) = generate_decoration_geometry(&decoration, 100.0, 0.0, 200.0, 20.0);

        assert!(bg.is_some());
        assert!(ul.is_some());
        assert!(st.is_some());

        let bg = bg.unwrap();
        assert_eq!(bg.color, Color::YELLOW);

        let ul = ul.unwrap();
        assert_eq!(ul.color, Color::RED);
        assert_eq!(ul.start.0, 0.0);
        assert_eq!(ul.end.0, 200.0);

        let st = st.unwrap();
        assert_eq!(st.color, Color::BLACK);
        assert_eq!(st.start.0, 0.0);
        assert_eq!(st.end.0, 200.0);
    }

    #[test]
    fn test_background_padding() {
        let decoration = TextDecoration::new()
            .background(Color::YELLOW)
            .background_padding_ltrb(5.0, 3.0, 5.0, 3.0);

        let (bg, _, _) = generate_decoration_geometry(&decoration, 100.0, 0.0, 200.0, 20.0);

        let bg = bg.unwrap();
        let (x, _y, width, height) = bg.as_rect();

        // Check padding is applied
        assert_eq!(x, -5.0); // left padding
        assert_eq!(width, 210.0); // original 200 + left 5 + right 5
        assert_eq!(height, 26.0); // original 20 + top 3 + bottom 3
    }

    #[test]
    fn test_solid_line_style() {
        let bounds = TextBounds::new(0.0, 0.0, 100.0, 20.0, 15.0);
        let decoration = TextDecoration::new()
            .underline(UnderlineStyle::solid(Color::RED, 1.0));

        let quads = generate_decoration_quads(&bounds, &decoration);

        // Solid line should generate exactly 1 quad
        assert_eq!(quads.len(), 1);
        assert!(quads[0].is_underline());
        assert_eq!(quads[0].color, Color::RED);
    }

    #[test]
    fn test_dashed_line_style() {
        let bounds = TextBounds::new(0.0, 0.0, 100.0, 20.0, 15.0);
        let decoration = TextDecoration::new()
            .underline(UnderlineStyle::dashed(Color::BLUE, 2.0));

        let quads = generate_decoration_quads(&bounds, &decoration);

        // Dashed line should generate multiple quads (dashes with gaps)
        assert!(quads.len() > 1, "Dashed line should generate multiple quads");
        assert!(quads[0].is_underline());
        assert_eq!(quads[0].color, Color::BLUE);
    }

    #[test]
    fn test_dotted_line_style() {
        let bounds = TextBounds::new(0.0, 0.0, 100.0, 20.0, 15.0);
        let decoration = TextDecoration::new()
            .underline(UnderlineStyle::dotted(Color::GREEN, 1.5));

        let quads = generate_decoration_quads(&bounds, &decoration);

        // Dotted line should generate multiple quads (dots with gaps)
        assert!(quads.len() > 1, "Dotted line should generate multiple quads");
        assert!(quads[0].is_underline());
        assert_eq!(quads[0].color, Color::GREEN);
    }

    #[test]
    fn test_wavy_line_style() {
        let bounds = TextBounds::new(0.0, 0.0, 100.0, 20.0, 15.0);
        let decoration = TextDecoration::new()
            .underline(UnderlineStyle::wavy(Color::YELLOW, 1.0));

        let quads = generate_decoration_quads(&bounds, &decoration);

        // Wavy line should generate multiple quads forming a wave
        assert!(quads.len() > 1, "Wavy line should generate multiple quads");
        assert!(quads[0].is_underline());
        assert_eq!(quads[0].color, Color::YELLOW);

        // Verify that y positions vary (wave effect)
        if quads.len() >= 2 {
            let y_positions: Vec<f32> = quads.iter().map(|q| q.bounds.1).collect();
            let all_same = y_positions.windows(2).all(|w| w[0] == w[1]);
            assert!(!all_same, "Wavy line should have varying y positions");
        }
    }

    #[test]
    fn test_strikethrough_line_styles() {
        let bounds = TextBounds::new(0.0, 0.0, 100.0, 20.0, 15.0);

        // Test solid strikethrough
        let decoration = TextDecoration::new()
            .strikethrough(StrikethroughStyle::solid(Color::BLACK, 1.0));
        let quads = generate_decoration_quads(&bounds, &decoration);
        assert_eq!(quads.len(), 1);
        assert!(quads[0].is_strikethrough());

        // Test dashed strikethrough
        let decoration = TextDecoration::new()
            .strikethrough(StrikethroughStyle::dashed(Color::BLACK, 1.0));
        let quads = generate_decoration_quads(&bounds, &decoration);
        assert!(quads.len() > 1);
        assert!(quads[0].is_strikethrough());
    }

    #[test]
    fn test_combined_decorations_with_line_styles() {
        let bounds = TextBounds::new(0.0, 0.0, 100.0, 20.0, 15.0);
        let decoration = TextDecoration::new()
            .background(Color::YELLOW)
            .underline(UnderlineStyle::wavy(Color::RED, 1.0))
            .strikethrough(StrikethroughStyle::dashed(Color::BLACK, 1.0));

        let quads = generate_decoration_quads(&bounds, &decoration);

        // Should have: 1 background + multiple underline + multiple strikethrough
        assert!(quads.len() > 3, "Combined decorations should generate multiple quads");

        // First quad should be background
        assert!(quads[0].is_background());
        assert_eq!(quads[0].color, Color::YELLOW);
    }
}
