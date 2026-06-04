//! Pure geometry for debug primitives (grid, axes).
//!
//! Kept as data-producing functions so they are unit-testable without
//! a GPU; `Renderer3D` feeds the segments into its line buffer.

use astrelis_core::color::Color;
use astrelis_core::math::{Mat4, Vec3};

/// Grid lines on the XZ plane at y=0, every `spacing` units out to
/// `±half_extent` on both axes.
pub(crate) fn grid_segments(half_extent: f32, spacing: f32) -> Vec<(Vec3, Vec3)> {
    let mut segments = Vec::new();
    let steps = (half_extent / spacing).floor() as i32;
    for i in -steps..=steps {
        let d = i as f32 * spacing;
        segments.push((Vec3::new(d, 0.0, -half_extent), Vec3::new(d, 0.0, half_extent)));
        segments.push((Vec3::new(-half_extent, 0.0, d), Vec3::new(half_extent, 0.0, d)));
    }
    segments
}

/// X/Y/Z axis segments (RGB) of `length`, drawn in `transform`'s frame.
pub(crate) fn axes_segments(transform: Mat4, length: f32) -> [(Vec3, Vec3, Color); 3] {
    let o = transform.transform_point3(Vec3::ZERO);
    [
        (o, transform.transform_point3(Vec3::X * length), Color::new(0.9, 0.2, 0.2, 1.0)),
        (o, transform.transform_point3(Vec3::Y * length), Color::new(0.2, 0.9, 0.2, 1.0)),
        (o, transform.transform_point3(Vec3::Z * length), Color::new(0.25, 0.45, 1.0, 1.0)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_segment_count_and_extents() {
        // half_extent 2, spacing 1 → lines at -2,-1,0,1,2 in both
        // directions: 5 + 5 = 10 segments.
        let segs = grid_segments(2.0, 1.0);
        assert_eq!(segs.len(), 10);
        for (a, b) in &segs {
            assert_eq!(a.y, 0.0);
            assert_eq!(b.y, 0.0);
            assert!((*b - *a).length() > 3.9, "segments span the grid");
        }
    }

    #[test]
    fn axes_follow_the_transform() {
        let t = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let [(x0, x1, _), (y0, y1, _), (z0, z1, _)] = axes_segments(t, 2.0);
        let origin = Vec3::new(1.0, 2.0, 3.0);
        assert!((x0 - origin).length() < 1e-6);
        assert!((x1 - (origin + Vec3::X * 2.0)).length() < 1e-6);
        assert!((y0 - origin).length() < 1e-6);
        assert!((y1 - (origin + Vec3::Y * 2.0)).length() < 1e-6);
        assert!((z1 - (origin + Vec3::Z * 2.0)).length() < 1e-6);
        let _ = z0;
    }
}
