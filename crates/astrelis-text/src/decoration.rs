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
}
