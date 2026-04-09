//! Text decoration - underline, strikethrough, and background highlighting.
//!
//! Supports solid, dashed, dotted, and wavy line styles for underlines
//! and strikethrough, plus background highlighting with configurable padding.

use astrelis_core::color::Color;

/// Line style for underlines and strikethrough.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineStyle {
    /// Solid line.
    #[default]
    Solid,
    /// Dashed line.
    Dashed,
    /// Dotted line.
    Dotted,
    /// Wavy line (sine wave).
    Wavy,
}

/// Underline style configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnderlineStyle {
    /// Line color.
    pub color: Color,
    /// Line thickness in pixels.
    pub thickness: f32,
    /// Line style.
    pub style: LineStyle,
    /// Offset below baseline in pixels (positive = below).
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
    /// Line color.
    pub color: Color,
    /// Line thickness in pixels.
    pub thickness: f32,
    /// Line style.
    pub style: LineStyle,
    /// Offset from baseline in pixels.
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
    /// Underline style.
    pub underline: Option<UnderlineStyle>,
    /// Strikethrough style.
    pub strikethrough: Option<StrikethroughStyle>,
    /// Background highlight color.
    pub background: Option<Color>,
    /// Background padding `[left, top, right, bottom]`.
    pub background_padding: [f32; 4],
}

