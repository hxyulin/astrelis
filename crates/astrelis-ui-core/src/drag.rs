//! Drag-and-drop session lifecycle.

use super::*;

impl<Message: 'static> Ui<Message> {
    /// Routes a drag owned by another retained UI tree over this tree.
    ///
    /// # Work in progress
    ///
    /// This is destination-side plumbing only. Application shells must route
    /// captured source-window coordinates to the destination tree; Astrelis
    /// does not yet provide a platform-native drag-and-drop session.
    ///
    /// Application shells use this when a pointer crosses native windows.
    /// The payload remains application-owned and cloneable; accepted targets
    /// receive the same routed enter/over/drop events as an in-tree drag.
    pub fn update_external_drag(
        &mut self,
        session_id: DragSessionId,
        device_id: DeviceId,
        position: LogicalPoint,
        payload: DragPayload,
        allowed: DragOperations,
    ) -> Result<Option<DropOperation>, UiError> {
        self.ensure_layout()?;
        let mut session =
            self.external_drag_sessions
                .remove(&session_id)
                .unwrap_or(ExternalDragSession {
                    device_id,
                    payload,
                    allowed,
                    candidate: None,
                    accepted: None,
                });
        session.device_id = device_id;
        session.allowed = allowed;
        let candidate = self.hit_test(position);
        if candidate != session.candidate {
            if let Some(previous) = session.candidate.filter(|id| self.node(*id).is_ok()) {
                self.dispatch_routed(
                    previous,
                    RoutedEventKind::DragLeft {
                        session: session_id,
                        device_id,
                        position,
                        payload: session.payload.clone(),
                    },
                )?;
            }
            if let Some(candidate) = candidate {
                self.dispatch_routed(
                    candidate,
                    RoutedEventKind::DragEntered {
                        session: session_id,
                        device_id,
                        position,
                        payload: session.payload.clone(),
                        allowed,
                    },
                )?;
            }
            session.candidate = candidate;
        }
        self.drop_acceptance = None;
        if let Some(candidate) = candidate {
            self.dispatch_routed(
                candidate,
                RoutedEventKind::DragOver {
                    session: session_id,
                    device_id,
                    position,
                    payload: session.payload.clone(),
                    allowed,
                },
            )?;
        }
        session.accepted = self
            .drop_acceptance
            .take()
            .and_then(|(device, target, operation)| {
                (device == device_id && allowed.contains(operation.flag()))
                    .then_some((target, operation))
            });
        let accepted = session.accepted.map(|(_, operation)| operation);
        self.external_drag_sessions.insert(session_id, session);
        self.dirty |= Dirty::PAINT;
        Ok(accepted)
    }

    /// Removes hover state for a cross-window drag that left this UI tree.
    pub fn leave_external_drag(
        &mut self,
        session_id: DragSessionId,
        position: LogicalPoint,
    ) -> Result<bool, UiError> {
        let Some(session) = self.external_drag_sessions.remove(&session_id) else {
            return Ok(false);
        };
        if let Some(candidate) = session.candidate.filter(|id| self.node(*id).is_ok()) {
            self.dispatch_routed(
                candidate,
                RoutedEventKind::DragLeft {
                    session: session_id,
                    device_id: session.device_id,
                    position,
                    payload: session.payload,
                },
            )?;
        }
        self.dirty |= Dirty::PAINT;
        Ok(true)
    }

    /// Drops a cross-window drag on the currently accepted target.
    pub fn finish_external_drag(
        &mut self,
        session_id: DragSessionId,
        position: LogicalPoint,
    ) -> Result<Option<DropOperation>, UiError> {
        let Some(session) = self.external_drag_sessions.remove(&session_id) else {
            return Ok(None);
        };
        if let Some(candidate) = session.candidate.filter(|id| self.node(*id).is_ok()) {
            self.dispatch_routed(
                candidate,
                RoutedEventKind::DragLeft {
                    session: session_id,
                    device_id: session.device_id,
                    position,
                    payload: session.payload.clone(),
                },
            )?;
        }
        let Some((target, operation)) = session.accepted else {
            self.dirty |= Dirty::PAINT;
            return Ok(None);
        };
        self.dispatch_routed(
            target,
            RoutedEventKind::Dropped {
                session: session_id,
                device_id: session.device_id,
                position,
                payload: session.payload,
                operation,
            },
        )?;
        self.dirty |= Dirty::PAINT;
        Ok(Some(operation))
    }

    pub(crate) fn update_drag(
        &mut self,
        device_id: DeviceId,
        position: LogicalPoint,
    ) -> Result<(), UiError> {
        let Some(mut session) = self.drag_sessions.remove(&device_id) else {
            return Ok(());
        };
        if !session.active {
            let delta = Vec2::new(position.x - session.start.x, position.y - session.start.y);
            if delta.length() < session.options.threshold {
                self.drag_sessions.insert(device_id, session);
                return Ok(());
            }
            session.active = true;
            self.dispatch_routed(
                session.source,
                RoutedEventKind::DragStarted {
                    session: session.id,
                    device_id,
                    position,
                    payload: session.payload.clone(),
                    allowed: session.options.allowed,
                },
            )?;
        }

        let candidate = self.hit_test(position);
        if candidate != session.candidate {
            if let Some(previous) = session.candidate {
                self.dispatch_routed(
                    previous,
                    RoutedEventKind::DragLeft {
                        session: session.id,
                        device_id,
                        position,
                        payload: session.payload.clone(),
                    },
                )?;
            }
            if let Some(candidate) = candidate {
                self.dispatch_routed(
                    candidate,
                    RoutedEventKind::DragEntered {
                        session: session.id,
                        device_id,
                        position,
                        payload: session.payload.clone(),
                        allowed: session.options.allowed,
                    },
                )?;
            }
            session.candidate = candidate;
        }

        self.drop_acceptance = None;
        if let Some(candidate) = candidate {
            self.dispatch_routed(
                candidate,
                RoutedEventKind::DragOver {
                    session: session.id,
                    device_id,
                    position,
                    payload: session.payload.clone(),
                    allowed: session.options.allowed,
                },
            )?;
        }
        session.accepted = self
            .drop_acceptance
            .take()
            .and_then(|(device, target, operation)| {
                (device == device_id && session.options.allowed.contains(operation.flag()))
                    .then_some((target, operation))
            });
        self.drag_sessions.insert(device_id, session);
        self.dirty |= Dirty::PAINT;
        Ok(())
    }

    pub(crate) fn finish_drag(
        &mut self,
        device_id: DeviceId,
        position: LogicalPoint,
    ) -> Result<bool, UiError> {
        let Some(session) = self.drag_sessions.remove(&device_id) else {
            return Ok(false);
        };
        if !session.active {
            return Ok(false);
        }
        if let Some(candidate) = session
            .candidate
            .filter(|target| self.node(*target).is_ok())
        {
            self.dispatch_routed(
                candidate,
                RoutedEventKind::DragLeft {
                    session: session.id,
                    device_id,
                    position,
                    payload: session.payload.clone(),
                },
            )?;
        }
        let outcome = if let Some((target, operation)) = session.accepted {
            self.dispatch_routed(
                target,
                RoutedEventKind::Dropped {
                    session: session.id,
                    device_id,
                    position,
                    payload: session.payload.clone(),
                    operation,
                },
            )?;
            DragOutcome::Dropped(operation)
        } else {
            DragOutcome::Cancelled
        };
        self.dispatch_routed(
            session.source,
            RoutedEventKind::DragEnded {
                session: session.id,
                device_id,
                outcome,
            },
        )?;
        if let Ok(node) = self.node_mut(session.source) {
            node.pressed = false;
        }
        self.capture.remove(&device_id);
        self.dirty |= Dirty::PAINT;
        Ok(true)
    }

    pub(crate) fn cancel_drag_id(&mut self, device_id: DeviceId) -> Result<(), UiError> {
        let Some(session) = self.drag_sessions.remove(&device_id) else {
            return Ok(());
        };
        if session.active
            && let Some(candidate) = session
                .candidate
                .filter(|target| self.node(*target).is_ok())
        {
            let position = self
                .pointer_positions
                .get(&device_id)
                .copied()
                .unwrap_or(session.start);
            self.dispatch_routed(
                candidate,
                RoutedEventKind::DragLeft {
                    session: session.id,
                    device_id,
                    position,
                    payload: session.payload.clone(),
                },
            )?;
        }
        if session.active && self.node(session.source).is_ok() {
            self.dispatch_routed(
                session.source,
                RoutedEventKind::DragEnded {
                    session: session.id,
                    device_id,
                    outcome: DragOutcome::Cancelled,
                },
            )?;
        }
        if let Ok(node) = self.node_mut(session.source) {
            node.pressed = false;
        }
        self.capture.remove(&device_id);
        self.dirty |= Dirty::PAINT;
        Ok(())
    }
}
