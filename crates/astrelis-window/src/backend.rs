//! Backend traits for windowing system integration.

use crate::builder::WindowAttributes;
use crate::capability::Capabilities;
use crate::control_flow::ControlFlow;
use crate::error::WindowError;
use crate::event::{DeviceEvent, WindowEvent};
use crate::lifecycle::AppLifecycle;
use crate::monitor::MonitorInfo;
use crate::window::Window;
use crate::window_id::WindowId;

/// Handler trait that users implement to receive events from the window backend.
///
/// Follows a callback pattern (similar to winit's `ApplicationHandler`) rather
/// than an iterator, because the backend controls the event loop lifetime on
/// most platforms.
pub trait AppHandler {
    /// Called when the application lifecycle state changes.
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle);

    /// Called for each window event.
    fn on_window_event(
        &mut self,
        ctx: &mut dyn EventLoopContext,
        window_id: WindowId,
        event: WindowEvent,
    );

    /// Called for device-level events not tied to a specific window.
    ///
    /// The most important event here is [`DeviceEvent::MouseMotion`], which
    /// provides raw mouse deltas when the cursor is locked — essential for
    /// first-person camera controls.
    ///
    /// Default implementation does nothing.
    fn on_device_event(
        &mut self,
        _ctx: &mut dyn EventLoopContext,
        _event: DeviceEvent,
    ) {
    }

    /// Called once per iteration after all pending events have been dispatched.
    ///
    /// This is the place to request redraws, update game state, set control
    /// flow, etc.
    fn on_events_cleared(&mut self, ctx: &mut dyn EventLoopContext);
}

/// Context provided to the [`AppHandler`] during event dispatch.
///
/// Allows the handler to create/destroy windows and control the event loop
/// without a direct reference to the backend.
pub trait EventLoopContext {
    /// Creates a new window with the given attributes.
    fn create_window(&mut self, attrs: WindowAttributes) -> Result<WindowId, WindowError>;

    /// Returns an immutable reference to a window by its ID.
    fn window(&self, id: WindowId) -> Option<&dyn Window>;

    /// Returns a mutable reference to a window by its ID.
    fn window_mut(&mut self, id: WindowId) -> Option<&mut dyn Window>;

    /// Destroys a window.
    fn destroy_window(&mut self, id: WindowId) -> Result<(), WindowError>;

    /// Sets the control flow mode for the event loop.
    fn set_control_flow(&mut self, flow: ControlFlow);

    /// Returns the current control flow mode.
    fn control_flow(&self) -> ControlFlow;

    /// Requests the event loop to exit after the current iteration completes.
    fn exit(&mut self);

    /// Returns the capabilities of this backend/platform.
    fn capabilities(&self) -> &Capabilities;

    /// Returns information about all connected monitors.
    fn monitors(&self) -> Vec<MonitorInfo>;

    /// Returns the primary monitor, if one can be determined.
    fn primary_monitor(&self) -> Option<MonitorInfo>;
}

/// Top-level backend entry point.
///
/// Each backend crate provides one implementation of this trait. Construct it,
/// then call [`run`](WindowBackend::run) with an [`AppHandler`].
pub trait WindowBackend: Sized {
    /// Creates a new backend instance, initializing the underlying platform
    /// windowing system.
    fn new() -> Result<Self, WindowError>;

    /// Runs the event loop, consuming the backend. This is the main blocking
    /// call.
    ///
    /// On most platforms this does not return (it exits the process). On
    /// platforms where it does return, it returns `Ok(())` on clean exit.
    fn run(self, handler: &mut dyn AppHandler) -> Result<(), WindowError>;
}
