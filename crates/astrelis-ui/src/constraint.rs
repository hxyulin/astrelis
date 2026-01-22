//! Advanced constraint expressions for responsive UI layouts.
//!
//! This module provides CSS-like constraint expressions:
//! - `calc()` - Arithmetic expressions like `calc(100% - 40px)`
//! - `min()` - Minimum value: `min(50%, 400px)`
//! - `max()` - Maximum value: `max(200px, 30%)`
//! - `clamp()` - Bounded value: `clamp(100px, 50%, 800px)`
//!
//! # Examples
//!
//! ```ignore
//! use astrelis_ui::constraint::{Constraint, CalcExpr};
//!
//! // calc(100% - 40px)
//! let width = Constraint::Calc(Box::new(
//!     CalcExpr::Sub(
//!         Box::new(CalcExpr::Value(Constraint::Percent(100.0))),
//!         Box::new(CalcExpr::Value(Constraint::Px(40.0))),
//!     )
//! ));
//!
//! // min(50%, 400px)
//! let min_width = Constraint::Min(vec![
//!     Constraint::Percent(50.0),
//!     Constraint::Px(400.0),
//! ]);
//!
//! // clamp(100px, 50%, 800px)
//! let clamped = Constraint::Clamp {
//!     min: Box::new(Constraint::Px(100.0)),
//!     val: Box::new(Constraint::Percent(50.0)),
//!     max: Box::new(Constraint::Px(800.0)),
//! };
//! ```

/// A constraint expression representing a responsive dimension value.
///
/// Constraints can be simple values (pixels, percentages, viewport units)
/// or complex expressions (calc, min, max, clamp).
#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    /// Fixed pixel value.
    Px(f32),

    /// Percentage of parent dimension.
    Percent(f32),

    /// Automatic sizing based on content.
    Auto,

    /// Percentage of viewport width.
    Vw(f32),

    /// Percentage of viewport height.
    Vh(f32),

    /// Percentage of minimum viewport dimension.
    Vmin(f32),

    /// Percentage of maximum viewport dimension.
    Vmax(f32),

    /// Calculated expression (arithmetic on other constraints).
    Calc(Box<CalcExpr>),

    /// Minimum of multiple constraints.
    Min(Vec<Constraint>),

    /// Maximum of multiple constraints.
    Max(Vec<Constraint>),

    /// Clamped value between min and max.
    Clamp {
        /// Minimum value.
        min: Box<Constraint>,
        /// Preferred value.
        val: Box<Constraint>,
        /// Maximum value.
        max: Box<Constraint>,
    },
}

impl Constraint {
    /// Create a pixel constraint.
    #[inline]
    pub fn px(value: f32) -> Self {
        Self::Px(value)
    }

    /// Create a percentage constraint.
    #[inline]
    pub fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    /// Create a viewport width constraint.
    #[inline]
    pub fn vw(value: f32) -> Self {
        Self::Vw(value)
    }

    /// Create a viewport height constraint.
    #[inline]
    pub fn vh(value: f32) -> Self {
        Self::Vh(value)
    }

    /// Create a viewport min constraint.
    #[inline]
    pub fn vmin(value: f32) -> Self {
        Self::Vmin(value)
    }

    /// Create a viewport max constraint.
    #[inline]
    pub fn vmax(value: f32) -> Self {
        Self::Vmax(value)
    }

    /// Create a calc expression constraint.
    pub fn calc(expr: CalcExpr) -> Self {
        Self::Calc(Box::new(expr.simplify()))
    }

    /// Create a minimum constraint.
    pub fn min(values: Vec<Constraint>) -> Self {
        debug_assert!(!values.is_empty(), "min() requires at least one value");
        Self::Min(values)
    }

    /// Create a maximum constraint.
    pub fn max(values: Vec<Constraint>) -> Self {
        debug_assert!(!values.is_empty(), "max() requires at least one value");
        Self::Max(values)
    }

    /// Create a clamp constraint.
    pub fn clamp(min: Constraint, val: Constraint, max: Constraint) -> Self {
        Self::Clamp {
            min: Box::new(min),
            val: Box::new(val),
            max: Box::new(max),
        }
    }

