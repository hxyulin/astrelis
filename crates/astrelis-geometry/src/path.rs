//! Path primitives for vector graphics.
//!
//! A path is a sequence of drawing commands that define a shape.

use crate::{CubicBezier, QuadraticBezier};
use glam::Vec2;

/// A command in a path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathCommand {
    /// Move to a new position without drawing.
    MoveTo(Vec2),
    /// Draw a line to a position.
    LineTo(Vec2),
    /// Draw a quadratic Bezier curve.
    QuadTo {
        /// Control point
        control: Vec2,
        /// End point
        to: Vec2,
    },
    /// Draw a cubic Bezier curve.
    CubicTo {
        /// First control point
        control1: Vec2,
        /// Second control point
        control2: Vec2,
        /// End point
        to: Vec2,
    },
    /// Draw an arc.
    ArcTo {
        /// Radii of the ellipse
        radii: Vec2,
        /// X-axis rotation in radians
        x_rotation: f32,
        /// Use large arc
        large_arc: bool,
        /// Sweep direction (clockwise if true)
        sweep: bool,
        /// End point
        to: Vec2,
    },
    /// Close the current sub-path by drawing a line to the start.
    Close,
}

/// A 2D path consisting of drawing commands.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Path {
    commands: Vec<PathCommand>,
}

impl Path {
    /// Create a new empty path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a path from a list of commands.
    pub fn from_commands(commands: Vec<PathCommand>) -> Self {
        Self { commands }
    }

    /// Get the commands in this path.
    pub fn commands(&self) -> &[PathCommand] {
        &self.commands
    }

    /// Check if the path is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Get the number of commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Get the bounding box of the path.
    ///
    /// Returns (min, max) corners.
    pub fn bounds(&self) -> Option<(Vec2, Vec2)> {
        if self.commands.is_empty() {
            return None;
        }

        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        let mut current = Vec2::ZERO;

        for cmd in &self.commands {
            match cmd {
                PathCommand::MoveTo(to) | PathCommand::LineTo(to) => {
                    min = min.min(*to);
                    max = max.max(*to);
                    current = *to;
                }
                PathCommand::QuadTo { control, to } => {
                    // Include control point for conservative bounds
                    min = min.min(*control).min(*to);
                    max = max.max(*control).max(*to);
                    current = *to;
                }
                PathCommand::CubicTo {
                    control1,
                    control2,
                    to,
                } => {
                    // Include control points for conservative bounds
                    min = min.min(*control1).min(*control2).min(*to);
                    max = max.max(*control1).max(*control2).max(*to);
                    current = *to;
                }
                PathCommand::ArcTo { to, radii, .. } => {
                    // Conservative bounds: include endpoint and radii
                    min = min.min(*to).min(current - *radii);
                    max = max.max(*to).max(current + *radii);
                    current = *to;
                }
                PathCommand::Close => {}
            }
        }

        if min.x.is_finite() && min.y.is_finite() && max.x.is_finite() && max.y.is_finite() {
            Some((min, max))
        } else {
            None
        }
    }

    /// Reverse the path direction.
    pub fn reverse(&self) -> Self {
        let mut reversed = Vec::new();
        let mut subpath_start = Vec2::ZERO;
        let mut current = Vec2::ZERO;
        let mut subpath_commands = Vec::new();

        for cmd in &self.commands {
            match cmd {
                PathCommand::MoveTo(to) => {
                    // Flush previous subpath
                    if !subpath_commands.is_empty() {
                        reversed.push(PathCommand::MoveTo(current));
                        for rcmd in subpath_commands.drain(..).rev() {
                            reversed.push(rcmd);
                        }
                    }
                    subpath_start = *to;
                    current = *to;
                }
                PathCommand::LineTo(to) => {
                    subpath_commands.push(PathCommand::LineTo(current));
                    current = *to;
                }
                PathCommand::QuadTo { control, to } => {
                    subpath_commands.push(PathCommand::QuadTo {
                        control: *control,
                        to: current,
                    });
                    current = *to;
                }
                PathCommand::CubicTo {
                    control1,
                    control2,
                    to,
                } => {
                    subpath_commands.push(PathCommand::CubicTo {
                        control1: *control2,
                        control2: *control1,
                        to: current,
                    });
                    current = *to;
                }
                PathCommand::ArcTo {
                    radii,
                    x_rotation,
                    large_arc,
                    sweep,
                    to,
                } => {
                    subpath_commands.push(PathCommand::ArcTo {
                        radii: *radii,
                        x_rotation: *x_rotation,
                        large_arc: *large_arc,
                        sweep: !sweep,
                        to: current,
                    });
                    current = *to;
                }
                PathCommand::Close => {
                    subpath_commands.push(PathCommand::LineTo(current));
                    current = subpath_start;
                }
            }
        }

        // Flush final subpath
        if !subpath_commands.is_empty() {
            reversed.push(PathCommand::MoveTo(current));
            for rcmd in subpath_commands.drain(..).rev() {
                reversed.push(rcmd);
            }
        }

        Self { commands: reversed }
    }
}

