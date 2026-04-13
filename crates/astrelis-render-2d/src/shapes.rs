//! Shape primitive to Instance2D conversion.

use astrelis_core::color::Color;
use astrelis_core::math::Vec2;

use crate::instance::{DrawType, Instance2D};

/// Converts a filled rectangle to an Instance2D.
pub(crate) fn filled_rect(position: Vec2, size: Vec2, color: Color, z_depth: f32) -> Instance2D {
    Instance2D {
        position: position.into(),
        size: size.into(),
        uv_min: [0.0, 0.0],
        uv_max: [1.0, 1.0],
        color: [color.r, color.g, color.b, color.a],
        rotation: 0.0,
        z_depth,
        texture_index: 0, // white pixel
        draw_type: DrawType::Rect as u32,
    }
}

/// Converts an outlined rectangle to instances (4 thin filled rects for edges).
pub(crate) fn outlined_rect(
    position: Vec2,
    size: Vec2,
    color: Color,
    thickness: f32,
    z_depth: f32,
) -> [Instance2D; 4] {
    let t = thickness;
    // Top edge
    let top = filled_rect(position, Vec2::new(size.x, t), color, z_depth);
    // Bottom edge
    let bottom = filled_rect(
        Vec2::new(position.x, position.y + size.y - t),
        Vec2::new(size.x, t),
        color,
        z_depth,
    );
    // Left edge (between top and bottom)
    let left = filled_rect(
        Vec2::new(position.x, position.y + t),
        Vec2::new(t, size.y - 2.0 * t),
        color,
        z_depth,
    );
    // Right edge
    let right = filled_rect(
        Vec2::new(position.x + size.x - t, position.y + t),
        Vec2::new(t, size.y - 2.0 * t),
        color,
        z_depth,
    );
    [top, bottom, left, right]
}

/// Converts a filled circle to an Instance2D (SDF in fragment shader).
pub(crate) fn filled_circle(center: Vec2, radius: f32, color: Color, z_depth: f32) -> Instance2D {
    Instance2D {
        position: [center.x - radius, center.y - radius],
        size: [radius * 2.0, radius * 2.0],
        uv_min: [0.0, 0.0],
        uv_max: [1.0, 1.0],
        color: [color.r, color.g, color.b, color.a],
        rotation: 0.0,
        z_depth,
        texture_index: 0,
        draw_type: DrawType::Circle as u32,
    }
}

/// Converts a line segment to a thin quad Instance2D.
pub(crate) fn line(start: Vec2, end: Vec2, thickness: f32, color: Color, z_depth: f32) -> Instance2D {
    let diff = end - start;
    let length = diff.length();
    let angle = diff.y.atan2(diff.x);

    Instance2D {
        position: start.into(),
        size: [length, thickness],
        uv_min: [0.0, 0.0],
        uv_max: [1.0, 1.0],
        color: [color.r, color.g, color.b, color.a],
        rotation: angle,
        z_depth,
        texture_index: 0,
        draw_type: DrawType::Line as u32,
    }
}
