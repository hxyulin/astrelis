//! Property-aware retained mutation used by framework reconciliation.

use std::{any::Any, ops::RangeInclusive};

use astrelis_core::{color::Color, geometry::LogicalSize};

use crate::{
    Axis, BoxElement, Button, ButtonIcon, Checkbox, Element, Flex, Frame, Invalidation, Label,
    NodeHandle, Scroll, ScrollAxis, SemanticData, Slider, Stack, TextField, UiError, UiRoot,
};

/// Typed property-aware access to one retained element.
pub struct ElementMut<'a, E: Element> {
    ui: &'a mut UiRoot,
    handle: NodeHandle<E>,
}

impl UiRoot {
    /// Begins property-aware mutation of a typed retained element.
    pub fn edit<E: Element>(&mut self, handle: NodeHandle<E>) -> ElementMut<'_, E> {
        ElementMut { ui: self, handle }
    }
}

impl ElementMut<'_, Label> {
    /// Replaces shaped label content and resolved typography.
    pub fn set_content(
        self,
        text: String,
        font_size: f32,
        color: Color,
        width: Option<f32>,
    ) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |label| {
                label.text = text;
                label.font_size = font_size;
                label.color = Some(color);
                label.set_width(width);
            })
    }
}

impl ElementMut<'_, BoxElement> {
    /// Replaces resolved box geometry, paint, semantics, and interaction.
    pub fn set_box(
        self,
        size: LogicalSize,
        color: Color,
        semantics: Option<SemanticData>,
        interactive: bool,
    ) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |element| {
                element.size = size;
                element.color = color;
                element.semantics = semantics;
                element.interactive = interactive;
            })
    }
}

impl ElementMut<'_, Button> {
    /// Replaces resolved button content and colors.
    pub fn set_button(
        &mut self,
        label: String,
        size: LogicalSize,
        color: Color,
        pressed_color: Color,
    ) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |button| {
                button.label = label;
                button.size = size;
                button.color = color;
                button.pressed_color = pressed_color;
            })
    }

    /// Replaces the typed-erased activation factory.
    pub fn set_action(self, action: impl Fn() -> Box<dyn Any> + 'static) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::empty(), |button| {
                button.set_action_factory(action);
            })
    }

    /// Replaces optional leading vector content.
    pub fn set_icon(self, icon: Option<ButtonIcon>) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |button| {
                button.set_icon(icon);
            })
    }

    /// Selects whether the accessible label is also painted.
    pub fn set_label_visible(self, visible: bool) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |button| {
                button.set_label_visible(visible);
            })
    }
}

impl ElementMut<'_, TextField> {
    /// Replaces controlled field content, resolved colors, and change action.
    pub fn set_field(
        &mut self,
        label: String,
        value: String,
        text_color: Color,
        background: Color,
        changed: impl Fn(String) -> Box<dyn Any> + 'static,
    ) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |field| {
                field.label = label;
                field.set_text(value);
                field.text_color = text_color;
                field.background = background;
                field.set_changed_factory(changed);
            })
    }

    /// Replaces the typed-erased change-action factory.
    pub fn set_change_action(
        self,
        changed: impl Fn(String) -> Box<dyn Any> + 'static,
    ) -> Result<(), UiError> {
        self.ui.update(self.handle, Invalidation::empty(), |field| {
            field.set_changed_factory(changed);
        })
    }
}

impl ElementMut<'_, Flex> {
    /// Replaces resolved flex layout and background properties.
    pub fn set_flex(
        self,
        axis: Axis,
        gap: f32,
        padding: f32,
        background: Option<Color>,
    ) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |flex| {
                flex.axis = axis;
                flex.gap = gap;
                flex.padding = padding;
                flex.background = background;
            })
    }
}

impl ElementMut<'_, Frame> {
    /// Replaces explicit sizing and flex growth.
    pub fn set_frame(
        self,
        width: Option<f32>,
        height: Option<f32>,
        min: LogicalSize,
        max: Option<LogicalSize>,
        grow: f32,
    ) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |frame| {
                frame.width = width.map(|value| value.max(0.0));
                frame.height = height.map(|value| value.max(0.0));
                frame.min = LogicalSize::new(min.width.max(0.0), min.height.max(0.0));
                frame.max = max.map(|size| {
                    LogicalSize::new(
                        size.width.max(frame.min.width),
                        size.height.max(frame.min.height),
                    )
                });
                frame.grow = grow.max(0.0);
            })
    }
}

impl ElementMut<'_, Stack> {
    /// Replaces resolved overlay padding and background properties.
    pub fn set_stack(self, padding: f32, background: Option<Color>) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |stack| {
                stack.padding = padding;
                stack.background = background;
            })
    }
}

impl ElementMut<'_, Scroll> {
    /// Replaces controlled scroll axis and offset.
    pub fn set_scroll(
        self,
        axis: ScrollAxis,
        offset: astrelis_core::geometry::LogicalPoint,
    ) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |scroll| {
                scroll.axis = axis;
                scroll.offset = offset;
            })
    }
}

impl ElementMut<'_, Checkbox> {
    /// Replaces controlled checkbox state and resolved presentation.
    pub fn set_checkbox(
        &mut self,
        label: String,
        checked: bool,
        text_color: Color,
        outline_color: Color,
        accent_color: Color,
    ) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |checkbox| {
                checkbox.label = label;
                checkbox.checked = checked;
                checkbox.text_color = text_color;
                checkbox.outline_color = outline_color;
                checkbox.accent_color = accent_color;
            })
    }

    /// Replaces the typed-erased checkbox change action.
    pub fn set_change_action(
        self,
        changed: impl Fn(bool) -> Box<dyn Any> + 'static,
    ) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::empty(), |checkbox| {
                checkbox.set_changed(changed);
            })
    }
}

impl ElementMut<'_, Slider> {
    /// Replaces controlled slider state and resolved presentation.
    pub fn set_slider(
        &mut self,
        label: String,
        value: f32,
        range: RangeInclusive<f32>,
        step: f32,
        track_color: Color,
        accent_color: Color,
    ) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::LAYOUT_ALL, |slider| {
                let start = (*range.start()).min(*range.end());
                let end = (*range.start()).max(*range.end());
                slider.label = label;
                slider.range = start..=end;
                slider.value = value.clamp(*slider.range.start(), *slider.range.end());
                slider.step = step.max(0.0);
                slider.track_color = track_color;
                slider.accent_color = accent_color;
            })
    }

    /// Replaces the typed-erased slider change action.
    pub fn set_change_action(
        self,
        changed: impl Fn(f32) -> Box<dyn Any> + 'static,
    ) -> Result<(), UiError> {
        self.ui
            .update(self.handle, Invalidation::empty(), |slider| {
                slider.set_changed(changed);
            })
    }
}
