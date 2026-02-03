//! Path generation and shape construction tests.
//!
//! These tests verify that the geometry API correctly constructs paths
//! for various shape operations.

use astrelis_geometry::{PathBuilder, Shape};
use glam::Vec2;

// ====================
// Shape API Tests
// ====================

#[test]
fn test_rect_shape_to_path() {
    let shape = Shape::rect(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
    let path = shape.to_path();

    assert!(!path.is_empty(), "Rectangle path should not be empty");
}

#[test]
fn test_circle_shape_to_path() {
    let shape = Shape::circle(Vec2::new(50.0, 50.0), 25.0);
    let path = shape.to_path();

    assert!(!path.is_empty(), "Circle path should not be empty");
}

#[test]
fn test_rounded_rect_shape_to_path() {
    let shape = Shape::rounded_rect(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0), 10.0);
    let path = shape.to_path();

    assert!(!path.is_empty(), "Rounded rectangle path should not be empty");
}

#[test]
fn test_ellipse_shape_to_path() {
    let shape = Shape::ellipse(Vec2::new(50.0, 50.0), Vec2::new(30.0, 20.0));
    let path = shape.to_path();

    assert!(!path.is_empty(), "Ellipse path should not be empty");
}

#[test]
fn test_line_shape_to_path() {
    let shape = Shape::line(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
    let path = shape.to_path();

    assert!(!path.is_empty(), "Line path should not be empty");
}

#[test]
fn test_rect_shape_factory_centered() {
    let shape = Shape::rect_centered(Vec2::new(50.0, 50.0), Vec2::new(100.0, 100.0));
    let path = shape.to_path();

    assert!(!path.is_empty(), "Centered rectangle path should not be empty");
}

#[test]
fn test_rounded_rect_varying_radii() {
    let shape = Shape::rounded_rect_varying(
        Vec2::new(0.0, 0.0),
        Vec2::new(100.0, 100.0),
        [5.0, 10.0, 15.0, 20.0],
    );
    let path = shape.to_path();

    assert!(
        !path.is_empty(),
        "Rounded rectangle with varying radii should not be empty"
    );
}

#[test]
fn test_polyline_shape() {
    let points = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(50.0, 25.0),
        Vec2::new(100.0, 0.0),
        Vec2::new(100.0, 100.0),
    ];

    let shape = Shape::polyline(points, false);
    let path = shape.to_path();

    assert!(!path.is_empty(), "Polyline path should not be empty");
}

#[test]
fn test_polygon_shape() {
    let points = vec![
        Vec2::new(50.0, 0.0),
        Vec2::new(100.0, 50.0),
        Vec2::new(50.0, 100.0),
        Vec2::new(0.0, 50.0),
    ];

    let shape = Shape::polygon(points);
    let path = shape.to_path();

    assert!(!path.is_empty(), "Polygon path should not be empty");
}

#[test]
fn test_regular_polygon() {
    let shape = Shape::regular_polygon(Vec2::new(50.0, 50.0), 30.0, 6);
    let path = shape.to_path();

    assert!(!path.is_empty(), "Regular polygon path should not be empty");
}

#[test]
fn test_star_shape() {
    let shape = Shape::star(Vec2::new(50.0, 50.0), 40.0, 20.0, 5);
    let path = shape.to_path();

    assert!(!path.is_empty(), "Star path should not be empty");
}

#[test]
fn test_arc_shape() {
    let shape = Shape::arc(Vec2::new(50.0, 50.0), 30.0, 0.0, std::f32::consts::PI);
    let path = shape.to_path();

    assert!(!path.is_empty(), "Arc path should not be empty");
}

#[test]
fn test_shape_conversions() {
    let shapes = vec![
        Shape::rect(Vec2::ZERO, Vec2::new(10.0, 10.0)),
        Shape::circle(Vec2::new(5.0, 5.0), 5.0),
        Shape::line(Vec2::ZERO, Vec2::new(10.0, 10.0)),
    ];

    for shape in shapes {
        let path = shape.to_path();
        assert!(!path.is_empty(), "Converted path should not be empty");
    }
}

// ====================
// PathBuilder Tests
// ====================

