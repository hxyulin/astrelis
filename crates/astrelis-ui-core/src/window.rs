//! Window event intake and platform state synchronization.

use super::*;

impl<Message: 'static> Ui<Message> {
    /// Routes one platform window event through the retained UI tree.
    pub fn handle_window_event(
        &mut self,
        window: &Window,
        clipboard: &Clipboard,
        event: &WindowEvent,
    ) -> Result<UiUpdate, UiError> {
        let was_dirty = self.needs_redraw();
        let mut platform_state_changed = false;
        match event {
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = *modifiers,
            WindowEvent::Focused(focused) => {
                self.window_focused = *focused;
                if !focused {
                    let dragging = self.drag_sessions.keys().copied().collect::<Vec<_>>();
                    for device_id in dragging {
                        self.cancel_drag_id(device_id)?;
                    }
                    self.capture.clear();
                    for index in 0..self.slots.len() {
                        let Some(id) = self.id_at(index) else {
                            continue;
                        };
                        if let Ok(node) = self.node_mut(id) {
                            node.pressed = false;
                        }
                    }
                    self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
                }
                platform_state_changed = true;
            }
            WindowEvent::PointerMoved {
                device_id,
                position,
            } => {
                self.ensure_layout()?;
                let logical = Point::new(
                    position.x as f32 / self.scale_factor,
                    position.y as f32 / self.scale_factor,
                );
                self.pointer_positions.insert(*device_id, logical);
                let target = self
                    .capture
                    .get(device_id)
                    .copied()
                    .or_else(|| self.hit_test(logical));
                self.set_hover(*device_id, logical, self.hit_test(logical))?;
                if let Some(target) = target {
                    self.dispatch_routed(
                        target,
                        RoutedEventKind::PointerMoved {
                            device_id: *device_id,
                            position: logical,
                        },
                    )?;
                }
                self.update_drag(*device_id, logical)?;
                if let Some(target) = target
                    && self.capture.get(device_id) == Some(&target)
                {
                    if matches!(self.node(target)?.kind, Kind::TextField(_)) {
                        self.place_text_caret(target, logical, true)?;
                    } else if matches!(self.node(target)?.kind, Kind::Slider { .. }) {
                        self.set_slider_from_point(target, logical)?;
                    } else if matches!(self.node(target)?.kind, Kind::ScrollView { .. }) {
                        self.set_scroll_from_point(target, logical)?;
                    }
                }
            }
            WindowEvent::PointerLeft { device_id } => {
                let position = self
                    .pointer_positions
                    .get(device_id)
                    .copied()
                    .unwrap_or(LogicalPoint::ZERO);
                self.set_hover(*device_id, position, None)?;
            }
            WindowEvent::PointerButton {
                device_id,
                button: PointerButton::Primary,
                state,
            } => {
                self.ensure_layout()?;
                let position = self.pointer_positions.get(device_id).copied();
                match state {
                    ElementState::Pressed => {
                        let target = position.and_then(|point| self.hit_test(point));
                        if let Some(target) = target {
                            let prevented = self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerButton {
                                    device_id: *device_id,
                                    position: position
                                        .expect("hit target requires a pointer position"),
                                    button: PointerButton::Primary,
                                    state: ElementState::Pressed,
                                },
                            )?;
                            // Capture pairs this press with its release, so it is
                            // mechanism rather than a control default: a listener that
                            // prevents the default still has to receive the matching
                            // Released. The release path is already shaped this way and
                            // gates only the defaults on `prevented`.
                            self.capture.insert(*device_id, target);
                            self.node_mut(target)?.pressed = true;
                            if !prevented {
                                self.set_focus(Some(target))?;
                                if matches!(self.node(target)?.kind, Kind::TextField(_))
                                    && let Some(position) = position
                                {
                                    self.place_text_caret(target, position, self.modifiers.shift)?;
                                } else if matches!(self.node(target)?.kind, Kind::Slider { .. })
                                    && let Some(position) = position
                                {
                                    self.set_slider_from_point(target, position)?;
                                } else if matches!(self.node(target)?.kind, Kind::ScrollView { .. })
                                    && let Some(position) = position
                                {
                                    self.set_scroll_from_point(target, position)?;
                                }
                            }
                            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
                        } else {
                            self.set_focus(None)?;
                        }
                        platform_state_changed = true;
                    }
                    ElementState::Released => {
                        if self.finish_drag(*device_id, position.unwrap_or(LogicalPoint::ZERO))? {
                            self.sync_platform_state(window)?;
                            return Ok(UiUpdate {
                                redraw: true,
                                platform_state_changed,
                            });
                        }
                        if let Some(target) = self.capture.remove(device_id) {
                            let prevented = self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerButton {
                                    device_id: *device_id,
                                    position: position.unwrap_or(LogicalPoint::ZERO),
                                    button: PointerButton::Primary,
                                    state: ElementState::Released,
                                },
                            )?;
                            self.node_mut(target)?.pressed = false;
                            if !prevented
                                && position.and_then(|point| self.hit_test(point)) == Some(target)
                            {
                                if matches!(self.node(target)?.kind, Kind::Button { .. }) {
                                    if !self.dispatch_routed(target, RoutedEventKind::Activate)? {
                                        self.events.push_back(UiEvent {
                                            target,
                                            kind: UiEventKind::ButtonActivated,
                                        });
                                    }
                                } else if matches!(self.node(target)?.kind, Kind::Checkbox { .. }) {
                                    self.toggle_checkbox_id(target)?;
                                }
                            }
                            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput(input) if input.state == ElementState::Pressed => {
                if matches!(input.logical_key, Key::Named(NamedKey::Escape))
                    && !self.drag_sessions.is_empty()
                {
                    let dragging = self.drag_sessions.keys().copied().collect::<Vec<_>>();
                    for device_id in dragging {
                        self.cancel_drag_id(device_id)?;
                    }
                } else if matches!(input.logical_key, Key::Named(NamedKey::Tab)) {
                    self.move_focus(!self.modifiers.shift)?;
                    platform_state_changed = true;
                } else if let Some(focus) = self.focus {
                    if self.dispatch_routed(focus, RoutedEventKind::Keyboard(input.clone()))? {
                        self.sync_platform_state(window)?;
                        return Ok(UiUpdate {
                            redraw: self.needs_redraw(),
                            platform_state_changed,
                        });
                    }
                    match self.node(focus)?.kind {
                        Kind::Button { .. }
                            if matches!(
                                input.logical_key,
                                Key::Named(NamedKey::Enter | NamedKey::Space)
                            ) =>
                        {
                            if !self.dispatch_routed(focus, RoutedEventKind::Activate)? {
                                self.events.push_back(UiEvent {
                                    target: focus,
                                    kind: UiEventKind::ButtonActivated,
                                });
                            }
                            self.dirty |= Dirty::PAINT;
                        }
                        Kind::Checkbox { .. }
                            if matches!(input.logical_key, Key::Named(NamedKey::Space)) =>
                        {
                            self.toggle_checkbox_id(focus)?
                        }
                        Kind::Slider { .. } => self.handle_slider_key(focus, &input.logical_key)?,
                        Kind::ScrollView { .. } => {
                            self.handle_scroll_key(focus, &input.logical_key)?
                        }
                        Kind::TextField(_) => {
                            self.handle_text_key(focus, input, clipboard)?;
                            platform_state_changed = true;
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::PointerWheel {
                device_id, delta, ..
            } => {
                self.ensure_layout()?;
                if let Some(position) = self.pointer_positions.get(device_id).copied()
                    && let Some(target) = self.hit_test(position)
                {
                    let amount = match delta {
                        ScrollDelta::Lines { y, .. } => -*y * 40.0,
                        ScrollDelta::Pixels(point) => -(point.y as f32) / self.scale_factor,
                    };
                    if !self.dispatch_routed(
                        target,
                        RoutedEventKind::Scroll {
                            device_id: *device_id,
                            delta: Point::new(0.0, amount),
                        },
                    )? {
                        let mut current = Some(target);
                        while let Some(id) = current {
                            if matches!(self.node(id)?.kind, Kind::ScrollView { .. })
                                && self.scroll_by_id(id, amount)?
                            {
                                break;
                            }
                            current = self.node(id)?.parent;
                        }
                    }
                }
            }
            WindowEvent::Touch(touch) => {
                let device_id = DeviceId(touch.device_id.0 ^ touch.id.rotate_left(32));
                let logical = Point::new(
                    touch.position.x as f32 / self.scale_factor,
                    touch.position.y as f32 / self.scale_factor,
                );
                self.pointer_positions.insert(device_id, logical);
                match touch.phase {
                    TouchPhase::Started => {
                        if let Some(target) = self.hit_test(logical)
                            && !self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerButton {
                                    device_id,
                                    position: logical,
                                    button: PointerButton::Primary,
                                    state: ElementState::Pressed,
                                },
                            )?
                        {
                            self.set_focus(Some(target))?;
                            self.capture.insert(device_id, target);
                            self.node_mut(target)?.pressed = true;
                            if matches!(self.node(target)?.kind, Kind::Slider { .. }) {
                                self.set_slider_from_point(target, logical)?;
                            } else if matches!(self.node(target)?.kind, Kind::ScrollView { .. }) {
                                self.set_scroll_from_point(target, logical)?;
                            }
                        }
                    }
                    TouchPhase::Moved => {
                        if let Some(target) = self.capture.get(&device_id).copied() {
                            self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerMoved {
                                    device_id,
                                    position: logical,
                                },
                            )?;
                            self.update_drag(device_id, logical)?;
                            if matches!(self.node(target)?.kind, Kind::Slider { .. }) {
                                self.set_slider_from_point(target, logical)?;
                            } else if matches!(self.node(target)?.kind, Kind::ScrollView { .. }) {
                                self.set_scroll_from_point(target, logical)?;
                            }
                        }
                    }
                    TouchPhase::Ended => {
                        if self.finish_drag(device_id, logical)? {
                            self.dirty |= Dirty::PAINT;
                            self.sync_platform_state(window)?;
                            return Ok(UiUpdate {
                                redraw: true,
                                platform_state_changed,
                            });
                        }
                        if let Some(target) = self.capture.remove(&device_id) {
                            self.node_mut(target)?.pressed = false;
                            if !self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerButton {
                                    device_id,
                                    position: logical,
                                    button: PointerButton::Primary,
                                    state: ElementState::Released,
                                },
                            )? && self.hit_test(logical) == Some(target)
                            {
                                if matches!(self.node(target)?.kind, Kind::Checkbox { .. }) {
                                    self.toggle_checkbox_id(target)?;
                                } else if matches!(self.node(target)?.kind, Kind::Button { .. }) {
                                    self.dispatch_routed(target, RoutedEventKind::Activate)?;
                                }
                            }
                        }
                    }
                    TouchPhase::Cancelled => {
                        self.cancel_drag_id(device_id)?;
                        if let Some(target) = self.capture.remove(&device_id) {
                            self.node_mut(target)?.pressed = false;
                            self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerCancelled { device_id },
                            )?;
                        }
                    }
                }
                self.dirty |= Dirty::PAINT;
            }
            WindowEvent::Ime(ime) => {
                if let Some(focus) = self.focus
                    && matches!(self.node(focus)?.kind, Kind::TextField(_))
                {
                    if !self.dispatch_routed(focus, RoutedEventKind::Ime(ime.clone()))? {
                        self.handle_ime(focus, ime)?;
                    }
                    platform_state_changed = true;
                }
            }
            _ => {}
        }
        self.sync_platform_state(window)?;
        Ok(UiUpdate {
            redraw: self.needs_redraw() || (!was_dirty && self.dirty.contains(Dirty::PAINT)),
            platform_state_changed,
        })
    }

    pub(crate) fn sync_platform_state(&mut self, window: &Window) -> Result<(), UiError> {
        self.ensure_layout()?;
        let drag_cursor = self
            .drag_sessions
            .values()
            .find(|session| session.active)
            .map(|session| {
                if session.accepted.is_some() {
                    CursorIcon::Move
                } else {
                    CursorIcon::NotAllowed
                }
            });
        let cursor = drag_cursor.unwrap_or_else(|| {
            self.hover
                .and_then(|leaf| self.route_to(leaf).ok())
                .and_then(|route| {
                    route.into_iter().rev().find_map(|id| {
                        let node = self.node(id).ok()?;
                        node.cursor
                            .or_else(|| {
                                self.custom_widgets
                                    .get(&id)
                                    .and_then(|widget| widget.cursor_icon())
                            })
                            .or(match node.kind {
                                Kind::Button { .. } => Some(CursorIcon::Pointer),
                                Kind::TextField(_) => Some(CursorIcon::Text),
                                _ => None,
                            })
                    })
                })
                .unwrap_or(CursorIcon::Default)
        });
        if self.applied_cursor != Some(cursor) {
            window.set_cursor_icon(cursor);
            self.applied_cursor = Some(cursor);
        }
        let Some(focus) = self.focus.filter(|_| self.window_focused) else {
            window.set_ime_allowed(false);
            return Ok(());
        };
        let node = self.node(focus)?;
        let Kind::TextField(field) = &node.kind else {
            window.set_ime_allowed(false);
            return Ok(());
        };
        window.set_ime_allowed(true);
        window.set_ime_purpose(if field.password {
            ImePurpose::Password
        } else {
            ImePurpose::Normal
        });
        if let Some(layout) = &node.text_layout {
            let caret = layout.caret_rect(to_layout_position(field, field.caret), 1.0);
            window.set_ime_cursor_area(Rect::from_xywh(
                (node.bounds.origin.x + self.theme.control_padding.left + caret.origin.x
                    - field.horizontal_offset) as f64,
                (node.bounds.origin.y + self.theme.control_padding.top + caret.origin.y) as f64,
                caret.size.width.max(1.0) as f64,
                caret.size.height as f64,
            ));
        }
        Ok(())
    }
}
