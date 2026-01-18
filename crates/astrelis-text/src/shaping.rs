//! Text shaping integration with cosmic-text.
//!
//! This module handles extraction of actual glyph data from cosmic-text's Buffer
//! for use in retained rendering. It bridges cosmic-text's shaping output to our
//! GPU instance format.

use astrelis_core::{math::Vec2, profiling::profile_function};
use cosmic_text::{Attrs, Buffer, CacheKey, FontSystem, Metrics, Shaping};

/// A positioned glyph after text shaping.
///
/// Glyphs are positioned in a coordinate system where:
/// - X is the horizontal advance from the text origin
/// - Y is the baseline Y position for the line containing this glyph
///
/// Note: This uses baseline-relative positioning internally. The conversion to
/// top-left positioning happens in the glyph_atlas module via `-placement.top`.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Complete cache key for glyph rasterization
    pub cache_key: CacheKey,
    /// Position relative to text origin (horizontal_advance, baseline_y)
    pub position: Vec2,
    /// Horizontal advance for this glyph
    pub advance: f32,
}

/// Result of text shaping containing positioned glyphs.
#[derive(Debug, Clone)]
pub struct ShapedTextResult {
    /// Measured bounds (width, height)
    pub bounds: (f32, f32),
    /// Positioned glyphs ready for rendering
    pub glyphs: Vec<ShapedGlyph>,
    /// Distance from top of bounding box to the first baseline
    /// This is useful for vertical alignment - approximately 80% of line height for most fonts
    pub baseline_offset: f32,
}

impl ShapedTextResult {
    /// Create a new shaped text result.
    pub fn new(bounds: (f32, f32), glyphs: Vec<ShapedGlyph>) -> Self {
        // Approximate baseline as 80% of height (typical ascent ratio)
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
/// This function performs actual text shaping using cosmic-text's layout engine
/// and extracts positioned glyphs with their metrics for retained rendering.
pub fn shape_text(
    font_system: &mut FontSystem,
    text: &str,
    font_size: f32,
    wrap_width: Option<f32>,
    scale: f32,
) -> ShapedTextResult {
    profile_function!();

    // Create a buffer for shaping
    let metrics = Metrics::new(font_size * scale, font_size * scale * 1.2);
    let mut buffer = Buffer::new(font_system, metrics);

    // Set text
    buffer.set_text(font_system, text, Attrs::new(), Shaping::Advanced);

    // Set wrap width if specified
    if let Some(width) = wrap_width {
        buffer.set_size(font_system, Some(width * scale), None);
    }

    // Shape the text
    buffer.shape_until_scroll(font_system, false);

    // Extract glyphs
    extract_glyphs_from_buffer(&buffer, font_size, scale)
}

/// Extract glyph data from an existing cosmic-text Buffer.
///
/// This is useful when you already have a shaped buffer and want to convert
/// it to our retained rendering format.
pub fn extract_glyphs_from_buffer(buffer: &Buffer, _font_size: f32, scale: f32) -> ShapedTextResult {
    let mut max_x = 0.0_f32;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut first_line_y = None;

    // First pass: determine the bounds
    for run in buffer.layout_runs() {
        if first_line_y.is_none() {
            first_line_y = Some(run.line_y);
        }

        for glyph in run.glyphs.iter() {
            let physical = glyph.physical((0., 0.), 1.0);
            let glyph_right = physical.x as f32 + glyph.w;

            // For cosmic-text, line_y is the baseline position
            // The top of the line is approximately at line_y - (line_height * 0.8) (ascent)
            // The bottom is at line_y + (line_height * 0.2) (descent)
            let line_top = run.line_y - (run.line_height * 0.8);
            let line_bottom = run.line_y + (run.line_height * 0.2);

            max_x = max_x.max(glyph_right);
            min_y = min_y.min(line_top);
            max_y = max_y.max(line_bottom);
        }
    }

    // Second pass: create glyphs with positions relative to min_y
    let mut glyphs = Vec::new();
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            let physical = glyph.physical((0., 0.), 1.0);

            glyphs.push(ShapedGlyph {
                cache_key: physical.cache_key,
                // Position relative to the top of the text bounds
                position: Vec2::new(physical.x as f32 / scale, (run.line_y - min_y) / scale),
                advance: glyph.w / scale,
            });
        }
    }

    // Bounds should be relative to the top of the first line
    let bounds = (max_x / scale, (max_y - min_y) / scale);

    // The first line's baseline Y position relative to the top
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

    buffer.set_text(font_system, text, Attrs::new(), Shaping::Advanced);

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

        buffer.set_text(&mut font_system, "ABC", Attrs::new(), Shaping::Advanced);
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
}
