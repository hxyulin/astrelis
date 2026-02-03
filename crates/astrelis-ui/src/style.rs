//! Style system for UI widgets.
//!
//! # Constraint-based Styling
//!
//! The Style API accepts `Constraint` values for dimensions, supporting:
//! - Simple values: `Constraint::Px(100.0)`, `Constraint::Percent(50.0)`, `Constraint::Auto`
//! - Viewport units: `Constraint::Vw(80.0)`, `Constraint::Vh(60.0)`
//! - Complex expressions: `Constraint::calc()`, `Constraint::min()`, `Constraint::max()`, `Constraint::clamp()`
//!
//! For backward compatibility, raw `f32` values and `Length` types are also accepted.
//!
//! # Note on Viewport Units and Complex Constraints
//!
//! Viewport-relative units (vw, vh, vmin, vmax) and complex constraints (calc, min, max, clamp)
//! need to be resolved to absolute pixel values during layout. The layout engine handles this
//! automatically using the current viewport context.

use crate::constraint::Constraint;
use astrelis_render::Color;
use taffy::{
    AlignContent, AlignItems, Dimension, Display, FlexDirection, FlexWrap, JustifyContent,
    LengthPercentage as TaffyLengthPercentage, LengthPercentageAuto as TaffyLengthPercentageAuto,
    Position, Rect, Size, style::Style as TaffyStyle,
};

/// Overflow behavior for content that exceeds widget bounds.
///
/// Controls how content that extends beyond a widget's boundaries is handled
/// during rendering. This affects visual clipping using GPU scissor rectangles.
///
/// # Examples
/// ```
/// use astrelis_ui::{Style, Overflow};
///
/// let style = Style::new()
///     .width(400.0)
///     .height(300.0)
///     .overflow(Overflow::Hidden);  // Clip content at bounds
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    /// Content can overflow - no clipping applied (default).
    ///
    /// This is the current behavior and maintains backward compatibility.
    #[default]
    Visible,

    /// Content is clipped at widget boundaries.
    ///
    /// Uses GPU scissor rectangles for hardware-accelerated clipping.
    /// Ideal for fixed-size containers, scrollable areas, and text fields.
    Hidden,

    /// Content is clipped and scrollbars are shown when needed.
    ///
    /// Phase 2.1 implementation: behaves like Hidden. Scrollbar functionality
    /// will be added in a future phase.
    Scroll,

    /// Automatically show scrollbars only when content overflows.
    ///
    /// Phase 2.1 implementation: behaves like Hidden. Auto-detection will be
    /// added in a future phase.
    Auto,
}

/// Consolidated constraint storage for viewport-relative units.
///
/// Boxed and optional in Style to reduce memory overhead for the common case
/// where no viewport constraints (vw, vh, vmin, vmax, calc, min, max, clamp) are used.
/// This reduces Style size from ~80+ bytes to 8 bytes for the no-constraint case.
#[derive(Debug, Clone, Default)]
pub struct ConstraintSet {
    /// Width constraint
    pub width: Option<Constraint>,
    /// Height constraint
    pub height: Option<Constraint>,
    /// Min width constraint
    pub min_width: Option<Constraint>,
    /// Min height constraint
    pub min_height: Option<Constraint>,
    /// Max width constraint
    pub max_width: Option<Constraint>,
    /// Max height constraint
    pub max_height: Option<Constraint>,
    /// Padding constraints [left, top, right, bottom]
    pub padding: Option<[Constraint; 4]>,
    /// Margin constraints [left, top, right, bottom]
    pub margin: Option<[Constraint; 4]>,
    /// Gap constraint
    pub gap: Option<Constraint>,
    /// Flex basis constraint
    pub flex_basis: Option<Constraint>,
}

