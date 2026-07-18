//! Hit testing, hover tracking, and keyboard focus.

use super::*;

impl<Message: 'static> Ui<Message> {
    pub(crate) fn hit_test(&self, point: LogicalPoint) -> Option<ElementId> {
        astrelis_profiling::profile_scope!("ui.hit_test");
        let mut overlays = self
            .ids()
            .filter(|id| {
                self.node(*id)
                    .is_ok_and(|node| matches!(node.kind, Kind::Overlay { .. }))
            })
            .collect::<Vec<_>>();
        overlays.sort_by_key(|id| self.node(*id).map_or(0, |node| node.z_index));
        for overlay in overlays.into_iter().rev() {
            let enabled = self
                .node(overlay)
                .ok()
                .and_then(|node| match node.kind {
                    Kind::Overlay { owner, .. } => Some(self.is_effectively_interactive(owner)),
                    _ => None,
                })
                .unwrap_or(true);
            if let Some(hit) = self.hit_test_node(overlay, point, enabled) {
                return Some(hit);
            }
        }
        self.hit_test_node(self.root, point, true)
    }

    pub(crate) fn hit_test_node(
        &self,
        id: ElementId,
        point: LogicalPoint,
        ancestors_enabled: bool,
    ) -> Option<ElementId> {
        let node = self.node(id).ok()?;
        if node.visibility != Visibility::Visible || !ancestors_enabled {
            return None;
        }
        let transform = node_local_transform(node);
        let determinant = transform.matrix2.determinant();
        if !determinant.is_finite() || determinant.abs() <= f32::EPSILON {
            return None;
        }
        let local = transform
            .inverse()
            .transform_point2(Vec2::new(point.x, point.y));
        let point = Point::new(local.x, local.y);
        if (node.overflow == Overflow::Clip || matches!(node.kind, Kind::ScrollView { .. }))
            && !node.bounds.contains(point)
        {
            return None;
        }
        if matches!(node.kind, Kind::ScrollView { content_height, .. } if content_height > node.bounds.size.height)
            && node.bounds.contains(point)
            && point.x >= node.bounds.max_x() - 12.0
        {
            return Some(id);
        }
        let child_point = match node.kind {
            Kind::ScrollView { offset, .. } => Point::new(point.x, point.y + offset),
            _ => point,
        };
        let mut children = node
            .children
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();
        children.sort_by_key(|(index, child)| {
            (self.node(*child).map_or(0, |node| node.z_index), *index)
        });
        for (_, child) in children.into_iter().rev() {
            if self
                .node(child)
                .is_ok_and(|node| matches!(node.kind, Kind::Overlay { .. }))
            {
                continue;
            }
            if let Some(hit) = self.hit_test_node(child, child_point, node.enabled) {
                return Some(hit);
            }
        }
        let shape_hit = self.custom_widgets.get(&id).map_or_else(
            || node.bounds.contains(point),
            |widget| widget.hit_test(point, node.bounds),
        );
        (node.enabled && shape_hit && self.is_hit_testable_id(id)).then_some(id)
    }

    pub(crate) fn set_hover(
        &mut self,
        device_id: DeviceId,
        position: LogicalPoint,
        target: Option<ElementId>,
    ) -> Result<(), UiError> {
        let old_path = self
            .hover_paths
            .get(&device_id)
            .cloned()
            .unwrap_or_default();
        let old = old_path.last().copied();
        if old == target {
            self.hover = target;
            return Ok(());
        }
        let new_path = target
            .map(|target| self.route_to(target))
            .transpose()?
            .unwrap_or_default();
        self.hover_paths.insert(device_id, new_path);
        self.hover = target;
        for index in 0..self.slots.len() {
            let Some(id) = self.id_at(index) else {
                continue;
            };
            let hovered = self.hover_paths.values().any(|path| path.contains(&id));
            self.node_mut(id)?.hovered = hovered;
        }
        if let Some(old) = old {
            self.dispatch_routed(
                old,
                RoutedEventKind::PointerLeft {
                    device_id,
                    position,
                    related_target: target,
                },
            )?;
        }
        if let Some(target) = target {
            self.dispatch_routed(
                target,
                RoutedEventKind::PointerEntered {
                    device_id,
                    position,
                    related_target: old,
                },
            )?;
        }
        self.dirty |= Dirty::PAINT;
        Ok(())
    }

    pub(crate) fn route_to(&self, target: ElementId) -> Result<Vec<ElementId>, UiError> {
        let mut route = Vec::new();
        let mut current = Some(target);
        while let Some(id) = current {
            route.push(id);
            current = self.node(id)?.parent;
        }
        route.reverse();
        Ok(route)
    }

    pub(crate) fn world_transform_for(&self, target: ElementId) -> Result<Affine2, UiError> {
        let mut world = Affine2::IDENTITY;
        for current in self.route_to(target)? {
            let node = self.node(current)?;
            world *= node_local_transform(node);
            if let Kind::ScrollView { offset, .. } = node.kind {
                world *= Affine2::from_translation(Vec2::new(0.0, -offset));
            }
        }
        Ok(world)
    }

    pub(crate) fn set_focus(&mut self, target: Option<ElementId>) -> Result<(), UiError> {
        let target = target.filter(|id| {
            self.node(*id)
                .is_ok_and(|_| self.is_effectively_interactive(*id) && self.is_focusable_id(*id))
        });
        if self.focus == target {
            return Ok(());
        }
        if let Some(old) = self.focus {
            self.events.push_back(UiEvent {
                target: old,
                kind: UiEventKind::FocusChanged(false),
            });
            if let Ok(node) = self.node_mut(old)
                && let Kind::TextField(field) = &mut node.kind
            {
                field.preedit.clear();
            }
            self.dispatch_routed(old, RoutedEventKind::FocusChanged(false))?;
        }
        self.focus = target;
        if let Some(target) = target {
            self.events.push_back(UiEvent {
                target,
                kind: UiEventKind::FocusChanged(true),
            });
            self.dispatch_routed(target, RoutedEventKind::FocusChanged(true))?;
            self.reveal_focused_descendant(target)?;
        }
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        Ok(())
    }

    pub(crate) fn move_focus(&mut self, forward: bool) -> Result<(), UiError> {
        let trapped = self.focus.and_then(|focus| {
            self.route_to(focus).ok()?.into_iter().rev().find(|id| {
                self.node(*id).is_ok_and(|node| matches!(node.kind, Kind::FocusScope { options, .. } if options.trapped) || matches!(node.kind, Kind::Overlay { options, .. } if options.focus.trapped))
            })
        });
        let focusable = self
            .ids()
            .filter(|id| {
                self.is_effectively_interactive(*id)
                    && self.is_focusable_id(*id)
                    && trapped.is_none_or(|scope| self.is_descendant_of(*id, scope))
            })
            .collect::<Vec<_>>();
        if focusable.is_empty() {
            return self.set_focus(None);
        }
        let index = self
            .focus
            .and_then(|focus| focusable.iter().position(|id| *id == focus));
        let next = match (index, forward) {
            (Some(index), true) => focusable[(index + 1) % focusable.len()],
            (Some(0), false) | (None, false) => *focusable.last().expect("not empty"),
            (Some(index), false) => focusable[index - 1],
            (None, true) => focusable[0],
        };
        self.set_focus(Some(next))
    }

    pub(crate) fn is_effectively_interactive(&self, id: ElementId) -> bool {
        self.route_to(id).is_ok_and(|route| {
            route.into_iter().all(|id| {
                self.node(id)
                    .is_ok_and(|node| node.enabled && node.visibility == Visibility::Visible)
            })
        })
    }

    pub(crate) fn is_focusable_id(&self, id: ElementId) -> bool {
        self.node(id).is_ok_and(|node| match node.kind {
            Kind::Button { .. }
            | Kind::TextField(_)
            | Kind::Checkbox { .. }
            | Kind::Slider { .. }
            | Kind::ScrollView { .. } => true,
            Kind::Custom => self
                .custom_widgets
                .get(&id)
                .is_some_and(|widget| widget.focusable()),
            _ => false,
        })
    }

    pub(crate) fn is_hit_testable_id(&self, id: ElementId) -> bool {
        self.node(id).is_ok_and(|node| match node.kind {
            Kind::Button { .. }
            | Kind::TextField(_)
            | Kind::Checkbox { .. }
            | Kind::Slider { .. }
            | Kind::ScrollView { .. } => true,
            Kind::Custom => self
                .custom_widgets
                .get(&id)
                .is_some_and(|widget| widget.hit_testable()),
            _ => false,
        })
    }
}