    /// Check if this constraint is a simple (non-expression) value.
    pub fn is_simple(&self) -> bool {
        matches!(
            self,
            Self::Px(_)
                | Self::Percent(_)
                | Self::Auto
                | Self::Vw(_)
                | Self::Vh(_)
                | Self::Vmin(_)
                | Self::Vmax(_)
        )
    }

    /// Check if this constraint contains any viewport units.
    pub fn has_viewport_units(&self) -> bool {
        match self {
            Self::Vw(_) | Self::Vh(_) | Self::Vmin(_) | Self::Vmax(_) => true,
            Self::Calc(expr) => expr.has_viewport_units(),
            Self::Min(values) | Self::Max(values) => {
                values.iter().any(|c| c.has_viewport_units())
            }
            Self::Clamp { min, val, max } => {
                min.has_viewport_units() || val.has_viewport_units() || max.has_viewport_units()
            }
            _ => false,
        }
    }

    /// Check if this constraint contains percentages (requires parent size).
    pub fn has_percentages(&self) -> bool {
        match self {
            Self::Percent(_) => true,
            Self::Calc(expr) => expr.has_percentages(),
            Self::Min(values) | Self::Max(values) => {
                values.iter().any(|c| c.has_percentages())
            }
            Self::Clamp { min, val, max } => {
                min.has_percentages() || val.has_percentages() || max.has_percentages()
            }
            _ => false,
        }
    }
}

impl Default for Constraint {
    fn default() -> Self {
        Self::Auto
    }
}

impl From<f32> for Constraint {
    fn from(value: f32) -> Self {
        Self::Px(value)
    }
}

// =============================================================================
// Length conversions (for backward compatibility)
// =============================================================================

impl From<crate::length::Length> for Constraint {
    fn from(length: crate::length::Length) -> Self {
        match length {
            crate::length::Length::Px(v) => Self::Px(v),
            crate::length::Length::Percent(v) => Self::Percent(v),
            crate::length::Length::Auto => Self::Auto,
            crate::length::Length::Vw(v) => Self::Vw(v),
            crate::length::Length::Vh(v) => Self::Vh(v),
            crate::length::Length::Vmin(v) => Self::Vmin(v),
            crate::length::Length::Vmax(v) => Self::Vmax(v),
        }
    }
}

impl From<crate::length::LengthAuto> for Constraint {
    fn from(length: crate::length::LengthAuto) -> Self {
        match length {
            crate::length::LengthAuto::Px(v) => Self::Px(v),
            crate::length::LengthAuto::Percent(v) => Self::Percent(v),
            crate::length::LengthAuto::Auto => Self::Auto,
            crate::length::LengthAuto::Vw(v) => Self::Vw(v),
            crate::length::LengthAuto::Vh(v) => Self::Vh(v),
            crate::length::LengthAuto::Vmin(v) => Self::Vmin(v),
            crate::length::LengthAuto::Vmax(v) => Self::Vmax(v),
        }
    }
}

impl From<crate::length::LengthPercentage> for Constraint {
    fn from(length: crate::length::LengthPercentage) -> Self {
        match length {
            crate::length::LengthPercentage::Px(v) => Self::Px(v),
            crate::length::LengthPercentage::Percent(v) => Self::Percent(v),
            crate::length::LengthPercentage::Vw(v) => Self::Vw(v),
            crate::length::LengthPercentage::Vh(v) => Self::Vh(v),
            crate::length::LengthPercentage::Vmin(v) => Self::Vmin(v),
            crate::length::LengthPercentage::Vmax(v) => Self::Vmax(v),
        }
    }
}

// =============================================================================
// Taffy conversions
// =============================================================================

