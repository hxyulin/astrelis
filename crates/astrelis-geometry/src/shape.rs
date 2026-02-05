//! High-level shape primitives.
//!
//! Shapes are convenient wrappers around paths for common geometric forms.

use crate::{Path, PathBuilder};
use glam::Vec2;

/// A high-level shape that can be converted to a path.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// A rectangle.
    Rect {
        /// Top-left position
        position: Vec2,
        /// Size (width, height)
        size: Vec2,
    },
    /// A rounded rectangle.
    RoundedRect {
        /// Top-left position
        position: Vec2,
        /// Size (width, height)
        size: Vec2,
        /// Corner radii [top-left, top-right, bottom-right, bottom-left]
        radii: [f32; 4],
    },
    /// A circle.
    Circle {
        /// Center point
        center: Vec2,
        /// Radius
        radius: f32,
    },
    /// An ellipse.
    Ellipse {
        /// Center point
        center: Vec2,
        /// Radii (x, y)
        radii: Vec2,
    },
    /// A line segment.
    Line {
        /// Start point
        start: Vec2,
        /// End point
        end: Vec2,
    },
    /// A polyline (connected line segments).
    Polyline {
        /// Points defining the polyline
        points: Vec<Vec2>,
        /// Whether to close the polyline into a polygon
        closed: bool,
    },
    /// A polygon (closed polyline).
    Polygon {
        /// Vertices of the polygon
        points: Vec<Vec2>,
    },
    /// A regular polygon with n sides.
    RegularPolygon {
        /// Center point
        center: Vec2,
        /// Radius (distance from center to vertices)
        radius: f32,
        /// Number of sides
        sides: u32,
        /// Rotation offset in radians
        rotation: f32,
    },
    /// A star shape.
    Star {
        /// Center point
        center: Vec2,
        /// Outer radius (tips)
        outer_radius: f32,
        /// Inner radius (valleys)
        inner_radius: f32,
        /// Number of points
        points: u32,
        /// Rotation offset in radians
        rotation: f32,
    },
    /// An arc (partial circle).
    Arc {
        /// Center point
        center: Vec2,
        /// Radius
        radius: f32,
        /// Start angle in radians
        start_angle: f32,
        /// End angle in radians
        end_angle: f32,
    },
    /// A pie/sector (arc with lines to center).
    Pie {
        /// Center point
        center: Vec2,
        /// Radius
        radius: f32,
        /// Start angle in radians
        start_angle: f32,
        /// End angle in radians
        end_angle: f32,
    },
    /// A custom path.
    Path(Path),
}

impl Shape {
    // =========================================================================
    // Constructors
    // =========================================================================

    /// Create a rectangle.
    pub fn rect(position: Vec2, size: Vec2) -> Self {
        Self::Rect { position, size }
    }

    /// Create a rectangle from center and size.
    pub fn rect_centered(center: Vec2, size: Vec2) -> Self {
        Self::Rect {
            position: center - size * 0.5,
            size,
        }
    }

    /// Create a rounded rectangle with uniform corner radius.
    pub fn rounded_rect(position: Vec2, size: Vec2, radius: f32) -> Self {
        Self::RoundedRect {
            position,
            size,
            radii: [radius; 4],
        }
    }

    /// Create a rounded rectangle with individual corner radii.
    pub fn rounded_rect_varying(position: Vec2, size: Vec2, radii: [f32; 4]) -> Self {
        Self::RoundedRect {
            position,
            size,
            radii,
        }
    }

    /// Create a circle.
    pub fn circle(center: Vec2, radius: f32) -> Self {
        Self::Circle { center, radius }
    }

    /// Create an ellipse.
    pub fn ellipse(center: Vec2, radii: Vec2) -> Self {
        Self::Ellipse { center, radii }
    }

    /// Create a line segment.
    pub fn line(start: Vec2, end: Vec2) -> Self {
        Self::Line { start, end }
    }

    /// Create a polyline.
    pub fn polyline(points: Vec<Vec2>, closed: bool) -> Self {
        Self::Polyline { points, closed }
    }

