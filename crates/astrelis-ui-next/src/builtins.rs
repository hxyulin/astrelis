//! Basic elements used by the research vertical slices.

use std::any::Any;

use astrelis_core::{
    color::Color,
    geometry::{LogicalPoint, LogicalRect, LogicalSize},
};
use astrelis_paint::{Brush, Painter};
use astrelis_text::{ParagraphStyle, TextLayout, TextLayoutRequest, TextStyle, TextWrap};

use crate::{
    Constraints, Element, EventResult, Invalidation, LayoutContext, SemanticData, SemanticRole,
    UiError, UiInput,
};

/// Main-axis direction for [`Flex`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Axis {
    /// Children advance from left to right.
    Horizontal,
    /// Children advance from top to bottom.
    #[default]
    Vertical,
}

/// Simple flex container with deterministic retained child layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Flex {
    /// Main axis.
    pub axis: Axis,
    /// Gap between adjacent children.
    pub gap: f32,
    /// Insets inside the container.
    pub padding: f32,
    /// Optional background fill.
    pub background: Option<Color>,
}

impl Default for Flex {
    fn default() -> Self {
        Self {
            axis: Axis::Vertical,
            gap: 0.0,
            padding: 0.0,
            background: None,
        }
    }
}

impl Element for Flex {
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
        let padding = self.padding.max(0.0);
        let inner_max = LogicalSize::new(
            (constraints.max.width - padding * 2.0).max(0.0),
            (constraints.max.height - padding * 2.0).max(0.0),
        );
        let children = context.children();
        let mut cursor = padding;
        let mut cross = 0.0f32;
        for child in children.iter().copied() {
            let child_constraints = match self.axis {
                Axis::Horizontal => Constraints::new(
                    LogicalSize::ZERO,
                    LogicalSize::new(inner_max.width, inner_max.height),
                ),
                Axis::Vertical => Constraints::new(
                    LogicalSize::ZERO,
                    LogicalSize::new(inner_max.width, inner_max.height),
                ),
            };
            let size = context.layout_child(child, child_constraints)?;
            let origin = match self.axis {
                Axis::Horizontal => LogicalPoint::new(cursor, padding),
                Axis::Vertical => LogicalPoint::new(padding, cursor),
            };
            context.place_child(child, origin)?;
            match self.axis {
                Axis::Horizontal => {
                    cursor += size.width + self.gap.max(0.0);
                    cross = cross.max(size.height);
                }
                Axis::Vertical => {
                    cursor += size.height + self.gap.max(0.0);
                    cross = cross.max(size.width);
                }
            }
        }
        if !children.is_empty() {
            cursor -= self.gap.max(0.0);
        }
        let desired = match self.axis {
            Axis::Horizontal => LogicalSize::new(cursor + padding, cross + padding * 2.0),
            Axis::Vertical => LogicalSize::new(cross + padding * 2.0, cursor + padding),
        };
        Ok(constraints.constrain(desired))
    }

    fn paint(
        &self,
        painter: &mut Painter,
        size: LogicalSize,
    ) -> Result<(), astrelis_paint::PaintError> {
        if let Some(color) = self.background {
            painter.fill_rect(
                LogicalRect::from_xywh(0.0, 0.0, size.width, size.height),
                Brush::Solid(color),
            )?;
        }
        Ok(())
    }
}

/// Fixed or constraint-filling visual box.
#[derive(Clone, Debug, PartialEq)]
pub struct BoxElement {
    /// Preferred size.
    pub size: LogicalSize,
    /// Background color.
    pub color: Color,
    /// Accessible properties.
    pub semantics: Option<SemanticData>,
    /// Whether pointer input targets this box.
    pub interactive: bool,
}

impl BoxElement {
    /// Creates a colored fixed-size box.
    pub fn new(size: LogicalSize, color: Color) -> Self {
        Self {
            size,
            color,
            semantics: None,
            interactive: false,
        }
    }
}

impl Element for BoxElement {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn layout(
        &mut self,
        _context: &mut LayoutContext<'_>,
        constraints: Constraints,
    ) -> Result<LogicalSize, UiError> {
        Ok(constraints.constrain(self.size))
    }

    fn paint(
        &self,
        painter: &mut Painter,
        size: LogicalSize,
    ) -> Result<(), astrelis_paint::PaintError> {
        painter.fill_rect(
            LogicalRect::from_xywh(0.0, 0.0, size.width, size.height),
            Brush::Solid(self.color),
        )?;
        Ok(())
    }

    fn accessibility(&self) -> Option<SemanticData> {
        self.semantics.clone()
    }

    fn hit_testable(&self) -> bool {
        self.interactive
    }
}

/// Retained shaped text label.
#[derive(Clone, Debug)]
pub struct Label {
    /// Display and accessible text.
    pub text: String,
    /// Nominal font size used for intrinsic measurement.
    pub font_size: f32,
    /// Optional paint color for a deterministic placeholder glyph bar.
    pub color: Option<Color>,
    preferred_width: Option<f32>,
    layout: Option<TextLayout>,
}

