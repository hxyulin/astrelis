//! Signed Distance Field (SDF) text rendering.
//!
//! SDF rendering stores distance information in textures instead of grayscale values,
//! enabling sharp text rendering at any scale without artifacts.
//!
//! # Algorithm
//!
//! For each pixel in the SDF texture, we store the distance to the nearest edge:
//! - Inside the glyph: positive distance (0.5 to 1.0)
//! - Outside the glyph: negative distance (0.0 to 0.5)
//! - Exactly on the edge: 0.5
//!
//! # Benefits
//!
//! - Resolution-independent scaling
//! - Better outline and shadow effects
//! - Reduced texture memory (can use lower resolution)
//! - Smooth anti-aliasing at any scale
//!
//! # Example
//!
//! ```ignore
//! use astrelis_text::*;
//!
//! let mut renderer = FontRenderer::new(context, font_system);
//! renderer.set_render_mode(TextRenderMode::SDF { spread: 4.0 });
//!
//! // Text will now render using SDF
//! let buffer = renderer.prepare(&text);
//! renderer.draw_text(&buffer, position);
//! ```

use cosmic_text::SwashImage;

/// Text rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextRenderMode {
    /// Standard grayscale bitmap rendering.
    #[default]
    Bitmap,
    /// Signed Distance Field rendering.
    SDF {
        /// Distance field spread in pixels.
        /// Higher values = smoother at larger scales, but more texture memory.
        /// Typical values: 2.0 to 8.0
        spread: f32,
    },
}

impl TextRenderMode {
    /// Check if this is SDF mode.
    pub fn is_sdf(&self) -> bool {
        matches!(self, Self::SDF { .. })
    }

    /// Get the SDF spread value, or 0.0 for bitmap mode.
    pub fn spread(&self) -> f32 {
        match self {
            Self::SDF { spread } => *spread,
            Self::Bitmap => 0.0,
        }
    }
}

/// Generate a signed distance field from a grayscale bitmap.
///
/// This uses a brute-force algorithm that's simple but slow for large textures.
/// For production use, consider using a more efficient algorithm like:
/// - Dead reckoning (8SSEDT)
/// - Jump flooding algorithm (JFA)
///
/// # Arguments
///
/// * `source` - Source grayscale image (0-255)
/// * `spread` - Distance field spread in pixels
///
/// # Returns
///
/// SDF image where:
/// - 0 = far outside (distance > spread)
/// - 127 = exactly on edge
/// - 255 = far inside (distance > spread)
pub fn generate_sdf(source: &SwashImage, spread: f32) -> Vec<u8> {
    let width = source.placement.width as usize;
    let height = source.placement.height as usize;

    if width == 0 || height == 0 {
        return Vec::new();
    }

    let mut output = vec![0u8; width * height];
    let threshold = 128u8;

    // For each output pixel, find distance to nearest edge
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let value = source.data[idx];

            // Determine if we're inside or outside the glyph
            let inside = value >= threshold;

            // Find minimum distance to an edge pixel
            let mut min_dist = spread;

            // Search within the spread radius
            let search_radius = (spread.ceil() as i32) + 1;

            for dy in -search_radius..=search_radius {
                for dx in -search_radius..=search_radius {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    // Skip out of bounds
                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                        continue;
                    }

                    let nidx = ny as usize * width + nx as usize;
                    let neighbor_value = source.data[nidx];
                    let neighbor_inside = neighbor_value >= threshold;

                    // Found an edge
                    if inside != neighbor_inside {
                        let dist = ((dx * dx + dy * dy) as f32).sqrt();
                        min_dist = min_dist.min(dist);
                    }
                }
            }

            // Normalize distance to [0, 1]
            let normalized = (min_dist / spread).clamp(0.0, 1.0);

            // Map to [0, 255]
            // Inside: 0.5 to 1.0 -> 127 to 255
            // Outside: 0.0 to 0.5 -> 0 to 127
            let sdf_value = if inside {
                127.0 + normalized * 128.0
            } else {
                127.0 - normalized * 127.0
            };

            output[idx] = sdf_value.clamp(0.0, 255.0) as u8;
        }
    }

    output
}

/// Generate a signed distance field with bilinear interpolation for smoother results.
///
/// This is a higher quality but slower version of `generate_sdf`.
pub fn generate_sdf_smooth(source: &SwashImage, spread: f32) -> Vec<u8> {
    let width = source.placement.width as usize;
    let height = source.placement.height as usize;

    if width == 0 || height == 0 {
        return Vec::new();
    }

    let mut output = vec![0u8; width * height];

    // For each output pixel
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;

            // Sample the source with bilinear filtering
            let source_value = bilinear_sample(source, x as f32, y as f32);
            let threshold = 0.5f32;
            let inside = source_value >= threshold;

            // Find minimum distance to edge
            let mut min_dist = spread;
            let search_radius = (spread.ceil() as i32) + 1;

            for dy in -search_radius..=search_radius {
                for dx in -search_radius..=search_radius {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                        continue;
                    }

                    let neighbor_value = bilinear_sample(source, nx as f32, ny as f32);
                    let neighbor_inside = neighbor_value >= threshold;

                    if inside != neighbor_inside {
                        let dist = ((dx * dx + dy * dy) as f32).sqrt();
                        min_dist = min_dist.min(dist);
                    }
                }
            }

            let normalized = (min_dist / spread).clamp(0.0, 1.0);
            let sdf_value = if inside {
                127.0 + normalized * 128.0
            } else {
                127.0 - normalized * 127.0
            };

            output[idx] = sdf_value.clamp(0.0, 255.0) as u8;
        }
    }

    output
}

