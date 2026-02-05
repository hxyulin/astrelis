//! Depth texture abstraction for render systems.
//!
//! Provides a first-class depth texture resource with Arc-wrapped views
//! for cheap, lifetime-free sharing across render passes and contexts.

use std::sync::Arc;

/// Default depth format used for depth textures.
pub const DEFAULT_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A depth texture with Arc-wrapped view for cheap, lifetime-free sharing.
///
/// The depth view is wrapped in an `Arc` to eliminate lifetime coupling when
/// passing depth textures to render passes. This is safe because `wgpu::TextureView`
/// is internally just an ID, so the Arc overhead is minimal.
///
/// # Example
///
/// ```ignore
/// let mut depth = DepthTexture::new(device, 800, 600, DEFAULT_DEPTH_FORMAT);
///
/// // Cheap clone of the Arc
/// let depth_view = depth.view();
///
/// // Later, if window resizes:
/// if depth.needs_resize(new_width, new_height) {
///     depth.resize(device, new_width, new_height);
/// }
/// ```
pub struct DepthTexture {
    texture: wgpu::Texture,
    view: Arc<wgpu::TextureView>,
    size: (u32, u32),
    format: wgpu::TextureFormat,
}

impl DepthTexture {
    /// Create a new depth texture with the given dimensions and format.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let (texture, view) = create_depth_texture(device, width, height, format, None);
        Self {
            texture,
            view: Arc::new(view),
            size: (width, height),
            format,
        }
    }

    /// Create a new depth texture with a debug label.
    pub fn with_label(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: &str,
    ) -> Self {
        let (texture, view) = create_depth_texture(device, width, height, format, Some(label));
        Self {
            texture,
            view: Arc::new(view),
            size: (width, height),
            format,
        }
    }

    /// Resize the depth texture if dimensions have changed.
    ///
    /// This recreates the texture and view. The old `Arc<TextureView>` remains
    /// valid until all references are dropped, but any render passes using it
    /// should be completed before resize.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.size == (width, height) {
            return;
        }

        let (texture, view) = create_depth_texture(device, width, height, self.format, None);
        self.texture = texture;
        self.view = Arc::new(view);
        self.size = (width, height);
    }

    /// Get a cheap clone of the depth view.
    ///
    /// The Arc wrapper allows the view to be shared without lifetime constraints,
    /// making it easy to pass to closures and render passes.
    pub fn view(&self) -> Arc<wgpu::TextureView> {
        self.view.clone()
    }

    /// Get a reference to the depth view (for cases where Arc is not needed).
    pub fn view_ref(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get the current size as (width, height).
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Get the width in pixels.
    pub fn width(&self) -> u32 {
        self.size.0
    }

    /// Get the height in pixels.
    pub fn height(&self) -> u32 {
        self.size.1
    }

    /// Check if the depth texture needs to be resized for the given dimensions.
    pub fn needs_resize(&self, width: u32, height: u32) -> bool {
        self.size != (width, height)
    }

    /// Get the depth format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Get the underlying wgpu texture.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

impl std::fmt::Debug for DepthTexture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DepthTexture")
            .field("size", &self.size)
            .field("format", &self.format)
            .finish()
    }
}

/// Create a depth texture and its view.
fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: Option<&str>,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: label.or(Some("Depth Texture")),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    (texture, view)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a GPU device, so they're typically run
    // as integration tests or with a test harness that provides a device.
}