impl ConstraintSet {
    /// Check if any constraint needs viewport resolution.
    pub fn needs_resolution(&self) -> bool {
        self.width.as_ref().is_some_and(|c| c.needs_resolution())
            || self.height.as_ref().is_some_and(|c| c.needs_resolution())
            || self
                .min_width
                .as_ref()
                .is_some_and(|c| c.needs_resolution())
            || self
                .min_height
                .as_ref()
                .is_some_and(|c| c.needs_resolution())
            || self
                .max_width
                .as_ref()
                .is_some_and(|c| c.needs_resolution())
            || self
                .max_height
                .as_ref()
                .is_some_and(|c| c.needs_resolution())
            || self
                .padding
                .as_ref()
                .is_some_and(|cs| cs.iter().any(|c| c.needs_resolution()))
            || self
                .margin
                .as_ref()
                .is_some_and(|cs| cs.iter().any(|c| c.needs_resolution()))
            || self.gap.as_ref().is_some_and(|c| c.needs_resolution())
            || self
                .flex_basis
                .as_ref()
                .is_some_and(|c| c.needs_resolution())
    }

    /// Check if any constraints are set.
    pub fn is_empty(&self) -> bool {
        self.width.is_none()
            && self.height.is_none()
            && self.min_width.is_none()
            && self.min_height.is_none()
            && self.max_width.is_none()
            && self.max_height.is_none()
            && self.padding.is_none()
            && self.margin.is_none()
            && self.gap.is_none()
            && self.flex_basis.is_none()
    }
}

/// UI style for widgets.
#[derive(Debug, Clone)]
pub struct Style {
    /// Taffy layout style
    pub layout: TaffyStyle,

    /// Background color
    pub background_color: Option<Color>,

    /// Border color
    pub border_color: Option<Color>,

    /// Border width
    pub border_width: f32,

    /// Border radius
    pub border_radius: f32,

    /// Horizontal overflow behavior
    pub overflow_x: Overflow,

    /// Vertical overflow behavior
    pub overflow_y: Overflow,

    /// Consolidated constraint storage for viewport-relative units.
    /// Boxed and optional to reduce memory overhead (~8 bytes vs ~80+ bytes).
    pub constraints: Option<Box<ConstraintSet>>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            layout: TaffyStyle::default(),
            background_color: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            overflow_x: Overflow::default(),
            overflow_y: Overflow::default(),
            constraints: None,
        }
    }
}

impl Style {
    /// Get mutable access to constraints, creating the box if needed.
    fn constraints_mut(&mut self) -> &mut ConstraintSet {
        self.constraints
            .get_or_insert_with(|| Box::new(ConstraintSet::default()))
    }

    /// Get the width constraint if set.
    pub fn width_constraint(&self) -> Option<&Constraint> {
        self.constraints.as_ref().and_then(|c| c.width.as_ref())
    }

    /// Get the height constraint if set.
    pub fn height_constraint(&self) -> Option<&Constraint> {
        self.constraints.as_ref().and_then(|c| c.height.as_ref())
    }

    /// Get the min width constraint if set.
    pub fn min_width_constraint(&self) -> Option<&Constraint> {
        self.constraints.as_ref().and_then(|c| c.min_width.as_ref())
    }

    /// Get the min height constraint if set.
    pub fn min_height_constraint(&self) -> Option<&Constraint> {
        self.constraints
            .as_ref()
            .and_then(|c| c.min_height.as_ref())
    }

    /// Get the max width constraint if set.
    pub fn max_width_constraint(&self) -> Option<&Constraint> {
        self.constraints.as_ref().and_then(|c| c.max_width.as_ref())
    }

    /// Get the max height constraint if set.
    pub fn max_height_constraint(&self) -> Option<&Constraint> {
        self.constraints
            .as_ref()
            .and_then(|c| c.max_height.as_ref())
    }

    /// Get the padding constraints if set.
    pub fn padding_constraints(&self) -> Option<&[Constraint; 4]> {
        self.constraints.as_ref().and_then(|c| c.padding.as_ref())
    }

    /// Get the margin constraints if set.
    pub fn margin_constraints(&self) -> Option<&[Constraint; 4]> {
        self.constraints.as_ref().and_then(|c| c.margin.as_ref())
    }

    /// Get the gap constraint if set.
    pub fn gap_constraint(&self) -> Option<&Constraint> {
        self.constraints.as_ref().and_then(|c| c.gap.as_ref())
    }

    /// Get the flex basis constraint if set.
    pub fn flex_basis_constraint(&self) -> Option<&Constraint> {
        self.constraints
            .as_ref()
            .and_then(|c| c.flex_basis.as_ref())
    }
}

