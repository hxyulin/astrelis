//! Combined style for geometry rendering.
//!
//! A style combines fill and stroke properties for complete geometry styling.

use crate::{Fill, Paint, Stroke, Transform2D};
use astrelis_render::Color;

/// Complete style for geometry rendering.
///
/// A style combines optional fill and stroke properties, along with a transform.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Optional fill properties
    pub fill: Option<Fill>,
    /// Optional stroke properties
    pub stroke: Option<Stroke>,
    /// Transform to apply to the geometry
    pub transform: Transform2D,
}

impl Style {
    /// Create a new empty style (invisible).
    pub fn new() -> Self {
        Self {
            fill: None,
            stroke: None,
            transform: Transform2D::IDENTITY,
        }
    }

    /// Create a fill-only style.
    pub fn fill(paint: impl Into<Paint>) -> Self {
        Self {
            fill: Some(Fill::from_paint(paint.into())),
            stroke: None,
            transform: Transform2D::IDENTITY,
        }
    }

    /// Create a fill-only style with a solid color.
    pub fn fill_color(color: Color) -> Self {
        Self::fill(Paint::solid(color))
    }

    /// Create a stroke-only style.
    pub fn stroke(paint: impl Into<Paint>, width: f32) -> Self {
        Self {
            fill: None,
            stroke: Some(Stroke::from_paint(paint.into(), width)),
            transform: Transform2D::IDENTITY,
        }
    }

    /// Create a stroke-only style with a solid color.
    pub fn stroke_color(color: Color, width: f32) -> Self {
        Self::stroke(Paint::solid(color), width)
    }

    /// Create a style with both fill and stroke.
    pub fn fill_and_stroke(
        fill_paint: impl Into<Paint>,
        stroke_paint: impl Into<Paint>,
        stroke_width: f32,
    ) -> Self {
        Self {
            fill: Some(Fill::from_paint(fill_paint.into())),
            stroke: Some(Stroke::from_paint(stroke_paint.into(), stroke_width)),
            transform: Transform2D::IDENTITY,
        }
    }

    /// Set the fill.
    pub fn with_fill(mut self, fill: Fill) -> Self {
        self.fill = Some(fill);
        self
    }

    /// Set the fill paint.
    pub fn with_fill_paint(mut self, paint: impl Into<Paint>) -> Self {
        self.fill = Some(Fill::from_paint(paint.into()));
        self
    }

    /// Set the fill color.
    pub fn with_fill_color(mut self, color: Color) -> Self {
        self.fill = Some(Fill::solid(color));
        self
    }

    /// Set the stroke.
    pub fn with_stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    /// Set the stroke paint and width.
    pub fn with_stroke_paint(mut self, paint: impl Into<Paint>, width: f32) -> Self {
        self.stroke = Some(Stroke::from_paint(paint.into(), width));
        self
    }

    /// Set the stroke color and width.
    pub fn with_stroke_color(mut self, color: Color, width: f32) -> Self {
        self.stroke = Some(Stroke::solid(color, width));
        self
    }

    /// Set the transform.
    pub fn with_transform(mut self, transform: Transform2D) -> Self {
        self.transform = transform;
        self
    }

    /// Check if this style has a visible fill.
    pub fn has_fill(&self) -> bool {
        self.fill.as_ref().is_some_and(|f| f.opacity > 0.0)
    }

    /// Check if this style has a visible stroke.
    pub fn has_stroke(&self) -> bool {
        self.stroke.as_ref().is_some_and(|s| s.is_visible())
    }

    /// Check if this style is visible (has fill or stroke).
    pub fn is_visible(&self) -> bool {
        self.has_fill() || self.has_stroke()
    }

    /// Get the fill color (for solid fills).
    pub fn get_fill_color(&self) -> Option<Color> {
        self.fill.as_ref().and_then(|f| f.effective_color())
    }

    /// Get the stroke color (for solid strokes).
    pub fn get_stroke_color(&self) -> Option<Color> {
        self.stroke.as_ref().and_then(|s| s.effective_color())
    }
}

impl Default for Style {
    fn default() -> Self {
        Self::new()
    }
}

/// Shorthand for creating common styles.
pub mod presets {
    use super::*;

    /// Red fill.
    pub fn red_fill() -> Style {
        Style::fill_color(Color::RED)
    }

    /// Green fill.
    pub fn green_fill() -> Style {
        Style::fill_color(Color::GREEN)
    }

    /// Blue fill.
    pub fn blue_fill() -> Style {
        Style::fill_color(Color::BLUE)
    }

    /// Black stroke (1px).
    pub fn black_stroke() -> Style {
        Style::stroke_color(Color::BLACK, 1.0)
    }

    /// White stroke (1px).
    pub fn white_stroke() -> Style {
        Style::stroke_color(Color::WHITE, 1.0)
    }

    /// Transparent fill with black stroke.
    pub fn outline() -> Style {
        Style::stroke_color(Color::BLACK, 1.0)
    }

    /// Debug style (red fill, blue 2px stroke).
    pub fn debug() -> Style {
        Style::fill_and_stroke(Paint::solid(Color::RED), Paint::solid(Color::BLUE), 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_style() {
        let style = Style::new();
        assert!(!style.is_visible());
        assert!(!style.has_fill());
        assert!(!style.has_stroke());
    }

    #[test]
    fn test_fill_only() {
        let style = Style::fill_color(Color::RED);
        assert!(style.is_visible());
        assert!(style.has_fill());
        assert!(!style.has_stroke());
    }

    #[test]
    fn test_stroke_only() {
        let style = Style::stroke_color(Color::BLUE, 2.0);
        assert!(style.is_visible());
        assert!(!style.has_fill());
        assert!(style.has_stroke());
    }

    #[test]
    fn test_fill_and_stroke() {
        let style =
            Style::fill_and_stroke(Paint::solid(Color::RED), Paint::solid(Color::BLACK), 1.0);
        assert!(style.has_fill());
        assert!(style.has_stroke());
    }
}
