//! Application lifecycle and active event-loop context.

use std::{fmt, sync::Arc, time::Instant};

use crate::{
    Clipboard, DeviceEvent, DeviceId, EventLoopClosed, Monitor, PlatformError, StartCause, Window,
    WindowAttributes, WindowEvent, WindowId, backend,
};

/// Determines how the event loop waits for new work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlFlow {
    /// Continuously run the event loop.
    Poll,
    /// Sleep until an event arrives.
    #[default]
    Wait,
    /// Sleep until an event arrives or the deadline is reached.
    WaitUntil(Instant),
}

/// A typed, thread-safe event-loop wake handle.
pub struct EventLoopProxy<T> {
    inner: Arc<dyn backend::EventLoopProxy<T>>,
}

impl<T> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> fmt::Debug for EventLoopProxy<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EventLoopProxy")
            .finish_non_exhaustive()
    }
}

impl<T> EventLoopProxy<T> {
    /// Wraps a backend proxy.
    pub fn from_backend(inner: Arc<dyn backend::EventLoopProxy<T>>) -> Self {
        Self { inner }
    }

    /// Sends an event and wakes the event loop.
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.inner.send_event(event)
    }
}

/// Access to operations that require the active event-loop thread.
pub struct PlatformContext<'a, T> {
    inner: &'a mut dyn backend::ActiveContext<T>,
}

impl<'a, T> PlatformContext<'a, T> {
    /// Wraps an active backend context.
    pub fn from_backend(inner: &'a mut dyn backend::ActiveContext<T>) -> Self {
        Self { inner }
    }

    /// Creates a native window.
    pub fn create_window(&mut self, attributes: WindowAttributes) -> Result<Window, PlatformError> {
        self.inner.create_window(attributes)
    }

    /// Selects how the event loop should wait.
    pub fn set_control_flow(&mut self, control_flow: ControlFlow) {
        self.inner.set_control_flow(control_flow);
    }

    /// Returns the selected control flow.
    pub fn control_flow(&self) -> ControlFlow {
        self.inner.control_flow()
    }

    /// Returns all available monitors.
    pub fn available_monitors(&self) -> Vec<Monitor> {
        self.inner.available_monitors()
    }

    /// Returns the primary monitor when the platform identifies one.
    pub fn primary_monitor(&self) -> Option<Monitor> {
        self.inner.primary_monitor()
    }

    /// Returns a cross-thread event proxy.
    pub fn event_loop_proxy(&self) -> EventLoopProxy<T> {
        self.inner.event_loop_proxy()
    }

    /// Returns a cloneable text clipboard handle.
    pub fn clipboard(&self) -> Clipboard {
        self.inner.clipboard()
    }

    /// Requests event-loop termination.
    pub fn exit(&mut self) {
        self.inner.exit();
    }
}

/// An application driven by the Astrelis platform event loop.
pub trait Application: 'static {
    /// Typed events accepted from other threads.
    type UserEvent: Send + 'static;

    /// Called at the start of a new batch of events.
    fn new_events(
        &mut self,
        _context: &mut PlatformContext<'_, Self::UserEvent>,
        _cause: StartCause,
    ) {
    }
    /// Called when native resources may be created.
    fn resumed(&mut self, _context: &mut PlatformContext<'_, Self::UserEvent>) {}
    /// Called when native resources should be considered unavailable.
    fn suspended(&mut self, _context: &mut PlatformContext<'_, Self::UserEvent>) {}
    /// Delivers an event for a window.
    fn window_event(
        &mut self,
        _context: &mut PlatformContext<'_, Self::UserEvent>,
        _window: WindowId,
        _event: WindowEvent,
    ) {
    }
    /// Delivers a raw device event.
    fn device_event(
        &mut self,
        _context: &mut PlatformContext<'_, Self::UserEvent>,
        _device: DeviceId,
        _event: DeviceEvent,
    ) {
    }
    /// Delivers a typed proxy event.
    fn user_event(
        &mut self,
        _context: &mut PlatformContext<'_, Self::UserEvent>,
        _event: Self::UserEvent,
    ) {
    }
    /// Called immediately before the event loop waits.
    fn about_to_wait(&mut self, _context: &mut PlatformContext<'_, Self::UserEvent>) {}
    /// Called when the event loop is terminating.
    fn exiting(&mut self, _context: &mut PlatformContext<'_, Self::UserEvent>) {}
}