impl Style {
    /// Create a new default style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set display mode.
    pub fn display(mut self, display: Display) -> Self {
        self.layout.display = display;
        self
    }

    /// Set width. Accepts f32 (pixels), Length, or Constraint.
    ///
    /// # Examples
    /// ```
    /// use astrelis_ui::{Style, constraint::Constraint};
    ///
    /// let style = Style::new()
    ///     .width(400.0)                      // pixels
    ///     .width(Constraint::Percent(50.0))  // percentage
    ///     .width(Constraint::Vw(50.0));      // viewport units (resolved at layout time)
    /// ```
    pub fn width(mut self, width: impl Into<Constraint>) -> Self {
        let constraint = width.into();
        self.constraints_mut().width = Some(constraint.clone());
        // Only resolve simple constraints immediately; others resolved at layout time
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.size.width = dim;
        }
        self
    }

    /// Set height. Accepts f32 (pixels), Length, or Constraint.
    pub fn height(mut self, height: impl Into<Constraint>) -> Self {
        let constraint = height.into();
        self.constraints_mut().height = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.size.height = dim;
        }
        self
    }

    /// Set minimum width. Accepts f32 (pixels), Length, or Constraint.
    pub fn min_width(mut self, width: impl Into<Constraint>) -> Self {
        let constraint = width.into();
        self.constraints_mut().min_width = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.min_size.width = dim;
        }
        self
    }

    /// Set minimum height. Accepts f32 (pixels), Length, or Constraint.
    pub fn min_height(mut self, height: impl Into<Constraint>) -> Self {
        let constraint = height.into();
        self.constraints_mut().min_height = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.min_size.height = dim;
        }
        self
    }

    /// Set maximum width. Accepts f32 (pixels), Length, or Constraint.
    pub fn max_width(mut self, width: impl Into<Constraint>) -> Self {
        let constraint = width.into();
        self.constraints_mut().max_width = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.max_size.width = dim;
        }
        self
    }

    /// Set maximum height. Accepts f32 (pixels), Length, or Constraint.
    pub fn max_height(mut self, height: impl Into<Constraint>) -> Self {
        let constraint = height.into();
        self.constraints_mut().max_height = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.max_size.height = dim;
        }
        self
    }

    /// Set padding for all sides. Accepts f32 (pixels) or Constraint.
    ///
    /// Note: Padding does not support Auto. Use Px or Percent constraints.
    pub fn padding(mut self, padding: impl Into<Constraint> + Copy) -> Self {
        let constraint = padding.into();
        self.constraints_mut().padding = Some([
            constraint.clone(),
            constraint.clone(),
            constraint.clone(),
            constraint.clone(),
        ]);
        if let Some(p) = constraint.try_to_length_percentage() {
            self.layout.padding = Rect {
                left: p,
                top: p,
                right: p,
                bottom: p,
            };
        }
        self
    }

    /// Set padding individually. Accepts f32 (pixels) or Constraint for each side.
    pub fn padding_ltrb(
        mut self,
        left: impl Into<Constraint>,
        top: impl Into<Constraint>,
        right: impl Into<Constraint>,
        bottom: impl Into<Constraint>,
    ) -> Self {
        let left_c = left.into();
        let top_c = top.into();
        let right_c = right.into();
        let bottom_c = bottom.into();
        self.constraints_mut().padding = Some([
            left_c.clone(),
            top_c.clone(),
            right_c.clone(),
            bottom_c.clone(),
        ]);
        // Set simple constraints immediately
        if let Some(l) = left_c.try_to_length_percentage() {
            self.layout.padding.left = l;
        }
        if let Some(t) = top_c.try_to_length_percentage() {
            self.layout.padding.top = t;
        }
        if let Some(r) = right_c.try_to_length_percentage() {
            self.layout.padding.right = r;
        }
        if let Some(b) = bottom_c.try_to_length_percentage() {
            self.layout.padding.bottom = b;
        }
        self
    }

    /// Set margin for all sides. Accepts f32 (pixels) or Constraint.
    pub fn margin(mut self, margin: impl Into<Constraint> + Copy) -> Self {
        let constraint = margin.into();
        self.constraints_mut().margin = Some([
            constraint.clone(),
            constraint.clone(),
            constraint.clone(),
            constraint.clone(),
        ]);
        if let Some(m) = constraint.try_to_length_percentage_auto() {
            self.layout.margin = Rect {
                left: m,
                top: m,
                right: m,
                bottom: m,
            };
        }
        self
    }

    /// Set margin individually. Accepts f32 (pixels) or Constraint for each side.
    pub fn margin_ltrb(
        mut self,
        left: impl Into<Constraint>,
        top: impl Into<Constraint>,
        right: impl Into<Constraint>,
        bottom: impl Into<Constraint>,
    ) -> Self {
        let left_c = left.into();
        let top_c = top.into();
        let right_c = right.into();
        let bottom_c = bottom.into();
        self.constraints_mut().margin = Some([
            left_c.clone(),
            top_c.clone(),
            right_c.clone(),
            bottom_c.clone(),
        ]);
        // Set simple constraints immediately
        if let Some(l) = left_c.try_to_length_percentage_auto() {
            self.layout.margin.left = l;
        }
        if let Some(t) = top_c.try_to_length_percentage_auto() {
            self.layout.margin.top = t;
        }
        if let Some(r) = right_c.try_to_length_percentage_auto() {
            self.layout.margin.right = r;
        }
        if let Some(b) = bottom_c.try_to_length_percentage_auto() {
            self.layout.margin.bottom = b;
        }
        self
    }

    /// Set flex direction.
    pub fn flex_direction(mut self, direction: FlexDirection) -> Self {
        self.layout.flex_direction = direction;
        self
    }

    /// Set flex wrap.
    pub fn flex_wrap(mut self, wrap: FlexWrap) -> Self {
        self.layout.flex_wrap = wrap;
        self
    }

    /// Set flex grow factor.
    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.layout.flex_grow = grow;
        self
    }

    /// Set flex shrink factor.
    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.layout.flex_shrink = shrink;
        self
    }

    /// Set flex basis. Accepts f32 (pixels) or Constraint.
    pub fn flex_basis(mut self, basis: impl Into<Constraint>) -> Self {
        let constraint = basis.into();
        self.constraints_mut().flex_basis = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.flex_basis = dim;
        }
        self
    }

    /// Set justify content.
    pub fn justify_content(mut self, justify: JustifyContent) -> Self {
        self.layout.justify_content = Some(justify);
        self
    }

    /// Set align items.
    pub fn align_items(mut self, align: AlignItems) -> Self {
        self.layout.align_items = Some(align);
        self
    }

    /// Set align content.
    pub fn align_content(mut self, align: AlignContent) -> Self {
        self.layout.align_content = Some(align);
        self
    }

    /// Set gap between items. Accepts f32 (pixels) or Constraint.
    pub fn gap(mut self, gap: impl Into<Constraint> + Copy) -> Self {
        let constraint = gap.into();
        self.constraints_mut().gap = Some(constraint.clone());
        if let Some(g) = constraint.try_to_length_percentage() {
            self.layout.gap = Size {
                width: g,
                height: g,
            };
        }
        self
    }

    /// Set background color.
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Set border color.
    pub fn border_color(mut self, color: Color) -> Self {
        self.border_color = Some(color);
        self
    }

    /// Set border width.
    pub fn border_width(mut self, width: f32) -> Self {
        self.border_width = width;
        self
    }

    /// Set border radius.
    pub fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    /// Set overflow behavior for both axes.
    ///
    /// # Examples
    /// ```
    /// use astrelis_ui::{Style, Overflow};
    ///
    /// let style = Style::new()
    ///     .width(400.0)
    ///     .height(300.0)
    ///     .overflow(Overflow::Hidden);  // Clip overflow content
    /// ```
    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow_x = overflow;
        self.overflow_y = overflow;
        self
    }

    /// Set horizontal overflow behavior.
    ///
    /// # Examples
    /// ```
    /// use astrelis_ui::{Style, Overflow};
    ///
    /// let style = Style::new()
    ///     .width(400.0)
    ///     .overflow_x(Overflow::Hidden);  // Clip horizontal overflow only
    /// ```
    pub fn overflow_x(mut self, overflow: Overflow) -> Self {
        self.overflow_x = overflow;
        self
    }

    /// Set vertical overflow behavior.
    ///
    /// # Examples
    /// ```
    /// use astrelis_ui::{Style, Overflow};
    ///
    /// let style = Style::new()
    ///     .height(300.0)
    ///     .overflow_y(Overflow::Hidden);  // Clip vertical overflow only
    /// ```
    pub fn overflow_y(mut self, overflow: Overflow) -> Self {
        self.overflow_y = overflow;
        self
    }

    /// Set overflow behavior for both axes independently.
    ///
    /// # Examples
    /// ```
    /// use astrelis_ui::{Style, Overflow};
    ///
    /// let style = Style::new()
    ///     .width(400.0)
    ///     .height(300.0)
    ///     .overflow_xy(Overflow::Hidden, Overflow::Scroll);  // Clip X, scroll Y
    /// ```
    pub fn overflow_xy(mut self, x: Overflow, y: Overflow) -> Self {
        self.overflow_x = x;
        self.overflow_y = y;
        self
    }

    /// Set position type.
    pub fn position(mut self, position: Position) -> Self {
        self.layout.position = position;
        self
    }

    /// Set absolute position.
    pub fn absolute_position(mut self, left: f32, top: f32) -> Self {
        self.layout.position = Position::Absolute;
        self.layout.inset = Rect {
            left: TaffyLengthPercentageAuto::Length(left),
            top: TaffyLengthPercentageAuto::Length(top),
            right: TaffyLengthPercentageAuto::Auto,
            bottom: TaffyLengthPercentageAuto::Auto,
        };
        self
    }

    /// Set the aspect ratio constraint.
    ///
    /// When set, the layout will maintain this width-to-height ratio.
    /// Works with Taffy's built-in aspect ratio support.
    ///
    /// # Arguments
    /// * `ratio` - The width-to-height ratio (e.g., 16.0/9.0 for 16:9)
    ///
    /// # Examples
    /// ```
    /// use astrelis_ui::Style;
    ///
    /// // 16:9 video container
    /// let style = Style::new()
    ///     .width(640.0)
    ///     .aspect_ratio(16.0 / 9.0);  // Height will be ~360px
    ///
    /// // Square avatar
    /// let style = Style::new()
    ///     .width(100.0)
    ///     .aspect_ratio(1.0);  // Height will be 100px
    /// ```
    pub fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.layout.aspect_ratio = Some(ratio);
        self
    }

    /// Remove the aspect ratio constraint.
    ///
    /// # Examples
    /// ```
    /// use astrelis_ui::Style;
    ///
    /// let style = Style::new()
    ///     .width(640.0)
    ///     .aspect_ratio(16.0 / 9.0)
    ///     .clear_aspect_ratio();  // Remove aspect ratio constraint
    /// ```
    pub fn clear_aspect_ratio(mut self) -> Self {
        self.layout.aspect_ratio = None;
        self
    }

    /// Check if this style has an aspect ratio constraint.
    pub fn has_aspect_ratio(&self) -> bool {
        self.layout.aspect_ratio.is_some()
    }

    /// Check if this style has any constraints that need viewport resolution.
    ///
    /// Returns true if any dimension constraints use viewport units (vw, vh, vmin, vmax)
    /// or complex expressions (calc, min, max, clamp).
    pub fn has_unresolved_constraints(&self) -> bool {
        self.constraints
            .as_ref()
            .is_some_and(|c| c.needs_resolution())
    }
}

