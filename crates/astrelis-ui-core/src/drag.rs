//! Drag-and-drop session lifecycle.

use super::*;

impl<Message: 'static> Ui<Message> {
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