impl Constraint {
    /// Convert to Taffy Dimension.
    ///
    /// # Note
    /// This only works for simple constraints (Px, Percent, Auto).
    /// For viewport units (Vw, Vh, Vmin, Vmax), you must resolve them first
    /// using `ConstraintResolver::resolve()` with a viewport context.
    /// For complex constraints (Calc, Min, Max, Clamp), you must resolve
    /// them first using `ConstraintResolver::resolve()`.
    ///
    /// # Panics
    /// Panics if called on viewport-relative units or complex constraints.
    /// Use `try_to_dimension()` for fallible conversion.
    pub fn to_dimension(&self) -> taffy::Dimension {
        match self {
            Constraint::Px(v) => taffy::Dimension::Length(*v),
            Constraint::Percent(v) => taffy::Dimension::Percent(*v / 100.0),
            Constraint::Auto => taffy::Dimension::Auto,
            Constraint::Vw(_) | Constraint::Vh(_) | Constraint::Vmin(_) | Constraint::Vmax(_) => {
                panic!(
                    "Viewport-relative constraints must be resolved to pixels before converting to Taffy dimension. \
                     Use ConstraintResolver::resolve() first."
                );
            }
            Constraint::Calc(_) | Constraint::Min(_) | Constraint::Max(_) | Constraint::Clamp { .. } => {
                panic!(
                    "Complex constraints (calc/min/max/clamp) must be resolved to pixels before converting to Taffy dimension. \
                     Use ConstraintResolver::resolve() first."
                );
            }
        }
    }

    /// Try to convert to Taffy Dimension.
    ///
    /// Returns `None` for viewport units and complex constraints that need resolution.
    pub fn try_to_dimension(&self) -> Option<taffy::Dimension> {
        match self {
            Constraint::Px(v) => Some(taffy::Dimension::Length(*v)),
            Constraint::Percent(v) => Some(taffy::Dimension::Percent(*v / 100.0)),
            Constraint::Auto => Some(taffy::Dimension::Auto),
            _ => None,
        }
    }

    /// Convert to Taffy LengthPercentageAuto.
    ///
    /// # Panics
    /// Panics if called on viewport-relative units or complex constraints.
    pub fn to_length_percentage_auto(&self) -> taffy::LengthPercentageAuto {
        match self {
            Constraint::Px(v) => taffy::LengthPercentageAuto::Length(*v),
            Constraint::Percent(v) => taffy::LengthPercentageAuto::Percent(*v / 100.0),
            Constraint::Auto => taffy::LengthPercentageAuto::Auto,
            Constraint::Vw(_) | Constraint::Vh(_) | Constraint::Vmin(_) | Constraint::Vmax(_) => {
                panic!(
                    "Viewport-relative constraints must be resolved to pixels first."
                );
            }
            Constraint::Calc(_) | Constraint::Min(_) | Constraint::Max(_) | Constraint::Clamp { .. } => {
                panic!(
                    "Complex constraints must be resolved to pixels first."
                );
            }
        }
    }

    /// Convert to Taffy LengthPercentage.
    ///
    /// # Panics
    /// Panics if called on Auto, viewport-relative units, or complex constraints.
    pub fn to_length_percentage(&self) -> taffy::LengthPercentage {
        match self {
            Constraint::Px(v) => taffy::LengthPercentage::Length(*v),
            Constraint::Percent(v) => taffy::LengthPercentage::Percent(*v / 100.0),
            Constraint::Auto => panic!("Auto is not valid for LengthPercentage"),
            Constraint::Vw(_) | Constraint::Vh(_) | Constraint::Vmin(_) | Constraint::Vmax(_) => {
                panic!(
                    "Viewport-relative constraints must be resolved to pixels first."
                );
            }
            Constraint::Calc(_) | Constraint::Min(_) | Constraint::Max(_) | Constraint::Clamp { .. } => {
                panic!(
                    "Complex constraints must be resolved to pixels first."
                );
            }
        }
    }
}

impl From<Constraint> for taffy::Dimension {
    fn from(constraint: Constraint) -> Self {
        constraint.to_dimension()
    }
}

/// A calculation expression AST node.
///
/// Used inside `Constraint::Calc` to represent arithmetic operations.
#[derive(Debug, Clone, PartialEq)]
pub enum CalcExpr {
    /// A terminal constraint value.
    Value(Constraint),

    /// Addition of two expressions.
    Add(Box<CalcExpr>, Box<CalcExpr>),

    /// Subtraction of two expressions.
    Sub(Box<CalcExpr>, Box<CalcExpr>),

    /// Multiplication by a scalar.
    Mul(Box<CalcExpr>, f32),

    /// Division by a scalar.
    Div(Box<CalcExpr>, f32),
}