impl Default for TextDecoration {
    fn default() -> Self {
        Self {
            underline: None,
            strikethrough: None,
            background: None,
            background_padding: [0.0; 4],
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

    /// Set uniform background padding.
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
#[derive(Debug, Clone, PartialEq)]
pub struct DecorationQuad {
    /// Quad bounds `(x, y, width, height)` in logical pixels.
    pub bounds: (f32, f32, f32, f32),
    /// Quad color.
    pub color: Color,
    /// Type of decoration.
    pub quad_type: DecorationQuadType,
}

impl DecorationQuad {
    /// Create a new decoration quad.
    pub fn new(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
        quad_type: DecorationQuadType,
    ) -> Self {
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
        Self::new(
            x,
            y,
            width,
            thickness,
            color,
            DecorationQuadType::Underline { thickness },
        )
    }

    /// Create a strikethrough quad.
    pub fn strikethrough(x: f32, y: f32, width: f32, thickness: f32, color: Color) -> Self {
        Self::new(
            x,
            y,
            width,
            thickness,
            color,
            DecorationQuadType::Strikethrough { thickness },
        )
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

/// Text bounds information for decoration geometry generation.
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
        Self {
            x,
            y,
            width,
            height,
            baseline_offset,
        }
    }
}

/// Parameters for generating line decoration quads.
struct LineQuadParams {
    x: f32,
    y: f32,
    width: f32,
    thickness: f32,
    color: Color,
    style: LineStyle,
    quad_type: DecorationQuadType,
}

/// Generate line quads for a given line style.
fn generate_line_quads(quads: &mut Vec<DecorationQuad>, params: &LineQuadParams) {
    let LineQuadParams {
        x,
        y,
        width,
        thickness,
        color,
        style,
        quad_type,
    } = *params;

    match style {
        LineStyle::Solid => {
            quads.push(DecorationQuad::new(x, y, width, thickness, color, quad_type));
        }
        LineStyle::Dashed => {
            let dash_length = (4.0 * thickness).max(3.0);
            let gap_length = (2.0 * thickness).max(2.0);
            let segment_length = dash_length + gap_length;

            let mut current_x = x;
            while current_x < x + width {
                let remaining = (x + width) - current_x;
                let dash_width = dash_length.min(remaining);
                if dash_width > 0.5 {
                    quads.push(DecorationQuad::new(
                        current_x, y, dash_width, thickness, color, quad_type,
                    ));
                }
                current_x += segment_length;
            }
        }
        LineStyle::Dotted => {
            let dot_size = thickness;
            let dot_spacing = (2.0 * thickness).max(2.0);
            let segment_length = dot_size + dot_spacing;

            let mut current_x = x;
            while current_x < x + width {
                let remaining = (x + width) - current_x;
                let dot_width = dot_size.min(remaining);
                if dot_width > 0.5 {
                    quads.push(DecorationQuad::new(
                        current_x, y, dot_width, thickness, color, quad_type,
                    ));
                }
                current_x += segment_length;
            }
        }
        LineStyle::Wavy => {
            let wave_height = (thickness * 1.5).max(2.0);
            let wave_length = (thickness * 8.0).max(8.0);
            let segment_width = wave_length / 8.0;

            let mut current_x = x;
            let mut segment_index = 0;

            while current_x < x + width {
                let remaining = (x + width) - current_x;
                let seg_width = segment_width.min(remaining);
                if seg_width > 0.5 {
                    let phase = segment_index as f32 * segment_width / wave_length
                        * 2.0
                        * std::f32::consts::PI;
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
/// Returns quads in render order: backgrounds first, then underlines, then strikethroughs.
pub fn generate_decoration_quads(
    bounds: &TextBounds,
    decoration: &TextDecoration,
) -> Vec<DecorationQuad> {
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

    // Underline
    if let Some(ul_style) = decoration.underline {
        let baseline_y = bounds.y + bounds.baseline_offset;
        let y = baseline_y + ul_style.offset;
        generate_line_quads(
            &mut quads,
            &LineQuadParams {
                x: bounds.x,
                y,
                width: bounds.width,
                thickness: ul_style.thickness,
                color: ul_style.color,
                style: ul_style.style,
                quad_type: DecorationQuadType::Underline {
                    thickness: ul_style.thickness,
                },
            },
        );
    }

    // Strikethrough
    if let Some(st_style) = decoration.strikethrough {
        let baseline_y = bounds.y + bounds.baseline_offset;
        let y = baseline_y - (bounds.height * 0.35) + st_style.offset;
        generate_line_quads(
            &mut quads,
            &LineQuadParams {
                x: bounds.x,
                y,
                width: bounds.width,
                thickness: st_style.thickness,
                color: st_style.color,
                style: st_style.style,
                quad_type: DecorationQuadType::Strikethrough {
                    thickness: st_style.thickness,
                },
            },
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
    fn test_underline_style_wavy_with_offset() {
        let style = UnderlineStyle::wavy(Color::BLUE, 2.0).with_offset(3.0);
        assert_eq!(style.style, LineStyle::Wavy);
        assert_eq!(style.offset, 3.0);
    }

    #[test]
    fn test_strikethrough_style_solid() {
        let style = StrikethroughStyle::solid(Color::BLACK, 1.5);
        assert_eq!(style.color, Color::BLACK);
        assert_eq!(style.thickness, 1.5);
        assert_eq!(style.offset, 0.0);
    }

    #[test]
    fn test_text_decoration_default() {
        let decoration = TextDecoration::default();
        assert!(!decoration.has_decoration());
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
    fn test_solid_underline_quads() {
        let bounds = TextBounds::new(0.0, 0.0, 100.0, 20.0, 15.0);
        let decoration = TextDecoration::new().underline(UnderlineStyle::solid(Color::RED, 1.0));
        let quads = generate_decoration_quads(&bounds, &decoration);
        assert_eq!(quads.len(), 1);
        assert!(quads[0].is_underline());
    }

    #[test]
    fn test_dashed_underline_quads() {
        let bounds = TextBounds::new(0.0, 0.0, 100.0, 20.0, 15.0);
        let decoration = TextDecoration::new().underline(UnderlineStyle::dashed(Color::BLUE, 2.0));
        let quads = generate_decoration_quads(&bounds, &decoration);
        assert!(quads.len() > 1);
    }

    #[test]
    fn test_wavy_underline_quads() {
        let bounds = TextBounds::new(0.0, 0.0, 100.0, 20.0, 15.0);
        let decoration = TextDecoration::new().underline(UnderlineStyle::wavy(Color::YELLOW, 1.0));
        let quads = generate_decoration_quads(&bounds, &decoration);
        assert!(quads.len() > 1);
        // Verify y positions vary (wave)
        let y_positions: Vec<f32> = quads.iter().map(|q| q.bounds.1).collect();
        let all_same = y_positions.windows(2).all(|w| w[0] == w[1]);
        assert!(!all_same);
    }

    #[test]
    fn test_background_with_padding() {
        let bounds = TextBounds::new(10.0, 20.0, 100.0, 20.0, 15.0);
        let decoration = TextDecoration::new()
            .background(Color::YELLOW)
            .background_padding_ltrb(5.0, 3.0, 5.0, 3.0);
        let quads = generate_decoration_quads(&bounds, &decoration);
        assert_eq!(quads.len(), 1);
        let (x, _y, width, height) = quads[0].bounds;
        assert_eq!(x, 5.0);
        assert_eq!(width, 110.0);
        assert_eq!(height, 26.0);
    }

    #[test]
    fn test_combined_decorations() {
        let bounds = TextBounds::new(0.0, 0.0, 100.0, 20.0, 15.0);
        let decoration = TextDecoration::new()
            .background(Color::YELLOW)
            .underline(UnderlineStyle::wavy(Color::RED, 1.0))
            .strikethrough(StrikethroughStyle::dashed(Color::BLACK, 1.0));
        let quads = generate_decoration_quads(&bounds, &decoration);
        assert!(quads.len() > 3);
        assert!(quads[0].is_background());
    }
}
