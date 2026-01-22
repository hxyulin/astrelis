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
        }
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
    ///     .width(Constraint::Percent(50.0)); // percentage
    ///
    /// // Note: Viewport units (Vw, Vh, etc.) and complex constraints
    /// // (calc, min, max, clamp) require resolution before use.
    /// // See ConstraintResolver for resolving these values.
    /// ```
    pub fn width(mut self, width: impl Into<Constraint>) -> Self {
        self.layout.size.width = width.into().to_dimension();
        self
    }

    /// Set height. Accepts f32 (pixels), Length, or Constraint.
    pub fn height(mut self, height: impl Into<Constraint>) -> Self {
        self.layout.size.height = height.into().to_dimension();
        self
    }

    /// Set minimum width. Accepts f32 (pixels), Length, or Constraint.
    pub fn min_width(mut self, width: impl Into<Constraint>) -> Self {
        self.layout.min_size.width = width.into().to_dimension();
        self
    }

    /// Set minimum height. Accepts f32 (pixels), Length, or Constraint.
    pub fn min_height(mut self, height: impl Into<Constraint>) -> Self {
        self.layout.min_size.height = height.into().to_dimension();
        self
    }

    /// Set maximum width. Accepts f32 (pixels), Length, or Constraint.
    pub fn max_width(mut self, width: impl Into<Constraint>) -> Self {
        self.layout.max_size.width = width.into().to_dimension();
        self
    }

    /// Set maximum height. Accepts f32 (pixels), Length, or Constraint.
    pub fn max_height(mut self, height: impl Into<Constraint>) -> Self {
        self.layout.max_size.height = height.into().to_dimension();
        self
    }

    /// Set padding for all sides. Accepts f32 (pixels) or Constraint.
    ///
    /// Note: Padding does not support Auto. Use Px or Percent constraints.
    pub fn padding(mut self, padding: impl Into<Constraint> + Copy) -> Self {
        let p = padding.into().to_length_percentage();
        self.layout.padding = Rect {
            left: p,
            top: p,
            right: p,
            bottom: p,
        };
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
        self.layout.padding = Rect {
            left: left.into().to_length_percentage(),
            top: top.into().to_length_percentage(),
            right: right.into().to_length_percentage(),
            bottom: bottom.into().to_length_percentage(),
        };
        self
    }

    /// Set margin for all sides. Accepts f32 (pixels) or Constraint.
    pub fn margin(mut self, margin: impl Into<Constraint> + Copy) -> Self {
        let m = margin.into().to_length_percentage_auto();
        self.layout.margin = Rect {
            left: m,
            top: m,
            right: m,
            bottom: m,
        };
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
        self.layout.margin = Rect {
            left: left.into().to_length_percentage_auto(),
            top: top.into().to_length_percentage_auto(),
            right: right.into().to_length_percentage_auto(),
            bottom: bottom.into().to_length_percentage_auto(),
        };
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
        self.layout.flex_basis = basis.into().to_dimension();
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
        let g = gap.into().to_length_percentage();
        self.layout.gap = Size {
            width: g,
            height: g,
        };
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
