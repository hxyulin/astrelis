//! Constraint resolution logic for computing final pixel values.
//!
//! This module provides resolution of constraint expressions to concrete pixel values
//! given a resolution context (viewport size, parent dimensions).
//!
//! # Examples
//!
//! ```
//! use astrelis_ui::constraint::{Constraint, CalcExpr};
//! use astrelis_ui::constraint_resolver::{ConstraintResolver, ResolveContext};
//! use astrelis_core::math::Vec2;
//!
//! let viewport = Vec2::new(1280.0, 720.0);
//! let ctx = ResolveContext::new(viewport, Some(640.0)); // 640px parent width
//!
//! // Resolve 50% of parent
//! let width = ConstraintResolver::resolve(&Constraint::Percent(50.0), &ctx);
//! assert_eq!(width, Some(320.0)); // 50% of 640px
//!
//! // Resolve viewport unit
//! let vw_width = ConstraintResolver::resolve(&Constraint::Vw(80.0), &ctx);
//! assert_eq!(vw_width, Some(1024.0)); // 80% of 1280px
//! ```

use astrelis_core::math::Vec2;
use crate::constraint::{CalcExpr, Constraint};

/// Context for resolving constraints.
///
/// Contains the information needed to resolve viewport units and percentages.
#[derive(Debug, Clone, Copy)]
pub struct ResolveContext {
    /// Viewport dimensions in pixels.
    pub viewport_size: Vec2,

    /// Parent dimension in pixels (for percentage calculations).
    /// `None` if no parent context is available.
    pub parent_size: Option<f32>,
}

impl ResolveContext {
    /// Create a new resolution context.
    ///
    /// # Arguments
    /// * `viewport_size` - The viewport dimensions (width, height) in pixels
    /// * `parent_size` - The parent dimension for percentage calculations, or None
    pub fn new(viewport_size: Vec2, parent_size: Option<f32>) -> Self {
        Self {
            viewport_size,
            parent_size,
        }
    }

    /// Create a context with only viewport size (no parent for percentages).
    pub fn viewport_only(viewport_size: Vec2) -> Self {
        Self {
            viewport_size,
            parent_size: None,
        }
    }

    /// Create a width-resolving context from viewport and parent width.
    pub fn for_width(viewport_size: Vec2, parent_width: Option<f32>) -> Self {
        Self {
            viewport_size,
            parent_size: parent_width,
        }
    }

    /// Create a height-resolving context from viewport and parent height.
    pub fn for_height(viewport_size: Vec2, parent_height: Option<f32>) -> Self {
        Self {
            viewport_size,
            parent_size: parent_height,
        }
    }
}

impl Default for ResolveContext {
    fn default() -> Self {
        Self {
            viewport_size: Vec2::new(800.0, 600.0),
            parent_size: None,
        }
    }
}

/// Constraint resolver for computing final pixel values.
pub struct ConstraintResolver;

impl ConstraintResolver {
    /// Resolve a constraint to a concrete pixel value.
    ///
    /// Returns `None` if the constraint cannot be resolved:
    /// - `Auto` constraints return `None` (requires layout algorithm)
    /// - `Percent` without parent size returns `None`
    ///
    /// # Arguments
    /// * `constraint` - The constraint to resolve
    /// * `ctx` - The resolution context
    ///
    /// # Returns
    /// The resolved pixel value, or `None` if unresolvable.
    pub fn resolve(constraint: &Constraint, ctx: &ResolveContext) -> Option<f32> {
        match constraint {
            Constraint::Px(v) => Some(*v),

            Constraint::Percent(p) => {
                ctx.parent_size.map(|parent| parent * p / 100.0)
            }

            Constraint::Auto => None, // Auto requires layout algorithm

            Constraint::Vw(v) => Some(v * ctx.viewport_size.x / 100.0),

            Constraint::Vh(v) => Some(v * ctx.viewport_size.y / 100.0),

            Constraint::Vmin(v) => {
                let min = ctx.viewport_size.x.min(ctx.viewport_size.y);
                Some(v * min / 100.0)
            }

            Constraint::Vmax(v) => {
                let max = ctx.viewport_size.x.max(ctx.viewport_size.y);
                Some(v * max / 100.0)
            }

            Constraint::Calc(expr) => Self::resolve_calc(expr, ctx),

            Constraint::Min(values) => {
                let resolved: Vec<f32> = values
                    .iter()
                    .filter_map(|c| Self::resolve(c, ctx))
                    .collect();

                if resolved.is_empty() {
                    None
                } else {
                    Some(resolved.into_iter().fold(f32::INFINITY, f32::min))
                }
            }

            Constraint::Max(values) => {
                let resolved: Vec<f32> = values
                    .iter()
                    .filter_map(|c| Self::resolve(c, ctx))
                    .collect();

                if resolved.is_empty() {
                    None
                } else {
                    Some(resolved.into_iter().fold(f32::NEG_INFINITY, f32::max))
                }
            }

            Constraint::Clamp { min, val, max } => {
                let min_val = Self::resolve(min, ctx)?;
                let val_val = Self::resolve(val, ctx)?;
                let max_val = Self::resolve(max, ctx)?;

                Some(val_val.clamp(min_val, max_val))
            }
        }
    }