impl CalcExpr {
    /// Create a value expression.
    pub fn value(constraint: Constraint) -> Self {
        Self::Value(constraint)
    }

    /// Simplify the expression by constant folding.
    ///
    /// This optimizes expressions like `px(10) + px(20)` to `px(30)`.
    pub fn simplify(self) -> Self {
        match self {
            // Add: try to fold if both sides are Px
            Self::Add(lhs, rhs) => {
                let lhs = lhs.simplify();
                let rhs = rhs.simplify();

                match (&lhs, &rhs) {
                    // px + px = px
                    (Self::Value(Constraint::Px(a)), Self::Value(Constraint::Px(b))) => {
                        Self::Value(Constraint::Px(a + b))
                    }
                    // 0 + x = x
                    (Self::Value(Constraint::Px(0.0)), _) => rhs,
                    // x + 0 = x
                    (_, Self::Value(Constraint::Px(0.0))) => lhs,
                    _ => Self::Add(Box::new(lhs), Box::new(rhs)),
                }
            }

            // Sub: try to fold if both sides are Px
            Self::Sub(lhs, rhs) => {
                let lhs = lhs.simplify();
                let rhs = rhs.simplify();

                match (&lhs, &rhs) {
                    // px - px = px
                    (Self::Value(Constraint::Px(a)), Self::Value(Constraint::Px(b))) => {
                        Self::Value(Constraint::Px(a - b))
                    }
                    // x - 0 = x
                    (_, Self::Value(Constraint::Px(0.0))) => lhs,
                    _ => Self::Sub(Box::new(lhs), Box::new(rhs)),
                }
            }

            // Mul: fold scalar multiplication
            Self::Mul(expr, scalar) => {
                let expr = expr.simplify();

                match &expr {
                    // px * scalar = px
                    Self::Value(Constraint::Px(v)) => {
                        Self::Value(Constraint::Px(v * scalar))
                    }
                    // x * 1 = x
                    _ if (scalar - 1.0).abs() < f32::EPSILON => expr,
                    // x * 0 = 0
                    _ if scalar.abs() < f32::EPSILON => Self::Value(Constraint::Px(0.0)),
                    _ => Self::Mul(Box::new(expr), scalar),
                }
            }

            // Div: fold scalar division
            Self::Div(expr, scalar) => {
                let expr = expr.simplify();

                match &expr {
                    // px / scalar = px
                    Self::Value(Constraint::Px(v)) => {
                        Self::Value(Constraint::Px(v / scalar))
                    }
                    // x / 1 = x
                    _ if (scalar - 1.0).abs() < f32::EPSILON => expr,
                    _ => Self::Div(Box::new(expr), scalar),
                }
            }

            // Value: no simplification needed
            Self::Value(c) => Self::Value(c),
        }
    }

    /// Check if this expression contains any viewport units.
    pub fn has_viewport_units(&self) -> bool {
        match self {
            Self::Value(c) => c.has_viewport_units(),
            Self::Add(lhs, rhs) | Self::Sub(lhs, rhs) => {
                lhs.has_viewport_units() || rhs.has_viewport_units()
            }
            Self::Mul(expr, _) | Self::Div(expr, _) => expr.has_viewport_units(),
        }
    }

    /// Check if this expression contains percentages.
    pub fn has_percentages(&self) -> bool {
        match self {
            Self::Value(c) => c.has_percentages(),
            Self::Add(lhs, rhs) | Self::Sub(lhs, rhs) => {
                lhs.has_percentages() || rhs.has_percentages()
            }
            Self::Mul(expr, _) | Self::Div(expr, _) => expr.has_percentages(),
        }
    }
}

impl From<Constraint> for CalcExpr {
    fn from(constraint: Constraint) -> Self {
        Self::Value(constraint)
    }
}

impl From<f32> for CalcExpr {
    fn from(value: f32) -> Self {
        Self::Value(Constraint::Px(value))
    }
}

// Operator implementations for ergonomic calc expressions

impl std::ops::Add for CalcExpr {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Add(Box::new(self), Box::new(rhs))
    }
}

impl std::ops::Sub for CalcExpr {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::Sub(Box::new(self), Box::new(rhs))
    }
}