#[test]
fn test_empty_path() {
    let path = PathBuilder::new().build();

    assert!(path.is_empty(), "Empty path should be empty");
}

#[test]
fn test_path_builder_convenience_methods() {
    let mut builder = PathBuilder::new();
    builder.rect(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
    let path = builder.build();

    assert!(!path.is_empty(), "Rectangle builder path should not be empty");
}

#[test]
fn test_path_builder_circle_convenience() {
    let mut builder = PathBuilder::new();
    builder.circle(Vec2::new(50.0, 50.0), 25.0);
    let path = builder.build();

    assert!(!path.is_empty(), "Circle builder path should not be empty");
}

#[test]
fn test_path_builder_rounded_rect_convenience() {
    let mut builder = PathBuilder::new();
    builder.rounded_rect(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0), 10.0);
    let path = builder.build();

    assert!(
        !path.is_empty(),
        "Rounded rectangle builder path should not be empty"
    );
}

#[test]
fn test_path_builder_chaining() {
    let mut builder = PathBuilder::new();
    builder.rect(Vec2::new(0.0, 0.0), Vec2::new(50.0, 50.0));
    builder.circle(Vec2::new(75.0, 75.0), 25.0);
    let path = builder.build();

    assert!(!path.is_empty(), "Chained paths should not be empty");
}

#[test]
fn test_path_builder_manual_commands() {
    let mut builder = PathBuilder::new();
    builder.move_to(Vec2::new(0.0, 0.0));
    builder.line_to(Vec2::new(100.0, 0.0));
    builder.line_to(Vec2::new(100.0, 100.0));
    builder.line_to(Vec2::new(0.0, 100.0));
    builder.close();
    let path = builder.build();

    assert!(!path.is_empty(), "Manually built path should not be empty");
}

#[test]
fn test_path_builder_bezier_curves() {
    let mut builder = PathBuilder::new();
    builder.move_to(Vec2::new(0.0, 0.0));
    builder.quad_to(Vec2::new(50.0, 100.0), Vec2::new(100.0, 0.0));
    let path = builder.build();

    assert!(!path.is_empty(), "Bezier path should not be empty");
}

#[test]
fn test_path_builder_cubic_bezier() {
    let mut builder = PathBuilder::new();
    builder.move_to(Vec2::new(0.0, 0.0));
    builder.cubic_to(
        Vec2::new(25.0, 100.0),
        Vec2::new(75.0, 100.0),
        Vec2::new(100.0, 0.0),
    );
    let path = builder.build();

    assert!(!path.is_empty(), "Cubic bezier path should not be empty");
}

#[test]
fn test_path_builder_multiple_subpaths() {
    let mut builder = PathBuilder::new();
    builder.move_to(Vec2::new(0.0, 0.0));
    builder.line_to(Vec2::new(50.0, 0.0));
    builder.line_to(Vec2::new(50.0, 50.0));
    builder.close();
    builder.move_to(Vec2::new(100.0, 0.0));
    builder.line_to(Vec2::new(150.0, 0.0));
    builder.line_to(Vec2::new(150.0, 50.0));
    builder.close();
    let path = builder.build();

    assert!(!path.is_empty(), "Multiple subpaths should not be empty");
}

#[test]
fn test_path_builder_horizontal_line() {
    let mut builder = PathBuilder::new();
    builder.move_to(Vec2::new(0.0, 50.0));
    builder.horizontal_line_to(100.0);
    let path = builder.build();

    assert!(!path.is_empty(), "Horizontal line path should not be empty");
}

#[test]
fn test_path_builder_vertical_line() {
    let mut builder = PathBuilder::new();
    builder.move_to(Vec2::new(50.0, 0.0));
    builder.vertical_line_to(100.0);
    let path = builder.build();

    assert!(!path.is_empty(), "Vertical line path should not be empty");
}

#[test]
fn test_path_builder_arc() {
    let mut builder = PathBuilder::new();
    builder.move_to(Vec2::new(0.0, 0.0));
    builder.arc_to(
        Vec2::new(50.0, 50.0),
        0.0,
        false,
        true,
        Vec2::new(100.0, 0.0),
    );
    let path = builder.build();

    assert!(!path.is_empty(), "Arc path should not be empty");
}
