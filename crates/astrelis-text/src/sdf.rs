//! Signed Distance Field (SDF) text rendering.
//!
//! SDF stores distance information in textures instead of grayscale values,
//! enabling sharp text at any scale and high-quality effects (outlines,
//! shadows, glows).

use cosmic_text::SwashImage;

/// Convert a [`SwashImage`] to single-channel grayscale (alpha mask).
///
/// Handles all swash content types:
/// - `Mask` — already 1 byte/pixel, returned as-is
/// - `SubpixelMask` / `Color` — 4 bytes/pixel RGBA, converted to luminance
pub fn swash_image_to_grayscale(image: &SwashImage) -> Vec<u8> {
    astrelis_profiling::profile_function!();
    let width = image.placement.width as usize;
    let height = image.placement.height as usize;
    let pixel_count = width * height;

    match image.content {
        cosmic_text::SwashContent::Mask => {
            // Already 1 byte per pixel
            image.data[..pixel_count].to_vec()
        }
        cosmic_text::SwashContent::SubpixelMask | cosmic_text::SwashContent::Color => {
            // 4 bytes per pixel (RGBA): convert to single-channel via max of RGB * A
            let mut out = vec![0u8; pixel_count];
            for i in 0..pixel_count {
                let base = i * 4;
                if base + 3 < image.data.len() {
                    let r = image.data[base] as u16;
                    let g = image.data[base + 1] as u16;
                    let b = image.data[base + 2] as u16;
                    let a = image.data[base + 3] as u16;
                    // Use max channel * alpha for sharpest edge detection
                    let max_rgb = r.max(g).max(b);
                    out[i] = ((max_rgb * a) / 255).min(255) as u8;
                }
            }
            out
        }
    }
}

/// Text rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextRenderMode {
    /// Standard grayscale bitmap rendering.
    #[default]
    Bitmap,
    /// Signed Distance Field rendering.
    SDF {
        /// Distance field spread in pixels (typically 2.0 to 8.0).
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

/// Default edge threshold for SDF generation.
///
/// Pixels with grayscale values at or above this threshold are considered
/// "inside" the glyph. The default of 128 splits the 0–255 range evenly.
pub const DEFAULT_SDF_THRESHOLD: u8 = 128;

/// Generate a signed distance field from a grayscale bitmap.
///
/// Uses [`DEFAULT_SDF_THRESHOLD`] for edge detection. For a custom
/// threshold, use [`generate_sdf_with_threshold`].
///
/// # Arguments
///
/// * `grayscale` - Single-channel grayscale data (1 byte/pixel, use [`swash_image_to_grayscale`] to convert)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
/// * `spread` - Distance field spread in pixels
pub fn generate_sdf(grayscale: &[u8], width: usize, height: usize, spread: f32) -> Vec<u8> {
    astrelis_profiling::profile_function!();
    generate_sdf_with_threshold(grayscale, width, height, spread, DEFAULT_SDF_THRESHOLD)
}

/// Generate an SDF with a custom edge threshold.
///
/// The `threshold` controls which grayscale values are considered "inside"
/// the glyph (values >= threshold). Lower values expand the glyph boundary;
/// higher values shrink it.
pub fn generate_sdf_with_threshold(
    grayscale: &[u8],
    width: usize,
    height: usize,
    spread: f32,
    threshold: u8,
) -> Vec<u8> {
    astrelis_profiling::profile_function!();

    if width == 0 || height == 0 {
        return Vec::new();
    }

    let mut output = vec![0u8; width * height];
    let search_radius = (spread.ceil() as i32) + 1;

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let value = grayscale[idx];
            let inside = value >= threshold;

            let mut min_dist = spread;

            for dy in -search_radius..=search_radius {
                for dx in -search_radius..=search_radius {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                        continue;
                    }

                    let nidx = ny as usize * width + nx as usize;
                    let neighbor_inside = grayscale[nidx] >= threshold;

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

/// Generate an SDF with bilinear interpolation for smoother results.
///
/// Higher quality but slower than [`generate_sdf`].
///
/// # Arguments
///
/// * `grayscale` - Single-channel grayscale data (1 byte/pixel, use [`swash_image_to_grayscale`] to convert)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
/// * `spread` - Distance field spread in pixels
pub fn generate_sdf_smooth(grayscale: &[u8], width: usize, height: usize, spread: f32) -> Vec<u8> {
    astrelis_profiling::profile_function!();
    if width == 0 || height == 0 {
        return Vec::new();
    }

    let mut output = vec![0u8; width * height];
    let search_radius = (spread.ceil() as i32) + 1;

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;

            let source_value = bilinear_sample(grayscale, width, height, x as f32, y as f32);
            let threshold = 0.5f32;
            let inside = source_value >= threshold;

            let mut min_dist = spread;

            for dy in -search_radius..=search_radius {
                for dx in -search_radius..=search_radius {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                        continue;
                    }

                    let neighbor_value = bilinear_sample(grayscale, width, height, nx as f32, ny as f32);
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
fn bilinear_sample(data: &[u8], width: usize, height: usize, x: f32, y: f32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = (x0 + 1).min(width as i32 - 1);
    let y1 = (y0 + 1).min(height as i32 - 1);

    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let sample = |ix: i32, iy: i32| -> f32 {
        if ix < 0 || iy < 0 || ix >= width as i32 || iy >= height as i32 {
            0.0
        } else {
            let idx = iy as usize * width + ix as usize;
            data[idx] as f32 / 255.0
        }
    };

    let v00 = sample(x0, y0);
    let v10 = sample(x1, y0);
    let v01 = sample(x0, y1);
    let v11 = sample(x1, y1);

    let v0 = v00 * (1.0 - fx) + v10 * fx;
    let v1 = v01 * (1.0 - fx) + v11 * fx;
    v0 * (1.0 - fy) + v1 * fy
}

/// SDF rendering configuration.
#[derive(Debug, Clone)]
pub struct SdfConfig {
    /// Render mode.
    pub mode: TextRenderMode,
    /// Edge softness for anti-aliasing (0.0 to 1.0).
    pub edge_softness: f32,
    /// Outline width (0.0 = no outline).
    pub outline_width: f32,
    /// Use smooth SDF generation (slower but higher quality).
    pub smooth: bool,
    /// Base size for SDF glyph rasterization in pixels (default: 48.0).
    pub base_size: f32,
    /// Default SDF spread in pixels (default: 4.0).
    pub default_spread: f32,
}

impl Default for SdfConfig {
    fn default() -> Self {
        Self {
            mode: TextRenderMode::Bitmap,
            edge_softness: 0.05,
            outline_width: 0.0,
            smooth: false,
            base_size: 48.0,
            default_spread: 4.0,
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
        let config = SdfConfig::new().edge_softness(2.0);
        assert_eq!(config.edge_softness, 1.0);
    }

    #[test]
    fn test_sdf_config_outline_width_clamp() {
        let config = SdfConfig::new().outline_width(-5.0);
        assert_eq!(config.outline_width, 0.0);
    }
}
