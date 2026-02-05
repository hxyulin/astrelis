//! Path tessellation using Lyon.
//!
//! Converts paths and shapes into triangle meshes for GPU rendering.

use crate::{
    FillRule, LineCap, LineJoin, Path, PathCommand, Shape, Stroke,
    vertex::{FillVertex, StrokeVertex, TessellatedMesh},
};
use glam::Vec2;
#[allow(unused_imports)]
use lyon::geom as lyon_geom;
use lyon::geom::{Arc, ArcFlags, SvgArc};
use lyon::lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex as LyonFillVertex, StrokeOptions,
    StrokeTessellator, StrokeVertex as LyonStrokeVertex, VertexBuffers,
};
use lyon::math::{Angle, Point, Vector};
use lyon::path::PathEvent;

/// Tessellator for converting paths to triangle meshes.
pub struct Tessellator {
    fill_tessellator: FillTessellator,
    stroke_tessellator: StrokeTessellator,
    /// Tolerance for curve flattening (smaller = more segments)
    pub tolerance: f32,
}

impl Default for Tessellator {
    fn default() -> Self {
        Self::new()
    }
}

impl Tessellator {
    /// Create a new tessellator with default settings.
    pub fn new() -> Self {
        Self {
            fill_tessellator: FillTessellator::new(),
            stroke_tessellator: StrokeTessellator::new(),
            tolerance: 0.5,
        }
    }

    /// Create a tessellator with custom tolerance.
    pub fn with_tolerance(tolerance: f32) -> Self {
        Self {
            fill_tessellator: FillTessellator::new(),
            stroke_tessellator: StrokeTessellator::new(),
            tolerance,
        }
    }

    /// Tessellate a path for filling.
    pub fn tessellate_fill(
        &mut self,
        path: &Path,
        fill_rule: FillRule,
    ) -> TessellatedMesh<FillVertex> {
        let mut buffers: VertexBuffers<FillVertex, u32> = VertexBuffers::new();

        let options = FillOptions::default()
            .with_tolerance(self.tolerance)
            .with_fill_rule(convert_fill_rule(fill_rule));

        let events = path_to_events(path);

        let result = self.fill_tessellator.tessellate(
            events,
            &options,
            &mut BuffersBuilder::new(&mut buffers, |vertex: LyonFillVertex| {
                FillVertex::new(vertex.position().x, vertex.position().y)
            }),
        );

        if result.is_err() {
            tracing::warn!("Fill tessellation failed");
            return TessellatedMesh::new();
        }

        TessellatedMesh::from_data(buffers.vertices, buffers.indices)
    }

    /// Tessellate a path for stroking.
    pub fn tessellate_stroke(
        &mut self,
        path: &Path,
        stroke: &Stroke,
    ) -> TessellatedMesh<StrokeVertex> {
        let mut buffers: VertexBuffers<StrokeVertex, u32> = VertexBuffers::new();

        let options = StrokeOptions::default()
            .with_tolerance(self.tolerance)
            .with_line_width(stroke.width)
            .with_line_cap(convert_line_cap(stroke.line_cap))
            .with_line_join(convert_line_join(stroke.line_join))
            .with_miter_limit(stroke.miter_limit);

        let events = path_to_events(path);

        let result = self.stroke_tessellator.tessellate(
            events,
            &options,
            &mut BuffersBuilder::new(&mut buffers, |vertex: LyonStrokeVertex| {
                StrokeVertex::new(
                    vertex.position().x,
                    vertex.position().y,
                    vertex.normal().x,
                    vertex.normal().y,
                    vertex.advancement(),
                    match vertex.side() {
                        lyon::lyon_tessellation::Side::Negative => -1.0,
                        lyon::lyon_tessellation::Side::Positive => 1.0,
                    },
                )
            }),
        );

        if result.is_err() {
            tracing::warn!("Stroke tessellation failed");
            return TessellatedMesh::new();
        }

        TessellatedMesh::from_data(buffers.vertices, buffers.indices)
    }

    /// Tessellate a shape for filling.
    pub fn tessellate_shape_fill(
        &mut self,
        shape: &Shape,
        fill_rule: FillRule,
    ) -> TessellatedMesh<FillVertex> {
        self.tessellate_fill(&shape.to_path(), fill_rule)
    }

    /// Tessellate a shape for stroking.
    pub fn tessellate_shape_stroke(
        &mut self,
        shape: &Shape,
        stroke: &Stroke,
    ) -> TessellatedMesh<StrokeVertex> {
        self.tessellate_stroke(&shape.to_path(), stroke)
    }

