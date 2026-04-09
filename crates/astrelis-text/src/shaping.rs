//! Text shaping integration with cosmic-text.
//!
//! Provides functions to shape text using cosmic-text's layout engine and
//! extract positioned glyph data for GPU rendering.

use astrelis_core::math::Vec2;
use cosmic_text::{Attrs, Buffer, CacheKey, FontSystem, Metrics, Shaping};

/// A positioned glyph after text shaping.
///
/// Glyphs use baseline-relative positioning where X is the horizontal
/// advance from the text origin and Y is the baseline Y position.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Cache key for glyph rasterization.
    pub cache_key: CacheKey,
    /// Position relative to text origin `(horizontal_advance, baseline_y)`.
    pub position: Vec2,
    /// Horizontal advance for this glyph.
    pub advance: f32,
}

/// Result of text shaping containing positioned glyphs.
#[derive(Debug, Clone)]
pub struct ShapedTextResult {
    /// Measured bounds `(width, height)`.
    pub bounds: (f32, f32),
    /// Positioned glyphs ready for rendering.
    pub glyphs: Vec<ShapedGlyph>,
    /// Distance from top of bounding box to the first baseline.
    pub baseline_offset: f32,
}

impl ShapedTextResult {
    /// Create a new shaped text result.
    pub fn new(bounds: (f32, f32), glyphs: Vec<ShapedGlyph>) -> Self {
        let baseline_offset = bounds.1 * 0.8;
        Self {
            bounds,
            glyphs,
            baseline_offset,
        }
    }

    /// Create with explicit baseline offset.
    pub fn with_baseline(
        bounds: (f32, f32),
        glyphs: Vec<ShapedGlyph>,
        baseline_offset: f32,
    ) -> Self {
        Self {
            bounds,
            glyphs,
            baseline_offset,
        }
    }
}

/// Shape text using cosmic-text and extract glyph data.
///
/// Performs text shaping using cosmic-text's layout engine and extracts
/// positioned glyphs with metrics for retained rendering.
pub fn shape_text(
    font_system: &mut FontSystem,
    text: &str,
    font_size: f32,
    wrap_width: Option<f32>,
    scale: f32,
) -> ShapedTextResult {
    astrelis_profiling::profile_function!();

    let metrics = Metrics::new(font_size * scale, font_size * scale * 1.2);
    let mut buffer = Buffer::new(font_system, metrics);

    buffer.set_text(font_system, text, &Attrs::new(), Shaping::Advanced, None);

    if let Some(width) = wrap_width {
        buffer.set_size(font_system, Some(width * scale), None);
    }

    buffer.shape_until_scroll(font_system, false);

    extract_glyphs_from_buffer(&buffer, font_size, scale)
}

/// Extract glyph data from an existing cosmic-text [`Buffer`].
///
/// Converts cosmic-text's internal glyph representation to positioned
/// [`ShapedGlyph`] values suitable for atlas-based rendering.
pub fn extract_glyphs_from_buffer(
    buffer: &Buffer,
    _font_size: f32,
    scale: f32,
) -> ShapedTextResult {
    let mut max_x = 0.0_f32;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut first_line_y = None;

    // First pass: determine bounds
    for run in buffer.layout_runs() {
        if first_line_y.is_none() {
            first_line_y = Some(run.line_y);
        }

        for glyph in run.glyphs.iter() {
            let physical = glyph.physical((0., 0.), 1.0);
            let glyph_right = physical.x as f32 + glyph.w;

            let line_top = run.line_y - (run.line_height * 0.8);
            let line_bottom = run.line_y + (run.line_height * 0.2);

            max_x = max_x.max(glyph_right);
            min_y = min_y.min(line_top);
            max_y = max_y.max(line_bottom);
        }
    }

    // Second pass: create glyphs relative to min_y
    let mut glyphs = Vec::new();
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            let physical = glyph.physical((0., 0.), 1.0);

            glyphs.push(ShapedGlyph {
                cache_key: physical.cache_key,
                position: Vec2::new(physical.x as f32 / scale, (run.line_y - min_y) / scale),
                advance: glyph.w / scale,
            });
        }
    }

    let bounds = (max_x / scale, (max_y - min_y) / scale);
    let baseline_offset = first_line_y.map(|y| (y - min_y) / scale).unwrap_or(0.0);

    ShapedTextResult::with_baseline(bounds, glyphs, baseline_offset)
}

/// Measure text without extracting glyph data (faster for layout-only).
pub fn measure_text_fast(
    font_system: &mut FontSystem,
    text: &str,
    font_size: f32,
    wrap_width: Option<f32>,
    scale: f32,
) -> (f32, f32) {
    let metrics = Metrics::new(font_size * scale, font_size * scale * 1.2);
    let mut buffer = Buffer::new(font_system, metrics);

    buffer.set_text(font_system, text, &Attrs::new(), Shaping::Advanced, None);

    if let Some(width) = wrap_width {
        buffer.set_size(font_system, Some(width * scale), None);
    }

    buffer.shape_until_scroll(font_system, false);

    let mut max_x = 0.0_f32;
    let mut max_y = 0.0_f32;

    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            let physical = glyph.physical((0., 0.), 1.0);
            let glyph_right = physical.x as f32 + glyph.w;
            let glyph_bottom = run.line_y + run.line_height;
            max_x = max_x.max(glyph_right);
            max_y = max_y.max(glyph_bottom);
        }
    }

    (max_x / scale, max_y / scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_text_basic() {
        let mut font_system = FontSystem::new();
        let result = shape_text(&mut font_system, "Hello", 16.0, None, 1.0);

        assert!(result.bounds.0 > 0.0);
        assert!(result.bounds.1 > 0.0);
        assert!(!result.glyphs.is_empty());
    }

    #[test]
    fn test_measure_text_fast() {
        let mut font_system = FontSystem::new();
        let (width, height) = measure_text_fast(&mut font_system, "Test", 16.0, None, 1.0);

        assert!(width > 0.0);
        assert!(height > 0.0);
    }

    #[test]
    fn test_extract_glyphs_from_buffer() {
        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(16.0, 19.2);
        let mut buffer = Buffer::new(&mut font_system, metrics);

        buffer.set_text(
            &mut font_system,
            "ABC",
            &Attrs::new(),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut font_system, false);

        let result = extract_glyphs_from_buffer(&buffer, 16.0, 1.0);
        assert!(result.glyphs.len() >= 3);
    }

    #[test]
    fn test_wrapped_text() {
        let mut font_system = FontSystem::new();
        let result = shape_text(
            &mut font_system,
            "This is a long line that should wrap",
            16.0,
            Some(100.0),
            1.0,
        );

        // Should have multiple lines
        assert!(result.bounds.1 > 16.0 * 1.2);
    }

    #[test]
    fn test_empty_text() {
        let mut font_system = FontSystem::new();
        let result = shape_text(&mut font_system, "", 16.0, None, 1.0);
        assert!(result.glyphs.is_empty());
    }
}
