//! Shared target and frame vocabulary for Astrelis scene renderers.

#![warn(missing_docs)]

use std::{error::Error, fmt};

use astrelis_core::{
    color::Color,
    geometry::{Physical, Size},
};
use astrelis_gpu::{DeviceId, TextureDimension, TextureView};

/// A rectangular scene destination supplied by a frame compositor.
///
/// The color attachment may be multisampled and already contains earlier UI
/// layers. Scene renderers must load it and restrict every draw to `scissor`.
#[derive(Clone, Debug)]
pub struct CompositedRenderTarget {
    /// Compositor-owned color attachment.
    pub view: TextureView,
    /// Full attachment dimensions.
    pub size: Size<Physical, u32>,
    /// Full physical viewport occupied by the scene.
    pub viewport: astrelis_core::geometry::Rect<Physical, u32>,
    /// Effective rectangular clip, which does not alter scene projection.
    pub scissor: astrelis_core::geometry::Rect<Physical, u32>,
    /// Logical-to-physical scale used by pixel-oriented cameras.
    pub scale_factor: f32,
    /// Linear scene background, cleared only inside `viewport`.
    pub clear_color: Color,
}

impl CompositedRenderTarget {
    /// Validates compositor metadata for an expected device.
    pub fn validate(&self, device: DeviceId) -> Result<(), TargetError> {
        if self.view.device_id() != device || self.view.dimension() != TextureDimension::D2 {
            return Err(TargetError::new(
                "composited target is incompatible with the device",
            ));
        }
        if self.size.width == 0
            || self.size.height == 0
            || self.viewport.origin.x + self.viewport.size.width > self.size.width
            || self.viewport.origin.y + self.viewport.size.height > self.size.height
            || self.scissor.origin.x + self.scissor.size.width > self.size.width
            || self.scissor.origin.y + self.scissor.size.height > self.size.height
        {
            return Err(TargetError::new(
                "composited target region is out of bounds",
            ));
        }
        if !self.scale_factor.is_finite() || self.scale_factor <= 0.0 {
            return Err(TargetError::new("composited target scale must be positive"));
        }
        Ok(())
    }
}

/// Scene-renderer edge antialiasing policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Antialiasing {
    /// Render directly into a single-sampled target.
    None,
    /// Render into a four-sample attachment and resolve into the target.
    #[default]
    Msaa4,
}

impl Antialiasing {
    /// Returns the rasterization sample count.
    pub const fn sample_count(self) -> u32 {
        match self {
            Self::None => 1,
            Self::Msaa4 => 4,
        }
    }
}

/// A complete scene-rendering destination.
///
/// `render_size` identifies the top-left subextent containing useful pixels.
/// It may be smaller than `allocation_size` when a retained render view uses
/// resize hysteresis.
#[derive(Clone, Debug)]
pub struct RenderTarget {
    /// Single-sampled destination texture view.
    pub view: TextureView,
    /// Full dimensions of the destination texture allocation.
    pub allocation_size: Size<Physical, u32>,
    /// Top-left dimensions which should receive projected scene content.
    pub render_size: Size<Physical, u32>,
    /// Logical-to-physical scale used by pixel-oriented 2D cameras.
    pub scale_factor: f32,
    /// Linear-space color used to clear the complete allocation.
    pub clear_color: Color,
}

impl RenderTarget {
    /// Validates target metadata for the expected device.
    pub fn validate(&self, device: DeviceId) -> Result<(), TargetError> {
        if self.view.device_id() != device {
            return Err(TargetError::new("render target belongs to another device"));
        }
        if self.view.dimension() != TextureDimension::D2 || self.view.sample_count() != 1 {
            return Err(TargetError::new(
                "render target must be a single-sampled two-dimensional view",
            ));
        }
        if self.allocation_size.width == 0 || self.allocation_size.height == 0 {
            return Err(TargetError::new(
                "render target allocation must be non-empty",
            ));
        }
        if self.render_size.width > self.allocation_size.width
            || self.render_size.height > self.allocation_size.height
        {
            return Err(TargetError::new(
                "render extent cannot exceed its texture allocation",
            ));
        }
        if !self.scale_factor.is_finite() || self.scale_factor <= 0.0 {
            return Err(TargetError::new(
                "render target scale factor must be finite and positive",
            ));
        }
        Ok(())
    }

    /// Returns whether no scene pixels should be recorded.
    pub const fn is_empty(&self) -> bool {
        self.render_size.width == 0 || self.render_size.height == 0
    }
}

/// Statistics common to scene-rendering frontends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    /// Recorded GPU draw calls.
    pub draw_calls: u32,
    /// Submitted visible instances.
    pub instances: u32,
    /// Submitted triangles.
    pub triangles: u32,
    /// Instances or chunks removed by CPU visibility tests.
    pub culled: u32,
}

/// Invalid scene-render target metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetError(String);

impl TargetError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for TargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for TargetError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn antialiasing_sample_counts_are_stable() {
        assert_eq!(Antialiasing::None.sample_count(), 1);
        assert_eq!(Antialiasing::Msaa4.sample_count(), 4);
    }
}