    /// Tessellate a simple filled rectangle (no curve flattening needed).
    pub fn tessellate_rect_fill(&self, position: Vec2, size: Vec2) -> TessellatedMesh<FillVertex> {
        let vertices = vec![
            FillVertex::new(position.x, position.y),
            FillVertex::new(position.x + size.x, position.y),
            FillVertex::new(position.x + size.x, position.y + size.y),
            FillVertex::new(position.x, position.y + size.y),
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        TessellatedMesh::from_data(vertices, indices)
    }

    /// Tessellate a line segment for stroking.
    pub fn tessellate_line(
        &self,
        start: Vec2,
        end: Vec2,
        width: f32,
    ) -> TessellatedMesh<FillVertex> {
        let dir = (end - start).normalize_or_zero();
        let normal = Vec2::new(-dir.y, dir.x) * (width * 0.5);

        let vertices = vec![
            FillVertex::new(start.x - normal.x, start.y - normal.y),
            FillVertex::new(start.x + normal.x, start.y + normal.y),
            FillVertex::new(end.x + normal.x, end.y + normal.y),
            FillVertex::new(end.x - normal.x, end.y - normal.y),
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        TessellatedMesh::from_data(vertices, indices)
    }
}

/// Convert our path to Lyon path events.
///
/// Lyon requires every subpath (starting with Begin) to have a matching End event.
/// This function handles both closed and open paths correctly.
fn path_to_events(path: &Path) -> Vec<PathEvent> {
    let mut events = Vec::new();
    let mut current = lyon::math::point(0.0, 0.0);
    let mut subpath_start = current;
    let mut in_subpath = false;

    for cmd in path.commands() {
        match cmd {
            PathCommand::MoveTo(to) => {
                // End previous subpath if we're in one (open path)
                if in_subpath {
                    events.push(PathEvent::End {
                        last: current,
                        first: subpath_start,
                        close: false,
                    });
                }

                current = lyon::math::point(to.x, to.y);
                subpath_start = current;
                events.push(PathEvent::Begin { at: current });
                in_subpath = true;
            }
            PathCommand::LineTo(to) => {
                let from = current;
                current = lyon::math::point(to.x, to.y);
                events.push(PathEvent::Line { from, to: current });
            }
            PathCommand::QuadTo { control, to } => {
                let from = current;
                let ctrl = lyon::math::point(control.x, control.y);
                current = lyon::math::point(to.x, to.y);
                events.push(PathEvent::Quadratic {
                    from,
                    ctrl,
                    to: current,
                });
            }
            PathCommand::CubicTo {
                control1,
                control2,
                to,
            } => {
                let from = current;
                let ctrl1 = lyon::math::point(control1.x, control1.y);
                let ctrl2 = lyon::math::point(control2.x, control2.y);
                current = lyon::math::point(to.x, to.y);
                events.push(PathEvent::Cubic {
                    from,
                    ctrl1,
                    ctrl2,
                    to: current,
                });
            }
            PathCommand::ArcTo {
                radii,
                x_rotation,
                large_arc,
                sweep,
                to,
            } => {
                // Convert SVG-style arc to Lyon's representation
                let from_point = Point::new(current.x, current.y);
                let to_point = Point::new(to.x, to.y);
                let radii_vec = Vector::new(radii.x, radii.y);

                let svg_arc = SvgArc {
                    from: from_point,
                    to: to_point,
                    radii: radii_vec,
                    x_rotation: Angle::radians(*x_rotation),
                    flags: ArcFlags {
                        large_arc: *large_arc,
                        sweep: *sweep,
                    },
                };

                // Convert the SVG arc to a center-parameterized arc
                let arc = svg_arc.to_arc();

                // Check if arc is degenerate (very small sweep angle)
                if arc.sweep_angle.radians.abs() < 0.001 {
                    // Degenerate arc, just draw a line
                    let from = current;
                    current = lyon::math::point(to.x, to.y);
                    events.push(PathEvent::Line { from, to: current });
                } else {
                    // Approximate the arc with a cubic bezier
                    // This is reasonable for small arcs (quarter circles)
                    let (ctrl1, ctrl2) = arc_to_cubic_control_points(&arc);
                    let from = current;
                    current = lyon::math::point(to.x, to.y);
                    events.push(PathEvent::Cubic {
                        from,
                        ctrl1: lyon::math::point(ctrl1.x, ctrl1.y),
                        ctrl2: lyon::math::point(ctrl2.x, ctrl2.y),
                        to: current,
                    });
                }
            }
            PathCommand::Close => {
                let last = current;
                current = subpath_start;
                events.push(PathEvent::End {
                    last,
                    first: subpath_start,
                    close: true,
                });
                in_subpath = false;
            }
        }
    }

    // End any remaining open subpath
    if in_subpath {
        events.push(PathEvent::End {
            last: current,
            first: subpath_start,
            close: false,
        });
    }

    events
}

/// Approximate an arc with a single cubic bezier curve.
/// Returns the two control points for the cubic bezier.
/// This works well for arcs that are quarter circles or smaller.
fn arc_to_cubic_control_points(arc: &Arc<f32>) -> (Vec2, Vec2) {
    // Use the standard approximation for circular arcs:
    // For an arc of angle θ, the optimal control point distance is:
    // k = (4/3) * tan(θ/4)
    let sweep_angle = arc.sweep_angle.radians.abs();

    // Handle full or near-full circles by clamping
    let sweep_angle = sweep_angle.min(std::f32::consts::FRAC_PI_2 * 0.99);

    let k = (4.0 / 3.0) * (sweep_angle / 4.0).tan();

    let start = arc.from();
    let end = arc.to();

    // Get the tangent vectors at start and end points
    let start_tangent = arc.sample_tangent(0.0);
    let end_tangent = arc.sample_tangent(1.0);

    // Calculate control points
    let ctrl1 = Vec2::new(
        start.x + k * start_tangent.x * arc.radii.x,
        start.y + k * start_tangent.y * arc.radii.y,
    );
    let ctrl2 = Vec2::new(
        end.x - k * end_tangent.x * arc.radii.x,
        end.y - k * end_tangent.y * arc.radii.y,
    );

    (ctrl1, ctrl2)
}

/// Convert our fill rule to Lyon's.
fn convert_fill_rule(rule: FillRule) -> lyon::lyon_tessellation::FillRule {
    match rule {
        FillRule::NonZero => lyon::lyon_tessellation::FillRule::NonZero,
        FillRule::EvenOdd => lyon::lyon_tessellation::FillRule::EvenOdd,
    }
}

/// Convert our line cap to Lyon's.
fn convert_line_cap(cap: LineCap) -> lyon::lyon_tessellation::LineCap {
    match cap {
        LineCap::Butt => lyon::lyon_tessellation::LineCap::Butt,
        LineCap::Round => lyon::lyon_tessellation::LineCap::Round,
        LineCap::Square => lyon::lyon_tessellation::LineCap::Square,
    }
}

/// Convert our line join to Lyon's.
fn convert_line_join(join: LineJoin) -> lyon::lyon_tessellation::LineJoin {
    match join {
        LineJoin::Miter => lyon::lyon_tessellation::LineJoin::Miter,
        LineJoin::Round => lyon::lyon_tessellation::LineJoin::Round,
        LineJoin::Bevel => lyon::lyon_tessellation::LineJoin::Bevel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PathBuilder;

    #[test]
    fn test_rect_tessellation() {
        let tessellator = Tessellator::new();
        let mesh = tessellator.tessellate_rect_fill(Vec2::ZERO, Vec2::new(100.0, 100.0));

        assert_eq!(mesh.vertex_count(), 4);
        assert_eq!(mesh.index_count(), 6);
        assert_eq!(mesh.triangle_count(), 2);
    }

    #[test]
    fn test_line_tessellation() {
        let tessellator = Tessellator::new();
        let mesh = tessellator.tessellate_line(Vec2::ZERO, Vec2::new(100.0, 0.0), 2.0);

        assert_eq!(mesh.vertex_count(), 4);
        assert_eq!(mesh.triangle_count(), 2);
    }

    #[test]
    fn test_path_fill_tessellation() {
        let mut tessellator = Tessellator::new();

        let mut builder = PathBuilder::new();
        builder.rect(Vec2::ZERO, Vec2::new(100.0, 100.0));
        let path = builder.build();

        let mesh = tessellator.tessellate_fill(&path, FillRule::NonZero);

        assert!(!mesh.is_empty());
    }

    #[test]
    fn test_circle_tessellation() {
        let mut tessellator = Tessellator::new();

        let shape = Shape::circle(Vec2::new(50.0, 50.0), 25.0);
        let mesh = tessellator.tessellate_shape_fill(&shape, FillRule::NonZero);

        assert!(!mesh.is_empty());
        // Circle should have many vertices due to curve flattening
        assert!(mesh.vertex_count() > 4);
    }
}
