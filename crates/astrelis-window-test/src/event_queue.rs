//! Scripted event queue for the mock backend.

use astrelis_window::event::{DeviceEvent, WindowEvent};
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::window_id::WindowId;

/// A single scripted event to be injected into the mock backend.
#[derive(Clone, Debug)]
pub enum ScriptedEvent {
    /// A lifecycle state change.
    Lifecycle(AppLifecycle),
    /// A window event delivered to a specific window.
    Window {
        /// The target window.
        id: WindowId,
        /// The event.
        event: WindowEvent,
    },
    /// A device-level event (not tied to any window).
    Device(DeviceEvent),
    /// Signals that all pending events in this batch have been dispatched.
    /// Triggers [`AppHandler::on_events_cleared`](astrelis_window::backend::AppHandler::on_events_cleared).
    EventsCleared,
}