/// Builder for constructing paths.
#[derive(Debug, Default)]
pub struct PathBuilder {
    commands: Vec<PathCommand>,
    current_pos: Vec2,
    subpath_start: Vec2,
}

impl PathBuilder {
    /// Create a new path builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Move to a new position without drawing.
    pub fn move_to(&mut self, to: Vec2) -> &mut Self {
        self.commands.push(PathCommand::MoveTo(to));
        self.current_pos = to;
        self.subpath_start = to;
        self
    }

    /// Draw a line to a position.
    pub fn line_to(&mut self, to: Vec2) -> &mut Self {
        self.commands.push(PathCommand::LineTo(to));
        self.current_pos = to;
        self
    }

    /// Draw a horizontal line to x coordinate.
    pub fn horizontal_line_to(&mut self, x: f32) -> &mut Self {
        let to = Vec2::new(x, self.current_pos.y);
        self.line_to(to)
    }

    /// Draw a vertical line to y coordinate.
    pub fn vertical_line_to(&mut self, y: f32) -> &mut Self {
        let to = Vec2::new(self.current_pos.x, y);
        self.line_to(to)
    }

    /// Draw a quadratic Bezier curve.
    pub fn quad_to(&mut self, control: Vec2, to: Vec2) -> &mut Self {
        self.commands.push(PathCommand::QuadTo { control, to });
        self.current_pos = to;
        self
    }

    /// Draw a smooth quadratic Bezier (control point reflected from previous).
    pub fn smooth_quad_to(&mut self, to: Vec2) -> &mut Self {
        // Reflect previous control point
        let control = if let Some(PathCommand::QuadTo { control, to: prev }) =
            self.commands.last().copied()
        {
            prev * 2.0 - control
        } else {
            self.current_pos
        };
        self.quad_to(control, to)
    }

    /// Draw a cubic Bezier curve.
    pub fn cubic_to(&mut self, control1: Vec2, control2: Vec2, to: Vec2) -> &mut Self {
        self.commands.push(PathCommand::CubicTo {
            control1,
            control2,
            to,
        });
        self.current_pos = to;
        self
    }

    /// Draw a smooth cubic Bezier (first control point reflected from previous).
    pub fn smooth_cubic_to(&mut self, control2: Vec2, to: Vec2) -> &mut Self {
        // Reflect previous control2
        let control1 = if let Some(PathCommand::CubicTo {
            control2, to: prev, ..
        }) = self.commands.last().copied()
        {
            prev * 2.0 - control2
        } else {
            self.current_pos
        };
        self.cubic_to(control1, control2, to)
    }

    /// Draw an arc.
    pub fn arc_to(
        &mut self,
        radii: Vec2,
        x_rotation: f32,
        large_arc: bool,
        sweep: bool,
        to: Vec2,
    ) -> &mut Self {
        self.commands.push(PathCommand::ArcTo {
            radii,
            x_rotation,
            large_arc,
            sweep,
            to,
        });
        self.current_pos = to;
        self
    }

    /// Close the current sub-path.
    pub fn close(&mut self) -> &mut Self {
        self.commands.push(PathCommand::Close);
        self.current_pos = self.subpath_start;
        self
    }

    /// Add a rectangle to the path.
    pub fn rect(&mut self, position: Vec2, size: Vec2) -> &mut Self {
        self.move_to(position);
        self.line_to(position + Vec2::new(size.x, 0.0));
        self.line_to(position + size);
        self.line_to(position + Vec2::new(0.0, size.y));
        self.close()
    }

    /// Add a rounded rectangle to the path.
    pub fn rounded_rect(&mut self, position: Vec2, size: Vec2, radius: f32) -> &mut Self {
        let r = radius.min(size.x / 2.0).min(size.y / 2.0);
        let radii = Vec2::splat(r);

        // Start at top-left corner (after the curve)
        self.move_to(position + Vec2::new(r, 0.0));

        // Top edge
        self.line_to(position + Vec2::new(size.x - r, 0.0));
        // Top-right corner
        self.arc_to(radii, 0.0, false, true, position + Vec2::new(size.x, r));

        // Right edge
        self.line_to(position + Vec2::new(size.x, size.y - r));
        // Bottom-right corner
        self.arc_to(
            radii,
            0.0,
            false,
            true,
            position + Vec2::new(size.x - r, size.y),
        );

        // Bottom edge
        self.line_to(position + Vec2::new(r, size.y));
        // Bottom-left corner
        self.arc_to(
            radii,
            0.0,
            false,
            true,
            position + Vec2::new(0.0, size.y - r),
        );

        // Left edge
        self.line_to(position + Vec2::new(0.0, r));
        // Top-left corner
        self.arc_to(radii, 0.0, false, true, position + Vec2::new(r, 0.0));

        self.close()
    }

