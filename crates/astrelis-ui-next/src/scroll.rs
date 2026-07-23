//! Clipped scrolling container.

use std::any::Any;

use astrelis_core::geometry::{LogicalPoint, LogicalSize};

use crate::{Constraints, Element, EventResult, Invalidation, LayoutContext, UiError, UiInput};

/// Axes along which content may exceed the viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAxis {
    /// Vertical scrolling only.
    #[default]
    Vertical,
    /// Horizontal scrolling only.
    Horizontal,
    /// Independent horizontal and vertical scrolling.
    Both,
}

/// One clipped viewport which overlays its children at a controlled offset.
#[derive(Clone, Debug, PartialEq)]
pub struct Scroll {
    /// Enabled scroll axes.
    pub axis: ScrollAxis,
    /// Current controlled logical offset.
    pub offset: LogicalPoint,
    viewport: LogicalSize,
    content: LogicalSize,
}

impl Scroll {
    /// Creates a viewport at offset zero.
    pub const fn new(axis: ScrollAxis) -> Self {
        Self {
            axis,
            offset: LogicalPoint::ZERO,
            viewport: LogicalSize::ZERO,
            content: LogicalSize::ZERO,
        }
    }

    /// Returns the largest valid offset after the latest layout.
    pub fn max_offset(&self) -> LogicalPoint {
        LogicalPoint::new(
            (self.content.width - self.viewport.width).max(0.0),
            (self.content.height - self.viewport.height).max(0.0),
        )
    }

    fn clamp_offset(&mut self) {
        let max = self.max_offset();
        self.offset.x = self.offset.x.clamp(0.0, max.x);
        self.offset.y = self.offset.y.clamp(0.0, max.y);
    }
}

impl Default for Scroll {
    fn default() -> Self {
        Self::new(ScrollAxis::Vertical)
    }
}

impl Element for Scroll {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn layout(
        &mut self,
        context: &mut LayoutContext<'_>,
        constraints: Constraints,
    ) -> Result<LogicalSize, UiError> {
        let viewport = constraints.max;
        let child_max = LogicalSize::new(
            if matches!(self.axis, ScrollAxis::Horizontal | ScrollAxis::Both) {
                1_000_000.0
            } else {
                viewport.width
            },
            if matches!(self.axis, ScrollAxis::Vertical | ScrollAxis::Both) {
                1_000_000.0
            } else {
                viewport.height
            },
        );
        let mut content = LogicalSize::ZERO;
        let children = context.children();
        for child in children.iter().copied() {
            let size =
                context.layout_child(child, Constraints::new(LogicalSize::ZERO, child_max))?;
            content.width = content.width.max(size.width);
            content.height = content.height.max(size.height);
        }
        self.viewport = viewport;
        self.content = content;
        self.clamp_offset();
        for child in children {
            context.place_child(child, LogicalPoint::new(-self.offset.x, -self.offset.y))?;
        }
        Ok(constraints.constrain(viewport))
    }

    fn event(&mut self, input: UiInput) -> EventResult {
        let UiInput::PointerWheel { delta, .. } = input else {
            return EventResult::default();
        };
        let before = self.offset;
        if matches!(self.axis, ScrollAxis::Horizontal | ScrollAxis::Both) {
            self.offset.x += delta.x;
        }
        if matches!(self.axis, ScrollAxis::Vertical | ScrollAxis::Both) {
            self.offset.y += delta.y;
        }
        self.clamp_offset();
        EventResult {
            invalidation: if self.offset != before {
                Invalidation::LAYOUT_ALL
            } else {
                Invalidation::default()
            },
            handled: true,
            ..EventResult::default()
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn clips_children(&self) -> bool {
        true
    }
}