impl std::ops::Mul<f32> for CalcExpr {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::Mul(Box::new(self), rhs)
    }
}

impl std::ops::Div<f32> for CalcExpr {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::Div(Box::new(self), rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_constructors() {
        assert_eq!(Constraint::px(100.0), Constraint::Px(100.0));
        assert_eq!(Constraint::percent(50.0), Constraint::Percent(50.0));
        assert_eq!(Constraint::vw(80.0), Constraint::Vw(80.0));
        assert_eq!(Constraint::vh(60.0), Constraint::Vh(60.0));
    }

    #[test]
    fn test_constraint_is_simple() {
        assert!(Constraint::Px(100.0).is_simple());
        assert!(Constraint::Percent(50.0).is_simple());
        assert!(Constraint::Auto.is_simple());
        assert!(Constraint::Vw(50.0).is_simple());

        let calc = Constraint::calc(CalcExpr::Value(Constraint::Px(100.0)));
        assert!(!calc.is_simple());

        let min = Constraint::min(vec![Constraint::Px(100.0)]);
        assert!(!min.is_simple());
    }

    #[test]
    fn test_constraint_has_viewport_units() {
        assert!(!Constraint::Px(100.0).has_viewport_units());
        assert!(!Constraint::Percent(50.0).has_viewport_units());
        assert!(Constraint::Vw(50.0).has_viewport_units());
        assert!(Constraint::Vh(50.0).has_viewport_units());
        assert!(Constraint::Vmin(50.0).has_viewport_units());
        assert!(Constraint::Vmax(50.0).has_viewport_units());

        let calc = Constraint::calc(CalcExpr::Value(Constraint::Vw(50.0)));
        assert!(calc.has_viewport_units());

        let min = Constraint::min(vec![Constraint::Px(100.0), Constraint::Vw(50.0)]);
        assert!(min.has_viewport_units());
    }

    #[test]
    fn test_calc_expr_simplify_add() {
        // px + px = px
        let expr = CalcExpr::Add(
            Box::new(CalcExpr::Value(Constraint::Px(10.0))),
            Box::new(CalcExpr::Value(Constraint::Px(20.0))),
        );
        assert_eq!(expr.simplify(), CalcExpr::Value(Constraint::Px(30.0)));

        // 0 + x = x
        let expr = CalcExpr::Add(
            Box::new(CalcExpr::Value(Constraint::Px(0.0))),
            Box::new(CalcExpr::Value(Constraint::Percent(50.0))),
        );
        assert_eq!(expr.simplify(), CalcExpr::Value(Constraint::Percent(50.0)));
    }

    #[test]
    fn test_calc_expr_simplify_sub() {
        // px - px = px
        let expr = CalcExpr::Sub(
            Box::new(CalcExpr::Value(Constraint::Px(30.0))),
            Box::new(CalcExpr::Value(Constraint::Px(10.0))),
        );
        assert_eq!(expr.simplify(), CalcExpr::Value(Constraint::Px(20.0)));
    }

    #[test]
    fn test_calc_expr_simplify_mul() {
        // px * scalar = px
        let expr = CalcExpr::Mul(Box::new(CalcExpr::Value(Constraint::Px(10.0))), 3.0);
        assert_eq!(expr.simplify(), CalcExpr::Value(Constraint::Px(30.0)));

        // x * 1 = x
        let expr = CalcExpr::Mul(Box::new(CalcExpr::Value(Constraint::Percent(50.0))), 1.0);
        assert_eq!(expr.simplify(), CalcExpr::Value(Constraint::Percent(50.0)));
    }

    #[test]
    fn test_calc_expr_operators() {
        let a = CalcExpr::from(Constraint::Percent(100.0));
        let b = CalcExpr::from(Constraint::Px(40.0));

        // calc(100% - 40px)
        let result = (a - b).simplify();
        match result {
            CalcExpr::Sub(lhs, rhs) => {
                assert_eq!(*lhs, CalcExpr::Value(Constraint::Percent(100.0)));
                assert_eq!(*rhs, CalcExpr::Value(Constraint::Px(40.0)));
            }
            _ => panic!("Expected Sub expression"),
        }
    }
}