    /// Create a polygon.
    pub fn polygon(points: Vec<Vec2>) -> Self {
        Self::Polygon { points }
    }

    /// Create a regular polygon.
    pub fn regular_polygon(center: Vec2, radius: f32, sides: u32) -> Self {
        Self::RegularPolygon {
            center,
            radius,
            sides,
            rotation: 0.0,
        }
    }

    /// Create a regular polygon with rotation.
    pub fn regular_polygon_rotated(center: Vec2, radius: f32, sides: u32, rotation: f32) -> Self {
        Self::RegularPolygon {
            center,
            radius,
            sides,
            rotation,
        }
    }

    /// Create a star.
    pub fn star(center: Vec2, outer_radius: f32, inner_radius: f32, points: u32) -> Self {
        Self::Star {
            center,
            outer_radius,
            inner_radius,
            points,
            rotation: 0.0,
        }
    }

    /// Create a star with rotation.
    pub fn star_rotated(
        center: Vec2,
        outer_radius: f32,
        inner_radius: f32,
        points: u32,
        rotation: f32,
    ) -> Self {
        Self::Star {
            center,
            outer_radius,
            inner_radius,
            points,
            rotation,
        }
    }

    /// Create an arc.
    pub fn arc(center: Vec2, radius: f32, start_angle: f32, end_angle: f32) -> Self {
        Self::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        }
    }

    /// Create a pie/sector.
    pub fn pie(center: Vec2, radius: f32, start_angle: f32, end_angle: f32) -> Self {
        Self::Pie {
            center,
            radius,
            start_angle,
            end_angle,
        }
    }

    /// Create from a path.
    pub fn path(path: Path) -> Self {
        Self::Path(path)
    }

    // =========================================================================
    // Conversion
    // =========================================================================

    /// Convert this shape to a path.
    pub fn to_path(&self) -> Path {
        let mut builder = PathBuilder::new();

        match self {
            Shape::Rect { position, size } => {
                builder.rect(*position, *size);
            }

            Shape::RoundedRect {
                position,
                size,
                radii,
            } => {
                // Use the first radius for uniform (simplified)
                // TODO: Support varying radii per corner
                let r = radii[0].min(size.x / 2.0).min(size.y / 2.0);
                builder.rounded_rect(*position, *size, r);
            }

            Shape::Circle { center, radius } => {
                builder.circle(*center, *radius);
            }

            Shape::Ellipse { center, radii } => {
                builder.ellipse(*center, *radii);
            }

            Shape::Line { start, end } => {
                builder.move_to(*start);
                builder.line_to(*end);
            }

            Shape::Polyline { points, closed } => {
                if !points.is_empty() {
                    builder.move_to(points[0]);
                    for point in &points[1..] {
                        builder.line_to(*point);
                    }
                    if *closed {
                        builder.close();
                    }
                }
            }

            Shape::Polygon { points } => {
                builder.polygon(points);
            }

            Shape::RegularPolygon {
                center,
                radius,
                sides,
                rotation,
            } => {
                let points = generate_regular_polygon(*center, *radius, *sides, *rotation);
                builder.polygon(&points);
            }

            Shape::Star {
                center,
                outer_radius,
                inner_radius,
                points,
                rotation,
            } => {
                let star_points =
                    generate_star(*center, *outer_radius, *inner_radius, *points, *rotation);
                builder.polygon(&star_points);
            }

            Shape::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let arc_points = approximate_arc(*center, *radius, *start_angle, *end_angle, 32);
                if !arc_points.is_empty() {
                    builder.move_to(arc_points[0]);
                    for point in &arc_points[1..] {
                        builder.line_to(*point);
                    }
                }
            }

            Shape::Pie {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let arc_points = approximate_arc(*center, *radius, *start_angle, *end_angle, 32);
                builder.move_to(*center);
                if !arc_points.is_empty() {
                    builder.line_to(arc_points[0]);
                    for point in &arc_points[1..] {
                        builder.line_to(*point);
                    }
                }
                builder.close();
            }

            Shape::Path(path) => {
                return path.clone();
            }
        }

        builder.build()
    }

    /// Get the bounding box of this shape.
    pub fn bounds(&self) -> Option<(Vec2, Vec2)> {
        match self {
            Shape::Rect { position, size } => Some((*position, *position + *size)),

            Shape::RoundedRect { position, size, .. } => Some((*position, *position + *size)),

            Shape::Circle { center, radius } => {
                let r = Vec2::splat(*radius);
                Some((*center - r, *center + r))
            }

            Shape::Ellipse { center, radii } => Some((*center - *radii, *center + *radii)),

            Shape::Line { start, end } => Some((start.min(*end), start.max(*end))),

            Shape::Polyline { points, .. } | Shape::Polygon { points } => {
                if points.is_empty() {
                    return None;
                }
                let mut min = points[0];
                let mut max = points[0];
                for p in &points[1..] {
                    min = min.min(*p);
                    max = max.max(*p);
                }
                Some((min, max))
            }

            Shape::RegularPolygon { center, radius, .. } => {
                let r = Vec2::splat(*radius);
                Some((*center - r, *center + r))
            }

            Shape::Star {
                center,
                outer_radius,
                ..
            } => {
                let r = Vec2::splat(*outer_radius);
                Some((*center - r, *center + r))
            }

            Shape::Arc { center, radius, .. } | Shape::Pie { center, radius, .. } => {
                // Conservative bounds
                let r = Vec2::splat(*radius);
                Some((*center - r, *center + r))
            }

            Shape::Path(path) => path.bounds(),
        }
    }
}

