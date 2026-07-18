//! Fluent construction of [`LayoutStyle`] values.
//!
//! `astrelis-ui-core` exposes `LayoutStyle` as a fifteen-field struct, so call
//! sites reach for `LayoutStyle { grow: 1.0, ..Default::default() }` even to set
//! one property. [`LayoutExt`] adds chainable setters, and [`px`]/[`percent`]
//! shorten the `Length` constructors, so the same intent reads as
//! `layout().grow(1.0).width(px(200.0))`.

use astrelis_ui_core::{Alignment, Edges, LayoutStyle, Length, Positioning};

/// A logical-pixel [`Length`].
pub const fn px(value: f32) -> Length {
    Length::Px(value)
}

/// A fractional [`Length`] of the containing block (`1.0` is 100%).
pub const fn percent(value: f32) -> Length {
    Length::Percent(value)
}

/// A fresh default [`LayoutStyle`] to start a fluent chain from.
pub fn layout() -> LayoutStyle {
    LayoutStyle::default()
}

/// Chainable setters for [`LayoutStyle`].
///
/// Every method consumes and returns the style, so they compose into one
/// expression. They map one-to-one onto the struct fields; reach for the struct
/// literal directly when setting many fields at once is clearer.
pub trait LayoutExt {
    /// Sets the preferred width.
    #[must_use]
    fn width(self, width: Length) -> Self;
    /// Sets the preferred height.
    #[must_use]
    fn height(self, height: Length) -> Self;
    /// Sets the minimum width.
    #[must_use]
    fn min_width(self, width: Length) -> Self;
    /// Sets the minimum height.
    #[must_use]
    fn min_height(self, height: Length) -> Self;
    /// Sets the maximum width.
    #[must_use]
    fn max_width(self, width: Length) -> Self;
    /// Sets the maximum height.
    #[must_use]
    fn max_height(self, height: Length) -> Self;
    /// Sets the flex growth factor.
    #[must_use]
    fn grow(self, factor: f32) -> Self;
    /// Sets the flex shrink factor.
    #[must_use]
    fn shrink(self, factor: f32) -> Self;
    /// Sets the flex basis.
    #[must_use]
    fn basis(self, basis: Length) -> Self;
    /// Sets a uniform margin on every edge.
    #[must_use]
    fn margin(self, margin: Length) -> Self;
    /// Sets the per-edge margin.
    #[must_use]
    fn margins(self, margin: Edges<Length>) -> Self;
    /// Overrides the cross-axis alignment for this element.
    #[must_use]
    fn align_self(self, alignment: Alignment) -> Self;
    /// Switches between flow and absolute positioning.
    #[must_use]
    fn positioning(self, positioning: Positioning) -> Self;
    /// Constrains the width-to-height ratio.
    #[must_use]
    fn aspect_ratio(self, ratio: f32) -> Self;
}

impl LayoutExt for LayoutStyle {
    fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }
    fn height(mut self, height: Length) -> Self {
        self.height = height;
        self
    }
    fn min_width(mut self, width: Length) -> Self {
        self.min_width = width;
        self
    }
    fn min_height(mut self, height: Length) -> Self {
        self.min_height = height;
        self
    }
    fn max_width(mut self, width: Length) -> Self {
        self.max_width = width;
        self
    }
    fn max_height(mut self, height: Length) -> Self {
        self.max_height = height;
        self
    }
    fn grow(mut self, factor: f32) -> Self {
        self.grow = factor;
        self
    }
    fn shrink(mut self, factor: f32) -> Self {
        self.shrink = factor;
        self
    }
    fn basis(mut self, basis: Length) -> Self {
        self.basis = basis;
        self
    }
    fn margin(mut self, margin: Length) -> Self {
        self.margin = Edges::all(margin);
        self
    }
    fn margins(mut self, margin: Edges<Length>) -> Self {
        self.margin = margin;
        self
    }
    fn align_self(mut self, alignment: Alignment) -> Self {
        self.align_self = Some(alignment);
        self
    }
    fn positioning(mut self, positioning: Positioning) -> Self {
        self.positioning = positioning;
        self
    }
    fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.aspect_ratio = Some(ratio);
        self
    }
}
