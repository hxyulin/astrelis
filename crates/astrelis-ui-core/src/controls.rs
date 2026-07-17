//! Built-in control behaviors: checkbox, slider, and scrolling.

use super::*;

impl<Message: 'static> Ui<Message> {
    pub(crate) fn toggle_checkbox_id(&mut self, id: ElementId) -> Result<(), UiError> {
        let checked = {
            let Kind::Checkbox { checked } = &mut self.node_mut(id)?.kind else {
                return Err(UiError::new("element is not a checkbox"));
            };
            *checked = !*checked;
            *checked
        };
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        self.dispatch_routed(id, RoutedEventKind::CheckedChanged(checked))?;
        Ok(())
    }

    pub(crate) fn set_slider_from_point(
        &mut self,
        id: ElementId,
        point: LogicalPoint,
    ) -> Result<(), UiError> {
        let bounds = self.node(id)?.bounds;
        let ratio = ((point.x - bounds.origin.x) / bounds.size.width.max(1.0)).clamp(0.0, 1.0);
        let value = {
            let Kind::Slider {
                min,
                max,
                step,
                value,
            } = &mut self.node_mut(id)?.kind
            else {
                return Err(UiError::new("element is not a slider"));
            };
            let next = snap_slider(*min + ratio * (*max - *min), *min, *max, *step);
            if *value == next {
                return Ok(());
            }
            *value = next;
            next
        };
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        self.dispatch_routed(id, RoutedEventKind::SliderChanged(value))?;
        Ok(())
    }

    pub(crate) fn handle_slider_key(&mut self, id: ElementId, key: &Key) -> Result<(), UiError> {
        let name = match key {
            Key::Named(NamedKey::Other(name)) => name.as_str(),
            _ => return Ok(()),
        };
        let value = {
            let Kind::Slider {
                min,
                max,
                step,
                value,
            } = &mut self.node_mut(id)?.kind
            else {
                return Ok(());
            };
            let next = match name {
                "ArrowLeft" | "ArrowDown" => *value - *step,
                "ArrowRight" | "ArrowUp" => *value + *step,
                "Home" => *min,
                "End" => *max,
                _ => return Ok(()),
            };
            let next = snap_slider(next, *min, *max, *step);
            if next == *value {
                return Ok(());
            }
            *value = next;
            next
        };
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        self.dispatch_routed(id, RoutedEventKind::SliderChanged(value))?;
        Ok(())
    }

    pub(crate) fn handle_scroll_key(&mut self, id: ElementId, key: &Key) -> Result<(), UiError> {
        let Key::Named(NamedKey::Other(name)) = key else {
            return Ok(());
        };
        let height = self.node(id)?.bounds.size.height;
        let delta = match name.as_str() {
            "ArrowUp" => -40.0,
            "ArrowDown" => 40.0,
            "PageUp" => -height * 0.9,
            "PageDown" => height * 0.9,
            "Home" => -f32::MAX,
            "End" => f32::MAX,
            _ => return Ok(()),
        };
        self.scroll_by_id(id, delta)?;
        Ok(())
    }

    pub(crate) fn set_scroll_from_point(
        &mut self,
        id: ElementId,
        point: LogicalPoint,
    ) -> Result<(), UiError> {
        let bounds = self.node(id)?.bounds;
        let (content_height, previous) = match self.node(id)?.kind {
            Kind::ScrollView {
                content_height,
                offset,
            } => (content_height, offset),
            _ => return Ok(()),
        };
        if content_height <= bounds.size.height || point.x < bounds.max_x() - 12.0 {
            return Ok(());
        }
        let thumb_height = (bounds.size.height * bounds.size.height / content_height)
            .max(24.0)
            .min(bounds.size.height);
        let travel = (bounds.size.height - thumb_height).max(1.0);
        let ratio = ((point.y - bounds.origin.y - thumb_height * 0.5) / travel).clamp(0.0, 1.0);
        let next = ratio * (content_height - bounds.size.height);
        if next != previous {
            if let Kind::ScrollView { offset, .. } = &mut self.node_mut(id)?.kind {
                *offset = next;
            }
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        }
        Ok(())
    }

    pub(crate) fn scroll_by_id(&mut self, id: ElementId, delta: f32) -> Result<bool, UiError> {
        let viewport = self.node(id)?.bounds.size.height;
        let changed = {
            let Kind::ScrollView {
                offset,
                content_height,
            } = &mut self.node_mut(id)?.kind
            else {
                return Err(UiError::new("element is not a scroll view"));
            };
            let previous = *offset;
            *offset = (*offset + delta).clamp(0.0, (*content_height - viewport).max(0.0));
            previous != *offset
        };
        if changed {
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        }
        Ok(changed)
    }

    pub(crate) fn reveal_focused_descendant(&mut self, target: ElementId) -> Result<(), UiError> {
        let target_bounds = self.node(target)?.bounds;
        let mut ancestor = self.node(target)?.parent;
        while let Some(id) = ancestor {
            if let Kind::ScrollView {
                offset,
                content_height,
            } = self.node(id)?.kind
            {
                let viewport = self.node(id)?.bounds;
                let mut next = offset;
                if target_bounds.origin.y < viewport.origin.y + offset {
                    next = target_bounds.origin.y - viewport.origin.y;
                } else if target_bounds.max_y() > viewport.max_y() + offset {
                    next = target_bounds.max_y() - viewport.max_y();
                }
                let max = (content_height - viewport.size.height).max(0.0);
                next = next.clamp(0.0, max);
                if next != offset {
                    if let Kind::ScrollView { offset, .. } = &mut self.node_mut(id)?.kind {
                        *offset = next;
                    }
                    self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
                }
            }
            ancestor = self.node(id)?.parent;
        }
        Ok(())
    }
}