/// Generate vertices for a regular polygon.
fn generate_regular_polygon(center: Vec2, radius: f32, sides: u32, rotation: f32) -> Vec<Vec2> {
    let mut points = Vec::with_capacity(sides as usize);
    let angle_step = std::f32::consts::TAU / sides as f32;

    for i in 0..sides {
        let angle = rotation + angle_step * i as f32;
        points.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
    }

    points
}

/// Generate vertices for a star.
fn generate_star(
    center: Vec2,
    outer_radius: f32,
    inner_radius: f32,
    points: u32,
    rotation: f32,
) -> Vec<Vec2> {
    let mut vertices = Vec::with_capacity(points as usize * 2);
    let angle_step = std::f32::consts::TAU / (points * 2) as f32;

    for i in 0..(points * 2) {
        let angle = rotation - std::f32::consts::FRAC_PI_2 + angle_step * i as f32;
        let radius = if i % 2 == 0 {
            outer_radius
        } else {
            inner_radius
        };
        vertices.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
    }

    vertices
}

/// Approximate an arc with line segments.
fn approximate_arc(
    center: Vec2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    segments: u32,
) -> Vec<Vec2> {
    let mut points = Vec::with_capacity(segments as usize + 1);
    let angle_span = end_angle - start_angle;
    let angle_step = angle_span / segments as f32;

    for i in 0..=segments {
        let angle = start_angle + angle_step * i as f32;
        points.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
    }

    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_bounds() {
        let shape = Shape::rect(Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0));
        let (min, max) = shape.bounds().unwrap();
        assert_eq!(min, Vec2::new(10.0, 20.0));
        assert_eq!(max, Vec2::new(110.0, 70.0));
    }

    #[test]
    fn test_circle_bounds() {
        let shape = Shape::circle(Vec2::new(50.0, 50.0), 25.0);
        let (min, max) = shape.bounds().unwrap();
        assert_eq!(min, Vec2::new(25.0, 25.0));
        assert_eq!(max, Vec2::new(75.0, 75.0));
    }

    #[test]
    fn test_rect_to_path() {
        let shape = Shape::rect(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
        let path = shape.to_path();
        assert!(!path.is_empty());
    }

    #[test]
    fn test_regular_polygon() {
        let points = generate_regular_polygon(Vec2::ZERO, 10.0, 4, 0.0);
        assert_eq!(points.len(), 4);
    }

    #[test]
    fn test_star() {
        let points = generate_star(Vec2::ZERO, 10.0, 5.0, 5, 0.0);
        assert_eq!(points.len(), 10);
    }
}