/// Bilinear sampling helper for smooth SDF generation.
fn bilinear_sample(image: &SwashImage, x: f32, y: f32) -> f32 {
    let width = image.placement.width as usize;
    let height = image.placement.height as usize;

    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = (x0 + 1).min(width as i32 - 1);
    let y1 = (y0 + 1).min(height as i32 - 1);

    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    // Sample corners
    let sample = |ix: i32, iy: i32| -> f32 {
        if ix < 0 || iy < 0 || ix >= width as i32 || iy >= height as i32 {
            0.0
        } else {
            let idx = iy as usize * width + ix as usize;
            image.data[idx] as f32 / 255.0
        }
    };

    let v00 = sample(x0, y0);
    let v10 = sample(x1, y0);
    let v01 = sample(x0, y1);
    let v11 = sample(x1, y1);

    // Bilinear interpolation
    let v0 = v00 * (1.0 - fx) + v10 * fx;
    let v1 = v01 * (1.0 - fx) + v11 * fx;
    v0 * (1.0 - fy) + v1 * fy
}

/// SDF rendering configuration.
#[derive(Debug, Clone)]
pub struct SdfConfig {
    /// Render mode
    pub mode: TextRenderMode,
    /// Edge softness for anti-aliasing (0.0 to 1.0)
    /// Lower = sharper, Higher = softer
    pub edge_softness: f32,
    /// Outline width (0.0 = no outline)
    pub outline_width: f32,
    /// Use smooth SDF generation (slower but higher quality)
    pub smooth: bool,
}

impl Default for SdfConfig {
    fn default() -> Self {
        Self {
            mode: TextRenderMode::Bitmap,
            edge_softness: 0.05,
            outline_width: 0.0,
            smooth: false,
        }
    }
}

impl SdfConfig {
    /// Create a new SDF config with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable SDF rendering with the specified spread.
    pub fn with_sdf(mut self, spread: f32) -> Self {
        self.mode = TextRenderMode::SDF { spread };
        self
    }

    /// Set edge softness.
    pub fn edge_softness(mut self, softness: f32) -> Self {
        self.edge_softness = softness.clamp(0.0, 1.0);
        self
    }

    /// Set outline width.
    pub fn outline_width(mut self, width: f32) -> Self {
        self.outline_width = width.max(0.0);
        self
    }

    /// Enable smooth SDF generation.
    pub fn smooth(mut self, enable: bool) -> Self {
        self.smooth = enable;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_mode_default() {
        let mode = TextRenderMode::default();
        assert!(!mode.is_sdf());
        assert_eq!(mode.spread(), 0.0);
    }

    #[test]
    fn test_render_mode_sdf() {
        let mode = TextRenderMode::SDF { spread: 4.0 };
        assert!(mode.is_sdf());
        assert_eq!(mode.spread(), 4.0);
    }

    #[test]
    fn test_render_mode_bitmap() {
        let mode = TextRenderMode::Bitmap;
        assert!(!mode.is_sdf());
        assert_eq!(mode.spread(), 0.0);
    }

    #[test]
    fn test_sdf_config_default() {
        let config = SdfConfig::default();
        assert!(!config.mode.is_sdf());
        assert_eq!(config.edge_softness, 0.05);
        assert_eq!(config.outline_width, 0.0);
        assert!(!config.smooth);
    }

    #[test]
    fn test_sdf_config_builder() {
        let config = SdfConfig::new()
            .with_sdf(6.0)
            .edge_softness(0.1)
            .outline_width(2.0)
            .smooth(true);

        assert!(config.mode.is_sdf());
        assert_eq!(config.mode.spread(), 6.0);
        assert_eq!(config.edge_softness, 0.1);
        assert_eq!(config.outline_width, 2.0);
        assert!(config.smooth);
    }

    #[test]
    fn test_sdf_config_edge_softness_clamp() {
        let config = SdfConfig::new().edge_softness(2.0); // Should clamp to 1.0

        assert_eq!(config.edge_softness, 1.0);
    }

    #[test]
    fn test_sdf_config_outline_width_clamp() {
        let config = SdfConfig::new().outline_width(-5.0); // Should clamp to 0.0

        assert_eq!(config.outline_width, 0.0);
    }

    // Note: Full SDF generation tests require proper SwashImage setup which is complex.
    // The generate_sdf and generate_sdf_smooth functions are integration-tested
    // in the renderer when actual glyphs are rasterized.
}