// In-place setter methods (`&mut self`) for use by node builders.
impl Style {
    /// Set display mode in place.
    pub fn set_display(&mut self, display: Display) {
        self.layout.display = display;
    }

    /// Set width in place.
    pub fn set_width(&mut self, width: impl Into<Constraint>) {
        let constraint = width.into();
        self.constraints_mut().width = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.size.width = dim;
        }
    }

    /// Set height in place.
    pub fn set_height(&mut self, height: impl Into<Constraint>) {
        let constraint = height.into();
        self.constraints_mut().height = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.size.height = dim;
        }
    }

    /// Set minimum width in place.
    pub fn set_min_width(&mut self, width: impl Into<Constraint>) {
        let constraint = width.into();
        self.constraints_mut().min_width = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.min_size.width = dim;
        }
    }

    /// Set minimum height in place.
    pub fn set_min_height(&mut self, height: impl Into<Constraint>) {
        let constraint = height.into();
        self.constraints_mut().min_height = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.min_size.height = dim;
        }
    }

    /// Set maximum width in place.
    pub fn set_max_width(&mut self, width: impl Into<Constraint>) {
        let constraint = width.into();
        self.constraints_mut().max_width = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.max_size.width = dim;
        }
    }

    /// Set maximum height in place.
    pub fn set_max_height(&mut self, height: impl Into<Constraint>) {
        let constraint = height.into();
        self.constraints_mut().max_height = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.max_size.height = dim;
        }
    }

    /// Set padding for all sides in place.
    pub fn set_padding(&mut self, padding: impl Into<Constraint> + Copy) {
        let constraint = padding.into();
        self.constraints_mut().padding = Some([
            constraint.clone(),
            constraint.clone(),
            constraint.clone(),
            constraint.clone(),
        ]);
        if let Some(p) = constraint.try_to_length_percentage() {
            self.layout.padding = Rect {
                left: p,
                top: p,
                right: p,
                bottom: p,
            };
        }
    }

    /// Set padding individually in place.
    pub fn set_padding_ltrb(
        &mut self,
        left: impl Into<Constraint>,
        top: impl Into<Constraint>,
        right: impl Into<Constraint>,
        bottom: impl Into<Constraint>,
    ) {
        let left_c = left.into();
        let top_c = top.into();
        let right_c = right.into();
        let bottom_c = bottom.into();
        self.constraints_mut().padding = Some([
            left_c.clone(),
            top_c.clone(),
            right_c.clone(),
            bottom_c.clone(),
        ]);
        if let Some(l) = left_c.try_to_length_percentage() {
            self.layout.padding.left = l;
        }
        if let Some(t) = top_c.try_to_length_percentage() {
            self.layout.padding.top = t;
        }
        if let Some(r) = right_c.try_to_length_percentage() {
            self.layout.padding.right = r;
        }
        if let Some(b) = bottom_c.try_to_length_percentage() {
            self.layout.padding.bottom = b;
        }
    }

    /// Set margin for all sides in place.
    pub fn set_margin(&mut self, margin: impl Into<Constraint> + Copy) {
        let constraint = margin.into();
        self.constraints_mut().margin = Some([
            constraint.clone(),
            constraint.clone(),
            constraint.clone(),
            constraint.clone(),
        ]);
        if let Some(m) = constraint.try_to_length_percentage_auto() {
            self.layout.margin = Rect {
                left: m,
                top: m,
                right: m,
                bottom: m,
            };
        }
    }

    /// Set margin individually in place.
    pub fn set_margin_ltrb(
        &mut self,
        left: impl Into<Constraint>,
        top: impl Into<Constraint>,
        right: impl Into<Constraint>,
        bottom: impl Into<Constraint>,
    ) {
        let left_c = left.into();
        let top_c = top.into();
        let right_c = right.into();
        let bottom_c = bottom.into();
        self.constraints_mut().margin = Some([
            left_c.clone(),
            top_c.clone(),
            right_c.clone(),
            bottom_c.clone(),
        ]);
        if let Some(l) = left_c.try_to_length_percentage_auto() {
            self.layout.margin.left = l;
        }
        if let Some(t) = top_c.try_to_length_percentage_auto() {
            self.layout.margin.top = t;
        }
        if let Some(r) = right_c.try_to_length_percentage_auto() {
            self.layout.margin.right = r;
        }
        if let Some(b) = bottom_c.try_to_length_percentage_auto() {
            self.layout.margin.bottom = b;
        }
    }

    /// Set flex direction in place.
    pub fn set_flex_direction(&mut self, direction: FlexDirection) {
        self.layout.flex_direction = direction;
    }

    /// Set flex wrap in place.
    pub fn set_flex_wrap(&mut self, wrap: FlexWrap) {
        self.layout.flex_wrap = wrap;
    }

    /// Set flex grow factor in place.
    pub fn set_flex_grow(&mut self, grow: f32) {
        self.layout.flex_grow = grow;
    }

    /// Set flex shrink factor in place.
    pub fn set_flex_shrink(&mut self, shrink: f32) {
        self.layout.flex_shrink = shrink;
    }

    /// Set flex basis in place.
    pub fn set_flex_basis(&mut self, basis: impl Into<Constraint>) {
        let constraint = basis.into();
        self.constraints_mut().flex_basis = Some(constraint.clone());
        if let Some(dim) = constraint.try_to_dimension() {
            self.layout.flex_basis = dim;
        }
    }

    /// Set justify content in place.
    pub fn set_justify_content(&mut self, justify: JustifyContent) {
        self.layout.justify_content = Some(justify);
    }

    /// Set align items in place.
    pub fn set_align_items(&mut self, align: AlignItems) {
        self.layout.align_items = Some(align);
    }

    /// Set align content in place.
    pub fn set_align_content(&mut self, align: AlignContent) {
        self.layout.align_content = Some(align);
    }

    /// Set gap between items in place.
    pub fn set_gap(&mut self, gap: impl Into<Constraint> + Copy) {
        let constraint = gap.into();
        self.constraints_mut().gap = Some(constraint.clone());
        if let Some(g) = constraint.try_to_length_percentage() {
            self.layout.gap = Size {
                width: g,
                height: g,
            };
        }
    }

    /// Set background color in place.
    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = Some(color);
    }

    /// Set border color in place.
    pub fn set_border_color(&mut self, color: Color) {
        self.border_color = Some(color);
    }

    /// Set border width in place.
    pub fn set_border_width(&mut self, width: f32) {
        self.border_width = width;
    }

    /// Set border radius in place.
    pub fn set_border_radius(&mut self, radius: f32) {
        self.border_radius = radius;
    }

    /// Set overflow behavior for both axes in place.
    pub fn set_overflow(&mut self, overflow: Overflow) {
        self.overflow_x = overflow;
        self.overflow_y = overflow;
    }

    /// Set horizontal overflow behavior in place.
    pub fn set_overflow_x(&mut self, overflow: Overflow) {
        self.overflow_x = overflow;
    }

    /// Set vertical overflow behavior in place.
    pub fn set_overflow_y(&mut self, overflow: Overflow) {
        self.overflow_y = overflow;
    }

    /// Set position type in place.
    pub fn set_position(&mut self, position: Position) {
        self.layout.position = position;
    }

    /// Set absolute position in place.
    pub fn set_absolute_position(&mut self, left: f32, top: f32) {
        self.layout.position = Position::Absolute;
        self.layout.inset = Rect {
            left: TaffyLengthPercentageAuto::Length(left),
            top: TaffyLengthPercentageAuto::Length(top),
            right: TaffyLengthPercentageAuto::Auto,
            bottom: TaffyLengthPercentageAuto::Auto,
        };
    }

    /// Set aspect ratio in place.
    pub fn set_aspect_ratio(&mut self, ratio: f32) {
        self.layout.aspect_ratio = Some(ratio);
    }

    /// Clear aspect ratio in place.
    pub fn clear_aspect_ratio_mut(&mut self) {
        self.layout.aspect_ratio = None;
    }
}

/// Helper to create a length dimension.
#[allow(dead_code)]
fn length(value: f32) -> Dimension {
    Dimension::Length(value)
}

/// Helper to create a rect with same length on all sides (for padding).
#[allow(dead_code)]
fn length_rect(value: f32) -> Rect<TaffyLengthPercentage> {
    Rect {
        left: TaffyLengthPercentage::Length(value),
        top: TaffyLengthPercentage::Length(value),
        right: TaffyLengthPercentage::Length(value),
        bottom: TaffyLengthPercentage::Length(value),
    }
}

/// Helper to create a rect with same length on all sides (for margin).
#[allow(dead_code)]
fn margin_rect(value: f32) -> Rect<TaffyLengthPercentageAuto> {
    Rect {
        left: TaffyLengthPercentageAuto::Length(value),
        top: TaffyLengthPercentageAuto::Length(value),
        right: TaffyLengthPercentageAuto::Length(value),
        bottom: TaffyLengthPercentageAuto::Length(value),
    }
}

/// Helper to create an auto dimension.
#[allow(dead_code)]
fn auto() -> TaffyLengthPercentageAuto {
    TaffyLengthPercentageAuto::Auto
}