    /// Resolve a calc expression.
    fn resolve_calc(expr: &CalcExpr, ctx: &ResolveContext) -> Option<f32> {
        match expr {
            CalcExpr::Value(c) => Self::resolve(c, ctx),

            CalcExpr::Add(lhs, rhs) => {
                let lhs_val = Self::resolve_calc(lhs, ctx)?;
                let rhs_val = Self::resolve_calc(rhs, ctx)?;
                Some(lhs_val + rhs_val)
            }

            CalcExpr::Sub(lhs, rhs) => {
                let lhs_val = Self::resolve_calc(lhs, ctx)?;
                let rhs_val = Self::resolve_calc(rhs, ctx)?;
                Some(lhs_val - rhs_val)
            }

            CalcExpr::Mul(expr, scalar) => {
                let expr_val = Self::resolve_calc(expr, ctx)?;
                Some(expr_val * scalar)
            }

            CalcExpr::Div(expr, scalar) => {
                let expr_val = Self::resolve_calc(expr, ctx)?;
                Some(expr_val / scalar)
            }
        }
    }

    /// Resolve a constraint, returning a default value for unresolvable constraints.
    ///
    /// This is useful when a fallback is acceptable (e.g., 0.0 for Auto).
    ///
    /// # Arguments
    /// * `constraint` - The constraint to resolve
    /// * `ctx` - The resolution context
    /// * `default` - The default value if resolution fails
    pub fn resolve_or(constraint: &Constraint, ctx: &ResolveContext, default: f32) -> f32 {
        Self::resolve(constraint, ctx).unwrap_or(default)
    }

    /// Resolve a constraint, treating `Auto` as a specific value.
    ///
    /// # Arguments
    /// * `constraint` - The constraint to resolve
    /// * `ctx` - The resolution context
    /// * `auto_value` - The value to use for Auto constraints
    pub fn resolve_with_auto(
        constraint: &Constraint,
        ctx: &ResolveContext,
        auto_value: f32,
    ) -> Option<f32> {
        if matches!(constraint, Constraint::Auto) {
            Some(auto_value)
        } else {
            Self::resolve(constraint, ctx)
        }
    }

    /// Check if a constraint can be resolved with the given context.
    ///
    /// This is useful for early validation before attempting resolution.
    pub fn can_resolve(constraint: &Constraint, ctx: &ResolveContext) -> bool {
        match constraint {
            Constraint::Auto => false,
            Constraint::Percent(_) => ctx.parent_size.is_some(),
            Constraint::Calc(expr) => Self::can_resolve_calc(expr, ctx),
            Constraint::Min(values) | Constraint::Max(values) => {
                values.iter().any(|c| Self::can_resolve(c, ctx))
            }
            Constraint::Clamp { min, val, max } => {
                Self::can_resolve(min, ctx)
                    && Self::can_resolve(val, ctx)
                    && Self::can_resolve(max, ctx)
            }
            _ => true, // Px and viewport units are always resolvable
        }
    }

