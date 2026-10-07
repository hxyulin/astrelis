//! Anti-aliased rounded-rectangle clipping evaluated by built-in 2D shading.

use crate::{CornerRadii, Error, Rect, Transform2D};
use bytemuck::{Pod, Zeroable};

/// A rounded-rectangle clip applied by built-in 2D shading in a [`crate::RenderPass`].
///
/// `rect` and `radii` use clip-local units; `transform` maps them to physical pixels
/// relative to the viewport's top-left, like a pixel-space draw transform. Any
/// invertible affine transform is supported, so rotated and scaled clips stay exact.
/// Coverage is filtered over about one pixel at the boundary, independently of MSAA,
/// and multiplies the output of shapes, lines, paths, text and built-in image shading.
/// Custom texture shaders receive the clip record at locations seven to nine and may
/// evaluate it the same way. Meshes, polylines and markers ignore it; the pass
/// scissor still applies to them.
///
/// Only one rounded clip is active at a time. Set the pass scissor to the clip's
/// bounds as well, so fragments outside it are not shaded at all.
/// [`crate::PaintSession::clipped_rounded`] does both and restores the parent's clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundedClip {
    /// Clip rectangle in clip-local units.
    pub rect: Rect,
    /// Corner radii in clip-local units, fitted to the rectangle like shape radii.
    pub radii: CornerRadii,
    /// Clip-local units to viewport-relative physical pixels.
    pub transform: Transform2D,
}
impl RoundedClip {
    /// A clip in viewport-relative pixels with an identity transform.
    pub const fn new(rect: Rect, radii: CornerRadii) -> Self {
        Self {
            rect,
            radii,
            transform: Transform2D::IDENTITY,
        }
    }
    /// Selects the transform from clip-local units to viewport-relative pixels.
    pub fn transform(mut self, transform: impl Into<Transform2D>) -> Self {
        self.transform = transform.into();
        self
    }
    /// Checks finite geometry, nonnegative extents/radii and an invertible transform.
    pub fn validate(&self) -> Result<(), Error> {
        if !self.rect.valid() || !self.radii.valid() || self.transform.invert().is_none() {
            return Err(Error::InvalidClip);
        }
        Ok(())
    }
    // Maps viewport-normalized positions to clip-local units centered on the rectangle.
    pub(crate) fn record(&self, viewport: [f32; 2]) -> ClipRecord {
        let inverse = self.transform.invert().unwrap_or(Transform2D::IDENTITY);
        let [a, b, c, d, x, y] = inverse.0;
        let [w, h] = viewport;
        let Rect {
            x: left,
            y: top,
            width,
            height,
        } = self.rect;
        ClipRecord {
            axes: [a * w, b * w, c * h, d * h],
            offset_half: [
                x - (left + width * 0.5),
                y - (top + height * 0.5),
                width * 0.5,
                height * 0.5,
            ],
            radii: self.radii.fitted(width, height),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct ClipRecord {
    axes: [f32; 4],
    offset_half: [f32; 4],
    radii: [f32; 4],
}
// A negative half size disables clipping in the shader.
pub(crate) const DISABLED: ClipRecord = ClipRecord {
    axes: [0.; 4],
    offset_half: [0., 0., -1., -1.],
    radii: [0.; 4],
};
pub(crate) const SIZE: u64 = std::mem::size_of::<ClipRecord>() as u64;

/// One per-draw record at stride zero, at three consecutive shader locations.
pub(crate) fn layout(first: u32) -> crate::VertexLayout {
    crate::VertexLayout {
        stride: 0,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: (0..3)
            .map(|i| wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: i * 16,
                shader_location: first + i as u32,
            })
            .collect(),
    }
}

/// Prepends the shared clip functions to a built-in shader.
pub(crate) fn shader(base: &str) -> String {
    format!("{}\n{base}", include_str!("clip.wgsl"))
}
