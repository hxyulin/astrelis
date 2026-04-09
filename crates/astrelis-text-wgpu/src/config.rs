//! Configuration for text renderer backends.

use astrelis_text::SdfConfig;

/// Configuration for text renderer backends.
///
/// Controls atlas texture sizes and surface formats for pipelines.
///
/// # Memory Usage
///
/// | Config | Atlas Size | Memory/Atlas |
/// |--------|------------|--------------|
/// | `small()` | 512x512 | ~0.5 MB |
/// | `medium()` | 1024x1024 | ~2 MB |
/// | `large()` | 2048x2048 | ~8 MB |
#[derive(Clone, Debug)]
pub struct TextRendererConfig {
    /// Atlas texture size (width and height, should be power of 2).
    pub atlas_size: u32,
    /// SDF-specific settings.
    pub sdf: SdfConfig,
    /// Surface texture format for pipelines.
    pub surface_format: wgpu::TextureFormat,
    /// Depth format for z-ordering. `None` disables depth testing.
    pub depth_format: Option<wgpu::TextureFormat>,
}

impl Default for TextRendererConfig {
    fn default() -> Self {
        Self {
            atlas_size: 2048,
            sdf: SdfConfig::default(),
            surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            depth_format: None,
        }
    }
}

impl TextRendererConfig {
    /// Create default configuration (2048x2048 atlas).
    pub fn new() -> Self {
        Self::default()
    }

    /// Small config for memory-constrained environments (512x512).
    pub fn small() -> Self {
        Self {
            atlas_size: 512,
            ..Default::default()
        }
    }

    /// Medium config (1024x1024).
    pub fn medium() -> Self {
        Self {
            atlas_size: 1024,
            ..Default::default()
        }
    }

    /// Large config for text-heavy applications (2048x2048).
    pub fn large() -> Self {
        Self::default()
    }

    /// Set custom atlas size.
    pub fn with_atlas_size(mut self, size: u32) -> Self {
        self.atlas_size = size;
        self
    }

    /// Set SDF configuration.
    pub fn with_sdf_config(mut self, config: SdfConfig) -> Self {
        self.sdf = config;
        self
    }

    /// Set surface format.
    pub fn with_surface_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.surface_format = format;
        self
    }

    /// Set depth format.
    pub fn with_depth_format(mut self, format: Option<wgpu::TextureFormat>) -> Self {
        self.depth_format = format;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = TextRendererConfig::default();
        assert_eq!(config.atlas_size, 2048);
        assert_eq!(config.surface_format, wgpu::TextureFormat::Bgra8UnormSrgb);
        assert!(config.depth_format.is_none());
    }

    #[test]
    fn test_config_presets() {
        assert_eq!(TextRendererConfig::small().atlas_size, 512);
        assert_eq!(TextRendererConfig::medium().atlas_size, 1024);
        assert_eq!(TextRendererConfig::large().atlas_size, 2048);
    }
}