impl Label {
    /// Creates a label.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            font_size: 14.0,
            color: Some(Color::WHITE),
            preferred_width: None,
            layout: None,
        }
    }

    /// Sets the label's font size.
    pub fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    /// Sets the label's glyph color.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Constrains wrapping and reserves a preferred width.
    pub fn with_width(mut self, width: f32) -> Self {
        self.preferred_width = Some(width.max(0.0));
        self
    }

    /// Changes the optional preferred width.
    pub fn set_width(&mut self, width: Option<f32>) {
        self.preferred_width = width.map(|width| width.max(0.0));
    }
}

impl Element for Label {
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
        let mut request = TextLayoutRequest::new(self.text.clone());
        request.style = TextStyle {
            size: self.font_size.max(1.0),
            color: self.color.unwrap_or(Color::WHITE),
            ..TextStyle::default()
        };
        let max_width = self
            .preferred_width
            .unwrap_or(constraints.max.width)
            .min(constraints.max.width);
        request.paragraph = ParagraphStyle {
            max_width: Some(max_width),
            wrap: TextWrap::Wrap,
            ..ParagraphStyle::default()
        };
        let layout = context.shape_text(request)?;
        let mut desired = layout.size();
        if let Some(width) = self.preferred_width {
            desired.width = width;
        }
        let size = constraints.constrain(desired);
        self.layout = Some(layout);
        Ok(size)
    }

    fn paint(
        &self,
        painter: &mut Painter,
        _size: LogicalSize,
    ) -> Result<(), astrelis_paint::PaintError> {
        if let Some(layout) = &self.layout {
            painter.draw_text(layout, LogicalPoint::ZERO, 1.0)?;
        }
        Ok(())
    }

    fn accessibility(&self) -> Option<SemanticData> {
        Some(SemanticData {
            role: SemanticRole::Label,
            label: self.text.clone(),
            ..SemanticData::default()
        })
    }
}

/// Minimal activatable control for action-routing tests.
pub struct Button {
    /// Accessible label.
    pub label: String,
    /// Preferred size.
    pub size: LogicalSize,
    /// Normal background.
    pub color: Color,
    /// Pressed background.
    pub pressed_color: Color,
    pressed: bool,
    action: Option<Box<dyn Fn() -> Box<dyn Any>>>,
}

impl Button {
    /// Creates a button which emits `action` on release.
    pub fn new<A: Any + Clone>(
        label: impl Into<String>,
        size: LogicalSize,
        color: Color,
        pressed_color: Color,
        action: A,
    ) -> Self {
        Self {
            label: label.into(),
            size,
            color,
            pressed_color,
            pressed: false,
            action: Some(Box::new(move || Box::new(action.clone()))),
        }
    }

    /// Creates a button backed by an erased action factory.
    ///
    /// Higher-level typed component runtimes use this to attach routing
    /// metadata without making the retained core generic over application
    /// action types.
    pub fn with_action_factory(
        label: impl Into<String>,
        size: LogicalSize,
        color: Color,
        pressed_color: Color,
        action: impl Fn() -> Box<dyn Any> + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            size,
            color,
            pressed_color,
            pressed: false,
            action: Some(Box::new(action)),
        }
    }

    /// Replaces the erased action factory without recreating the control.
    pub fn set_action_factory(&mut self, action: impl Fn() -> Box<dyn Any> + 'static) {
        self.action = Some(Box::new(action));
    }
}

impl Element for Button {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn layout(
        &mut self,
        _context: &mut LayoutContext<'_>,
        constraints: Constraints,
    ) -> Result<LogicalSize, UiError> {
        Ok(constraints.constrain(self.size))
    }

    fn paint(
        &self,
        painter: &mut Painter,
        size: LogicalSize,
    ) -> Result<(), astrelis_paint::PaintError> {
        painter.fill_rect(
            LogicalRect::from_xywh(0.0, 0.0, size.width, size.height),
            Brush::Solid(if self.pressed {
                self.pressed_color
            } else {
                self.color
            }),
        )?;
        Ok(())
    }

    fn accessibility(&self) -> Option<SemanticData> {
        Some(SemanticData {
            role: SemanticRole::Button,
            label: self.label.clone(),
            ..SemanticData::default()
        })
    }

    fn event(&mut self, input: UiInput) -> EventResult {
        match input {
            UiInput::PointerPressed(_) => {
                self.pressed = true;
                EventResult {
                    invalidation: Invalidation::PAINT,
                    handled: true,
                    ..EventResult::default()
                }
            }
            UiInput::PointerReleased(_) if self.pressed => {
                self.pressed = false;
                EventResult {
                    action: self.action.as_ref().map(|action| action()),
                    invalidation: Invalidation::PAINT,
                    handled: true,
                }
            }
            _ => EventResult::default(),
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn focusable(&self) -> bool {
        true
    }
}
