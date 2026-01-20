//! Framebuffer abstraction for offscreen rendering.

use crate::context::GraphicsContext;
use crate::types::GpuTexture;

/// Depth format used by framebuffers.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// An offscreen render target with optional depth and MSAA attachments.
pub struct Framebuffer {
    /// Color texture (always sample_count=1, used as resolve target or direct render).
    color: GpuTexture,
    /// Depth texture (sample count matches MSAA if enabled).
    depth: Option<GpuTexture>,
    /// MSAA texture (sample_count > 1, render target when MSAA enabled).
    msaa: Option<GpuTexture>,
    /// The render sample count (1 if no MSAA, otherwise the MSAA sample count).
    sample_count: u32,
}

impl std::fmt::Debug for Framebuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Framebuffer")
            .field("width", &self.color.width())
            .field("height", &self.color.height())
            .field("format", &self.color.format())
            .field("sample_count", &self.sample_count)
            .field("has_depth", &self.depth.is_some())
            .field("has_msaa", &self.msaa.is_some())
            .finish()
    }
}

impl Framebuffer {
    /// Create a new framebuffer builder.
    pub fn builder(width: u32, height: u32) -> FramebufferBuilder {
        FramebufferBuilder::new(width, height)
    }

    /// Get the color texture (resolved, non-MSAA).
    pub fn color_texture(&self) -> &wgpu::Texture {
        use crate::extension::AsWgpu;
        self.color.as_wgpu()
    }

    /// Get the color texture view (resolved, non-MSAA).
    pub fn color_view(&self) -> &wgpu::TextureView {
        self.color.view()
    }

    /// Get the depth texture, if present.
    pub fn depth_texture(&self) -> Option<&wgpu::Texture> {
        use crate::extension::AsWgpu;
        self.depth.as_ref().map(|d| d.as_wgpu())
    }

    /// Get the depth texture view, if present.
    pub fn depth_view(&self) -> Option<&wgpu::TextureView> {
        self.depth.as_ref().map(|d| d.view())
    }

    /// Get the MSAA texture (render target when MSAA enabled).
    pub fn msaa_texture(&self) -> Option<&wgpu::Texture> {
        use crate::extension::AsWgpu;
        self.msaa.as_ref().map(|m| m.as_wgpu())
    }

    /// Get the MSAA texture view (render target when MSAA enabled).
    pub fn msaa_view(&self) -> Option<&wgpu::TextureView> {
        self.msaa.as_ref().map(|m| m.view())
    }

    /// Get the view to render to (MSAA view if enabled, otherwise color view).
    pub fn render_view(&self) -> &wgpu::TextureView {
        self.msaa.as_ref().map(|m| m.view()).unwrap_or(self.color.view())
    }

    /// Get the resolve target (color view if MSAA enabled, None otherwise).
    pub fn resolve_target(&self) -> Option<&wgpu::TextureView> {
        if self.msaa.is_some() {
            Some(self.color.view())
        } else {
            None
        }
    }

    /// Get the framebuffer width.
    pub fn width(&self) -> u32 {
        self.color.width()
    }

    /// Get the framebuffer height.
    pub fn height(&self) -> u32 {
        self.color.height()
    }

    /// Get the framebuffer size as (width, height).
    pub fn size(&self) -> (u32, u32) {
        (self.color.width(), self.color.height())
    }

    /// Get the color format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.color.format()
    }

    /// Get the sample count (1 if no MSAA).
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    /// Check if MSAA is enabled.
    pub fn has_msaa(&self) -> bool {
        self.sample_count > 1
    }

    /// Check if depth buffer is enabled.
    pub fn has_depth(&self) -> bool {
        self.depth.is_some()
    }

    /// Resize the framebuffer, recreating all textures.
    pub fn resize(&mut self, context: &GraphicsContext, width: u32, height: u32) {
        if self.color.width() == width && self.color.height() == height {
            return;
        }

        let new_fb = FramebufferBuilder::new(width, height)
            .format(self.color.format())
            .sample_count_if(self.sample_count > 1, self.sample_count)
            .depth_if(self.depth.is_some())
            .build(context);

        *self = new_fb;
    }
}

/// Builder for creating framebuffers with optional attachments.
pub struct FramebufferBuilder {
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    sample_count: u32,
    with_depth: bool,
    label: Option<&'static str>,
}

impl FramebufferBuilder {
    /// Create a new framebuffer builder with the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            sample_count: 1,
            with_depth: false,
            label: None,
        }
    }

    /// Set the color format.
    pub fn format(mut self, format: wgpu::TextureFormat) -> Self {
        self.format = format;
        self
    }

    /// Enable MSAA with the given sample count (typically 4).
    pub fn with_msaa(mut self, sample_count: u32) -> Self {
        self.sample_count = sample_count;
        self
    }

    /// Conditionally set sample count.
    pub fn sample_count_if(mut self, condition: bool, sample_count: u32) -> Self {
        if condition {
            self.sample_count = sample_count;
        }
        self
    }

    /// Enable depth buffer.
    pub fn with_depth(mut self) -> Self {
        self.with_depth = true;
        self
    }

    /// Conditionally enable depth buffer.
    pub fn depth_if(mut self, condition: bool) -> Self {
        self.with_depth = condition;
        self
    }

    /// Set a debug label for the framebuffer textures.
    pub fn label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    /// Build the framebuffer.
    pub fn build(self, context: &GraphicsContext) -> Framebuffer {
        let label_prefix = self.label.unwrap_or("Framebuffer");

        let size = wgpu::Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        };

        // Create color texture (always sample_count=1, used as resolve target or direct render)
        let color = GpuTexture::new(
            &context.device,
            &wgpu::TextureDescriptor {
                label: Some(&format!("{} Color", label_prefix)),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
        );

        // Create MSAA texture if sample_count > 1
        let msaa = if self.sample_count > 1 {
            Some(GpuTexture::new(
                &context.device,
                &wgpu::TextureDescriptor {
                    label: Some(&format!("{} MSAA", label_prefix)),
                    size,
                    mip_level_count: 1,
                    sample_count: self.sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                },
            ))
        } else {
            None
        };

        // Create depth texture if requested
        let depth = if self.with_depth {
            let depth_sample_count = if self.sample_count > 1 {
                self.sample_count
            } else {
                1
            };

            Some(GpuTexture::new(
                &context.device,
                &wgpu::TextureDescriptor {
                    label: Some(&format!("{} Depth", label_prefix)),
                    size,
                    mip_level_count: 1,
                    sample_count: depth_sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format: DEPTH_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            ))
        } else {
            None
        };

        Framebuffer {
            color,
            depth,
            msaa,
            sample_count: self.sample_count,
        }
    }
}
