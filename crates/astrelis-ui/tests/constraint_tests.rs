//! Constraint system tests for viewport units and expressions.

use astrelis_ui::constraint::Constraint;
use astrelis_ui::constraint_builder::{vw, vh};

#[test]
fn test_constraint_pixel() {
    let c = Constraint::Px(100.0);
    match c {
        Constraint::Px(val) => assert_eq!(val, 100.0),
        _ => panic!("Expected Px constraint"),
    }
}

#[test]
fn test_constraint_percent() {
    let c = Constraint::Percent(50.0);
    match c {
        Constraint::Percent(val) => assert_eq!(val, 50.0),
        _ => panic!("Expected Percent constraint"),
    }
}

#[test]
fn test_constraint_auto() {
    let c = Constraint::Auto;
    matches!(c, Constraint::Auto);
}

#[test]
fn test_viewport_width() {
    let c = Constraint::Vw(50.0);
    match c {
        Constraint::Vw(val) => assert_eq!(val, 50.0),
        _ => panic!("Expected Vw constraint"),
    }
}

#[test]
fn test_viewport_height() {
    let c = Constraint::Vh(100.0);
    match c {
        Constraint::Vh(val) => assert_eq!(val, 100.0),
        _ => panic!("Expected Vh constraint"),
    }
}

#[test]
fn test_constraint_clone() {
    let c1 = Constraint::Px(200.0);
    let c2 = c1.clone();

    match (c1, c2) {
        (Constraint::Px(v1), Constraint::Px(v2)) => assert_eq!(v1, v2),
        _ => panic!("Cloning failed"),
    }
}

#[test]
fn test_constraint_debug() {
    let c = Constraint::Px(150.0);
    let debug_str = format!("{:?}", c);
    assert!(debug_str.contains("Px"));
    assert!(debug_str.contains("150"));
}

#[test]
fn test_mixed_constraints() {
    let constraints = vec![
        Constraint::Px(100.0),
        Constraint::Percent(50.0),
        Constraint::Auto,
        Constraint::Vw(25.0),
        Constraint::Vh(75.0),
    ];

    assert_eq!(constraints.len(), 5);

    // Verify each type
    match constraints[0] {
        Constraint::Px(_) => {}
        _ => panic!("Expected Px"),
    }
    match constraints[1] {
        Constraint::Percent(_) => {}
        _ => panic!("Expected Percent"),
    }
    match constraints[2] {
        Constraint::Auto => {}
        _ => panic!("Expected Auto"),
    }
    match constraints[3] {
        Constraint::Vw(_) => {}
        _ => panic!("Expected Vw"),
    }
    match constraints[4] {
        Constraint::Vh(_) => {}
        _ => panic!("Expected Vh"),
    }
}