    /// Add a circle to the path.
    pub fn circle(&mut self, center: Vec2, radius: f32) -> &mut Self {
        let r = Vec2::splat(radius);

        // Start at rightmost point
        self.move_to(center + Vec2::new(radius, 0.0));

        // Draw four arcs
        self.arc_to(r, 0.0, false, true, center + Vec2::new(0.0, radius));
        self.arc_to(r, 0.0, false, true, center + Vec2::new(-radius, 0.0));
        self.arc_to(r, 0.0, false, true, center + Vec2::new(0.0, -radius));
        self.arc_to(r, 0.0, false, true, center + Vec2::new(radius, 0.0));

        self.close()
    }

    /// Add an ellipse to the path.
    pub fn ellipse(&mut self, center: Vec2, radii: Vec2) -> &mut Self {
        // Start at rightmost point
        self.move_to(center + Vec2::new(radii.x, 0.0));

        // Draw four arcs
        self.arc_to(radii, 0.0, false, true, center + Vec2::new(0.0, radii.y));
        self.arc_to(radii, 0.0, false, true, center + Vec2::new(-radii.x, 0.0));
        self.arc_to(radii, 0.0, false, true, center + Vec2::new(0.0, -radii.y));
        self.arc_to(radii, 0.0, false, true, center + Vec2::new(radii.x, 0.0));

        self.close()
    }

    /// Add a polygon to the path.
    pub fn polygon(&mut self, points: &[Vec2]) -> &mut Self {
        if points.is_empty() {
            return self;
        }

        self.move_to(points[0]);
        for point in &points[1..] {
            self.line_to(*point);
        }
        self.close()
    }

    /// Get the current position.
    pub fn current_pos(&self) -> Vec2 {
        self.current_pos
    }

    /// Build the path.
    pub fn build(self) -> Path {
        Path {
            commands: self.commands,
        }
    }
}

/// Extension trait for extracting curve segments from paths.
pub trait PathCurves {
    /// Iterator over quadratic curves in the path.
    fn quadratic_curves(&self) -> impl Iterator<Item = QuadraticBezier> + '_;
    /// Iterator over cubic curves in the path.
    fn cubic_curves(&self) -> impl Iterator<Item = CubicBezier> + '_;
}

impl PathCurves for Path {
    fn quadratic_curves(&self) -> impl Iterator<Item = QuadraticBezier> + '_ {
        let mut current = Vec2::ZERO;
        self.commands.iter().filter_map(move |cmd| match cmd {
            PathCommand::MoveTo(to) | PathCommand::LineTo(to) => {
                current = *to;
                None
            }
            PathCommand::QuadTo { control, to } => {
                let curve = QuadraticBezier::new(current, *control, *to);
                current = *to;
                Some(curve)
            }
            PathCommand::CubicTo { to, .. } => {
                current = *to;
                None
            }
            PathCommand::ArcTo { to, .. } => {
                current = *to;
                None
            }
            PathCommand::Close => None,
        })
    }

    fn cubic_curves(&self) -> impl Iterator<Item = CubicBezier> + '_ {
        let mut current = Vec2::ZERO;
        self.commands.iter().filter_map(move |cmd| match cmd {
            PathCommand::MoveTo(to) | PathCommand::LineTo(to) => {
                current = *to;
                None
            }
            PathCommand::QuadTo { to, .. } => {
                current = *to;
                None
            }
            PathCommand::CubicTo {
                control1,
                control2,
                to,
            } => {
                let curve = CubicBezier::new(current, *control1, *control2, *to);
                current = *to;
                Some(curve)
            }
            PathCommand::ArcTo { to, .. } => {
                current = *to;
                None
            }
            PathCommand::Close => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_builder_line() {
        let mut builder = PathBuilder::new();
        builder
            .move_to(Vec2::new(0.0, 0.0))
            .line_to(Vec2::new(100.0, 0.0))
            .line_to(Vec2::new(100.0, 100.0))
            .close();
        let path = builder.build();

        assert_eq!(path.len(), 4);
    }

    #[test]
    fn test_path_bounds() {
        let mut builder = PathBuilder::new();
        builder
            .move_to(Vec2::new(10.0, 20.0))
            .line_to(Vec2::new(100.0, 50.0))
            .line_to(Vec2::new(50.0, 100.0));
        let path = builder.build();

        let (min, max) = path.bounds().unwrap();
        assert_eq!(min, Vec2::new(10.0, 20.0));
        assert_eq!(max, Vec2::new(100.0, 100.0));
    }

    #[test]
    fn test_circle_path() {
        let mut builder = PathBuilder::new();
        builder.circle(Vec2::new(50.0, 50.0), 25.0);
        let path = builder.build();

        // Should have: move, 4 arcs, close
        assert!(!path.is_empty());
    }

    #[test]
    fn test_rect_path() {
        let mut builder = PathBuilder::new();
        builder.rect(Vec2::new(10.0, 10.0), Vec2::new(80.0, 60.0));
        let path = builder.build();

        let (min, max) = path.bounds().unwrap();
        assert_eq!(min, Vec2::new(10.0, 10.0));
        assert_eq!(max, Vec2::new(90.0, 70.0));
    }
}