    fn can_resolve_calc(expr: &CalcExpr, ctx: &ResolveContext) -> bool {
        match expr {
            CalcExpr::Value(c) => Self::can_resolve(c, ctx),
            CalcExpr::Add(lhs, rhs) | CalcExpr::Sub(lhs, rhs) => {
                Self::can_resolve_calc(lhs, ctx) && Self::can_resolve_calc(rhs, ctx)
            }
            CalcExpr::Mul(expr, _) | CalcExpr::Div(expr, _) => Self::can_resolve_calc(expr, ctx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> ResolveContext {
        ResolveContext::new(Vec2::new(1280.0, 720.0), Some(640.0))
    }

    #[test]
    fn test_resolve_px() {
        let ctx = test_ctx();
        assert_eq!(ConstraintResolver::resolve(&Constraint::Px(100.0), &ctx), Some(100.0));
    }

    #[test]
    fn test_resolve_percent() {
        let ctx = test_ctx();
        assert_eq!(
            ConstraintResolver::resolve(&Constraint::Percent(50.0), &ctx),
            Some(320.0) // 50% of 640
        );

        // Without parent size
        let ctx_no_parent = ResolveContext::viewport_only(Vec2::new(1280.0, 720.0));
        assert_eq!(
            ConstraintResolver::resolve(&Constraint::Percent(50.0), &ctx_no_parent),
            None
        );
    }

    #[test]
    fn test_resolve_viewport_units() {
        let ctx = test_ctx();

        assert_eq!(
            ConstraintResolver::resolve(&Constraint::Vw(80.0), &ctx),
            Some(1024.0) // 80% of 1280
        );

        assert_eq!(
            ConstraintResolver::resolve(&Constraint::Vh(50.0), &ctx),
            Some(360.0) // 50% of 720
        );

        assert_eq!(
            ConstraintResolver::resolve(&Constraint::Vmin(10.0), &ctx),
            Some(72.0) // 10% of 720 (min)
        );

        assert_eq!(
            ConstraintResolver::resolve(&Constraint::Vmax(10.0), &ctx),
            Some(128.0) // 10% of 1280 (max)
        );
    }

    #[test]
    fn test_resolve_auto() {
        let ctx = test_ctx();
        assert_eq!(ConstraintResolver::resolve(&Constraint::Auto, &ctx), None);
    }

    #[test]
    fn test_resolve_calc() {
        let ctx = test_ctx();

        // calc(100% - 40px)
        let constraint = Constraint::Calc(Box::new(CalcExpr::Sub(
            Box::new(CalcExpr::Value(Constraint::Percent(100.0))),
            Box::new(CalcExpr::Value(Constraint::Px(40.0))),
        )));

        assert_eq!(
            ConstraintResolver::resolve(&constraint, &ctx),
            Some(600.0) // 640 - 40
        );
    }

    #[test]
    fn test_resolve_min() {
        let ctx = test_ctx();

        // min(50%, 400px) with parent 640px
        let constraint = Constraint::Min(vec![
            Constraint::Percent(50.0), // 320px
            Constraint::Px(400.0),
        ]);

        assert_eq!(
            ConstraintResolver::resolve(&constraint, &ctx),
            Some(320.0) // min(320, 400)
        );
    }

    #[test]
    fn test_resolve_max() {
        let ctx = test_ctx();

        // max(50%, 400px) with parent 640px
        let constraint = Constraint::Max(vec![
            Constraint::Percent(50.0), // 320px
            Constraint::Px(400.0),
        ]);

        assert_eq!(
            ConstraintResolver::resolve(&constraint, &ctx),
            Some(400.0) // max(320, 400)
        );
    }

    #[test]
    fn test_resolve_clamp() {
        let ctx = test_ctx();

        // clamp(100px, 50%, 200px) with parent 640px
        // 50% = 320px, clamped to [100, 200] = 200px
        let constraint = Constraint::Clamp {
            min: Box::new(Constraint::Px(100.0)),
            val: Box::new(Constraint::Percent(50.0)),
            max: Box::new(Constraint::Px(200.0)),
        };

        assert_eq!(
            ConstraintResolver::resolve(&constraint, &ctx),
            Some(200.0)
        );

        // clamp(100px, 10%, 400px) with parent 640px
        // 10% = 64px, clamped to [100, 400] = 100px
        let constraint = Constraint::Clamp {
            min: Box::new(Constraint::Px(100.0)),
            val: Box::new(Constraint::Percent(10.0)),
            max: Box::new(Constraint::Px(400.0)),
        };

        assert_eq!(
            ConstraintResolver::resolve(&constraint, &ctx),
            Some(100.0)
        );
    }

    #[test]
    fn test_can_resolve() {
        let ctx = test_ctx();
        let ctx_no_parent = ResolveContext::viewport_only(Vec2::new(1280.0, 720.0));

        assert!(ConstraintResolver::can_resolve(&Constraint::Px(100.0), &ctx));
        assert!(ConstraintResolver::can_resolve(&Constraint::Vw(50.0), &ctx));
        assert!(ConstraintResolver::can_resolve(&Constraint::Percent(50.0), &ctx));
        assert!(!ConstraintResolver::can_resolve(&Constraint::Percent(50.0), &ctx_no_parent));
        assert!(!ConstraintResolver::can_resolve(&Constraint::Auto, &ctx));
    }
}
