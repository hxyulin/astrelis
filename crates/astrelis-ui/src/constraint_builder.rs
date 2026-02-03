//! Builder helpers for constraint expressions.
//!
//! This module provides ergonomic constructors for building complex constraint
//! expressions. Use these functions for cleaner, more readable constraint definitions.
//!
//! # Examples
//!
//! ```
//! use astrelis_ui::constraint_builder::*;
//!
//! // calc(100% - 40px)
//! let width = calc(percent(100.0) - px(40.0));
//!
//! // min(50%, 400px)
//! let min_width = min2(percent(50.0), px(400.0));
//!
//! // clamp(100px, 50%, 800px)
//! let clamped = clamp(px(100.0), percent(50.0), px(800.0));
//! ```

use crate::constraint::{CalcExpr, Constraint};

/// Create a pixel constraint.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::px;
/// let width = px(100.0);
/// ```
#[inline]
pub fn px(value: f32) -> Constraint {
    Constraint::Px(value)
}

/// Create a percentage constraint.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::percent;
/// let width = percent(50.0); // 50% of parent
/// ```
#[inline]
pub fn percent(value: f32) -> Constraint {
    Constraint::Percent(value)
}

/// Create a viewport width constraint.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::vw;
/// let width = vw(80.0); // 80% of viewport width
/// ```
#[inline]
pub fn vw(value: f32) -> Constraint {
    Constraint::Vw(value)
}

/// Create a viewport height constraint.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::vh;
/// let height = vh(60.0); // 60% of viewport height
/// ```
#[inline]
pub fn vh(value: f32) -> Constraint {
    Constraint::Vh(value)
}

/// Create a viewport min constraint.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::vmin;
/// let size = vmin(10.0); // 10% of smaller viewport dimension
/// ```
#[inline]
pub fn vmin(value: f32) -> Constraint {
    Constraint::Vmin(value)
}

/// Create a viewport max constraint.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::vmax;
/// let size = vmax(10.0); // 10% of larger viewport dimension
/// ```
#[inline]
pub fn vmax(value: f32) -> Constraint {
    Constraint::Vmax(value)
}

/// Create an auto constraint.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::auto;
/// let width = auto();
/// ```
#[inline]
pub fn auto() -> Constraint {
    Constraint::Auto
}

/// Create a calc expression constraint.
///
/// The expression is automatically simplified during construction.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::*;
///
/// // calc(100% - 40px)
/// let width = calc(percent(100.0) - px(40.0));
///
/// // calc(50vw + 20px)
/// let mixed = calc(vw(50.0) + px(20.0));
/// ```
pub fn calc(expr: CalcExpr) -> Constraint {
    Constraint::Calc(Box::new(expr.simplify()))
}

/// Create a minimum constraint from two values.
///
/// Returns the smaller of the two resolved values.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::*;
///
/// // min(50%, 400px)
/// let width = min2(percent(50.0), px(400.0));
/// ```
pub fn min2(a: Constraint, b: Constraint) -> Constraint {
    Constraint::Min(vec![a, b])
}

/// Create a minimum constraint from multiple values.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::*;
///
/// // min(30vw, 50%, 400px)
/// let width = min_of(vec![vw(30.0), percent(50.0), px(400.0)]);
/// ```
pub fn min_of(values: Vec<Constraint>) -> Constraint {
    Constraint::Min(values)
}

/// Create a maximum constraint from two values.
///
/// Returns the larger of the two resolved values.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::*;
///
/// // max(200px, 30%)
/// let width = max2(px(200.0), percent(30.0));
/// ```
pub fn max2(a: Constraint, b: Constraint) -> Constraint {
    Constraint::Max(vec![a, b])
}

/// Create a maximum constraint from multiple values.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::*;
///
/// // max(100px, 20vw, 30%)
/// let width = max_of(vec![px(100.0), vw(20.0), percent(30.0)]);
/// ```
pub fn max_of(values: Vec<Constraint>) -> Constraint {
    Constraint::Max(values)
}

/// Create a clamp constraint.
///
/// The value is clamped between min and max.
/// Equivalent to CSS `clamp(min, val, max)`.
///
/// # Examples
/// ```
/// use astrelis_ui::constraint_builder::*;
///
/// // clamp(100px, 50%, 800px)
/// // Width will be at least 100px, at most 800px, preferring 50%
/// let width = clamp(px(100.0), percent(50.0), px(800.0));
/// ```
pub fn clamp(min: Constraint, val: Constraint, max: Constraint) -> Constraint {
    Constraint::Clamp {
        min: Box::new(min),
        val: Box::new(val),
        max: Box::new(max),
    }
}

