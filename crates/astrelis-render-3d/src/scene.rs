//! CPU-side lighting, material, and draw-list data.

use astrelis_core::{
    color::Color,
    math::{Mat4, Vec3},
};

use crate::{MaterialHandle, MeshHandle};

/// Material alpha policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlphaMode {
    /// Fully opaque with depth writes.
    Opaque,
    /// Discard fragments below this alpha threshold.
    Mask(f32),
    /// Premultiplied blending without depth writes.
    Blend,
}

/// Basic Lambert material settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialDescriptor {
    /// Linear straight-alpha base color.
    pub base_color: Color,
    /// Optional registered albedo texture; white is used when absent.
    pub albedo: Option<crate::TextureHandle>,
    /// Fragment alpha behavior.
    pub alpha_mode: AlphaMode,
    /// Disable back-face culling.
    pub double_sided: bool,
}

impl Default for MaterialDescriptor {
    fn default() -> Self {
        Self {
            base_color: Color::WHITE,
            albedo: None,
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
        }
    }
}

/// One directional Lambert light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectionalLight {
    /// Normalized world direction from a surface toward the light.
    pub direction_to_light: Vec3,
    /// Linear light color.
    pub color: Color,
    /// Nonnegative diffuse multiplier.
    pub intensity: f32,
}

/// Frame lighting parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lighting {
    /// Linear ambient color.
    pub ambient_color: Color,
    /// Nonnegative ambient multiplier.
    pub ambient_intensity: f32,
    /// Directional diffuse source.
    pub directional: DirectionalLight,
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            ambient_color: Color::WHITE,
            ambient_intensity: 0.15,
            directional: DirectionalLight {
                direction_to_light: Vec3::new(0.3, 1.0, 0.5).normalize(),
                color: Color::WHITE,
                intensity: 0.85,
            },
        }
    }
}

/// One retained mesh instance submission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshDraw {
    /// Registered mesh.
    pub mesh: MeshHandle,
    /// Registered material.
    pub material: MaterialHandle,
    /// Object-to-world transform.
    pub transform: Mat4,
    /// Per-instance straight-alpha tint.
    pub tint: Color,
}

/// One depth-tested colored line segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebugLine {
    /// World-space start.
    pub start: Vec3,
    /// World-space end.
    pub end: Vec3,
    /// Linear straight-alpha color.
    pub color: Color,
}

/// Per-camera 3D scene submissions.
#[derive(Clone, Debug, Default)]
pub struct DrawList3D {
    pub(crate) meshes: Vec<MeshDraw>,
    pub(crate) lines: Vec<DebugLine>,
}

impl DrawList3D {
    /// Creates an empty draw list.
    pub const fn new() -> Self {
        Self {
            meshes: Vec::new(),
            lines: Vec::new(),
        }
    }
    /// Records a mesh instance.
    pub fn draw_mesh(&mut self, draw: MeshDraw) {
        self.meshes.push(draw);
    }
    /// Records a line segment.
    pub fn draw_line(&mut self, line: DebugLine) {
        self.lines.push(line);
    }
    /// Records an XZ grid centered on the origin.
    pub fn draw_grid(&mut self, half_lines: u32, spacing: f32, color: Color) {
        let extent = half_lines as f32 * spacing;
        for index in 0..=half_lines * 2 {
            let offset = (index as f32 - half_lines as f32) * spacing;
            self.draw_line(DebugLine {
                start: Vec3::new(-extent, 0.0, offset),
                end: Vec3::new(extent, 0.0, offset),
                color,
            });
            self.draw_line(DebugLine {
                start: Vec3::new(offset, 0.0, -extent),
                end: Vec3::new(offset, 0.0, extent),
                color,
            });
        }
    }
    /// Records RGB local axes under a world transform.
    pub fn draw_axes(&mut self, transform: Mat4, length: f32) {
        let origin = transform.transform_point3(Vec3::ZERO);
        for (axis, color) in [
            (Vec3::X, Color::RED),
            (Vec3::Y, Color::GREEN),
            (Vec3::Z, Color::BLUE),
        ] {
            self.draw_line(DebugLine {
                start: origin,
                end: transform.transform_point3(axis * length),
                color,
            });
        }
    }
    /// Clears all submissions while retaining capacities.
    pub fn clear(&mut self) {
        self.meshes.clear();
        self.lines.clear();
    }
}
