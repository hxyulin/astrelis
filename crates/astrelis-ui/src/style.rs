//! Style system for UI widgets.

use crate::length::{Length, LengthAuto, LengthPercentage};
use astrelis_render::Color;
use taffy::{
    AlignContent, AlignItems, Dimension, Display, FlexDirection, FlexWrap, JustifyContent,
    LengthPercentage as TaffyLengthPercentage, LengthPercentageAuto as TaffyLengthPercentageAuto,
    Position, Rect, Size, style::Style as TaffyStyle,
};

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
}

impl Default for Style {
    fn default() -> Self {
        Self {
            layout: TaffyStyle::default(),
            background_color: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
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

    /// Set width. Accepts f32 (pixels), Length, or LengthAuto.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.layout.size.width = width.into().to_dimension();
        self
    }

    /// Set height. Accepts f32 (pixels), Length, or LengthAuto.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.layout.size.height = height.into().to_dimension();
        self
    }

    /// Set minimum width. Accepts f32 (pixels), Length, or LengthAuto.
    pub fn min_width(mut self, width: impl Into<Length>) -> Self {
        self.layout.min_size.width = width.into().to_dimension();
        self
    }

    /// Set minimum height. Accepts f32 (pixels), Length, or LengthAuto.
    pub fn min_height(mut self, height: impl Into<Length>) -> Self {
        self.layout.min_size.height = height.into().to_dimension();
        self
    }

    /// Set maximum width. Accepts f32 (pixels), Length, or LengthAuto.
    pub fn max_width(mut self, width: impl Into<Length>) -> Self {
        self.layout.max_size.width = width.into().to_dimension();
        self
    }

    /// Set maximum height. Accepts f32 (pixels), Length, or LengthAuto.
    pub fn max_height(mut self, height: impl Into<Length>) -> Self {
        self.layout.max_size.height = height.into().to_dimension();
        self
    }

    /// Set padding for all sides. Accepts f32 (pixels) or LengthPercentage.
    pub fn padding(mut self, padding: impl Into<LengthPercentage> + Copy) -> Self {
        let p = padding.into().to_length_percentage();
        self.layout.padding = Rect {
            left: p,
            top: p,
            right: p,
            bottom: p,
        };
        self
    }

    /// Set padding individually. Accepts f32 (pixels) or LengthPercentage for each side.
    pub fn padding_ltrb(
        mut self,
        left: impl Into<LengthPercentage>,
        top: impl Into<LengthPercentage>,
        right: impl Into<LengthPercentage>,
        bottom: impl Into<LengthPercentage>,
    ) -> Self {
        self.layout.padding = Rect {
            left: left.into().to_length_percentage(),
            top: top.into().to_length_percentage(),
            right: right.into().to_length_percentage(),
            bottom: bottom.into().to_length_percentage(),
        };
        self
    }

    /// Set margin for all sides. Accepts f32 (pixels) or LengthAuto.
    pub fn margin(mut self, margin: impl Into<LengthAuto> + Copy) -> Self {
        let m = margin.into().to_length_percentage_auto();
        self.layout.margin = Rect {
            left: m,
            top: m,
            right: m,
            bottom: m,
        };
        self
    }

    /// Set margin individually. Accepts f32 (pixels) or LengthAuto for each side.
    pub fn margin_ltrb(
        mut self,
        left: impl Into<LengthAuto>,
        top: impl Into<LengthAuto>,
        right: impl Into<LengthAuto>,
        bottom: impl Into<LengthAuto>,
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

    /// Set flex basis. Accepts f32 (pixels), Length, or LengthAuto.
    pub fn flex_basis(mut self, basis: impl Into<Length>) -> Self {
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

    /// Set gap between items. Accepts f32 (pixels) or LengthPercentage.
    pub fn gap(mut self, gap: impl Into<LengthPercentage> + Copy) -> Self {
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
}

/// Helper to create a length dimension.
fn length(value: f32) -> Dimension {
    Dimension::Length(value)
}

/// Helper to create a rect with same length on all sides (for padding).
fn length_rect(value: f32) -> Rect<TaffyLengthPercentage> {
    Rect {
        left: TaffyLengthPercentage::Length(value),
        top: TaffyLengthPercentage::Length(value),
        right: TaffyLengthPercentage::Length(value),
        bottom: TaffyLengthPercentage::Length(value),
    }
}

/// Helper to create a rect with same length on all sides (for margin).
fn margin_rect(value: f32) -> Rect<TaffyLengthPercentageAuto> {
    Rect {
        left: TaffyLengthPercentageAuto::Length(value),
        top: TaffyLengthPercentageAuto::Length(value),
        right: TaffyLengthPercentageAuto::Length(value),
        bottom: TaffyLengthPercentageAuto::Length(value),
    }
}

/// Helper to create an auto dimension.
fn auto() -> TaffyLengthPercentageAuto {
    TaffyLengthPercentageAuto::Auto
}