/// Extension trait for converting constraints to CalcExpr.
///
/// This enables cleaner calc expression building using operators.
pub trait IntoCalcExpr {
    /// Convert to a CalcExpr for use in calculations.
    fn into_expr(self) -> CalcExpr;
}

impl IntoCalcExpr for Constraint {
    fn into_expr(self) -> CalcExpr {
        CalcExpr::Value(self)
    }
}

impl IntoCalcExpr for f32 {
    fn into_expr(self) -> CalcExpr {
        CalcExpr::Value(Constraint::Px(self))
    }
}

// Operator overloading for Constraint to enable cleaner calc expressions

impl std::ops::Add<Constraint> for Constraint {
    type Output = CalcExpr;

    fn add(self, rhs: Constraint) -> CalcExpr {
        CalcExpr::Add(
            Box::new(CalcExpr::Value(self)),
            Box::new(CalcExpr::Value(rhs)),
        )
    }
}

impl std::ops::Sub<Constraint> for Constraint {
    type Output = CalcExpr;

    fn sub(self, rhs: Constraint) -> CalcExpr {
        CalcExpr::Sub(
            Box::new(CalcExpr::Value(self)),
            Box::new(CalcExpr::Value(rhs)),
        )
    }
}

impl std::ops::Mul<f32> for Constraint {
    type Output = CalcExpr;

    fn mul(self, rhs: f32) -> CalcExpr {
        CalcExpr::Mul(Box::new(CalcExpr::Value(self)), rhs)
    }
}

impl std::ops::Div<f32> for Constraint {
    type Output = CalcExpr;

    fn div(self, rhs: f32) -> CalcExpr {
        CalcExpr::Div(Box::new(CalcExpr::Value(self)), rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_constructors() {
        assert_eq!(px(100.0), Constraint::Px(100.0));
        assert_eq!(percent(50.0), Constraint::Percent(50.0));
        assert_eq!(vw(80.0), Constraint::Vw(80.0));
        assert_eq!(vh(60.0), Constraint::Vh(60.0));
        assert_eq!(vmin(10.0), Constraint::Vmin(10.0));
        assert_eq!(vmax(10.0), Constraint::Vmax(10.0));
        assert_eq!(auto(), Constraint::Auto);
    }

    #[test]
    fn test_calc_expr() {
        // calc(100% - 40px)
        let width = calc(percent(100.0) - px(40.0));
        match width {
            Constraint::Calc(expr) => match *expr {
                CalcExpr::Sub(lhs, rhs) => {
                    assert_eq!(*lhs, CalcExpr::Value(Constraint::Percent(100.0)));
                    assert_eq!(*rhs, CalcExpr::Value(Constraint::Px(40.0)));
                }
                _ => panic!("Expected Sub expression"),
            },
            _ => panic!("Expected Calc constraint"),
        }
    }

    #[test]
    fn test_min_max() {
        let min_width = min2(percent(50.0), px(400.0));
        match min_width {
            Constraint::Min(values) => {
                assert_eq!(values.len(), 2);
                assert_eq!(values[0], Constraint::Percent(50.0));
                assert_eq!(values[1], Constraint::Px(400.0));
            }
            _ => panic!("Expected Min constraint"),
        }

        let max_width = max2(px(200.0), percent(30.0));
        match max_width {
            Constraint::Max(values) => {
                assert_eq!(values.len(), 2);
                assert_eq!(values[0], Constraint::Px(200.0));
                assert_eq!(values[1], Constraint::Percent(30.0));
            }
            _ => panic!("Expected Max constraint"),
        }
    }

    #[test]
    fn test_clamp() {
        let width = clamp(px(100.0), percent(50.0), px(800.0));
        match width {
            Constraint::Clamp { min, val, max } => {
                assert_eq!(*min, Constraint::Px(100.0));
                assert_eq!(*val, Constraint::Percent(50.0));
                assert_eq!(*max, Constraint::Px(800.0));
            }
            _ => panic!("Expected Clamp constraint"),
        }
    }

    #[test]
    fn test_constraint_operators() {
        // Test add
        let add_expr = px(10.0) + px(20.0);
        let simplified = add_expr.simplify();
        assert_eq!(simplified, CalcExpr::Value(Constraint::Px(30.0)));

        // Test sub
        let sub_expr = px(50.0) - px(20.0);
        let simplified = sub_expr.simplify();
        assert_eq!(simplified, CalcExpr::Value(Constraint::Px(30.0)));

        // Test mul
        let mul_expr = px(10.0) * 3.0;
        let simplified = mul_expr.simplify();
        assert_eq!(simplified, CalcExpr::Value(Constraint::Px(30.0)));

        // Test div
        let div_expr = px(60.0) / 2.0;
        let simplified = div_expr.simplify();
        assert_eq!(simplified, CalcExpr::Value(Constraint::Px(30.0)));
    }
}
