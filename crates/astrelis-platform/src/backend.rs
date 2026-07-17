//! Unstable backend implementation contracts.

use std::fmt::Debug;

use crate::{
    Clipboard as ClipboardHandle, ControlFlow, EventLoopClosed, Monitor, PlatformError,
    WindowAttributes, WindowCapabilities, WindowId,
};

/// Backend text clipboard operations.
pub trait Clipboard: Debug + Send + Sync {
    /// Returns supported synchronous text operations.
    fn capabilities(&self) -> crate::ClipboardCapabilities;
    /// Reads text, returning `None` when the clipboard has no text representation.
    fn read_text(&self) -> Result<Option<String>, PlatformError>;
    /// Replaces the clipboard contents with text.
    fn write_text(&self, text: String) -> Result<(), PlatformError>;
}

/// Backend storage and operations for a native window.
pub trait Window:
    raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle + Debug + Send + Sync
{
    /// Stable Astrelis identifier.
    fn id(&self) -> WindowId;
    /// Current capabilities.
    fn capabilities(&self) -> WindowCapabilities;
    /// Executes a backend operation.
    fn command(
        &self,
        command: crate::WindowCommand,
    ) -> Result<Option<crate::WindowValue>, PlatformError>;
}

/// Backend storage for a typed proxy.
pub trait EventLoopProxy<T>: Debug + Send + Sync {
    /// Sends an event, returning ownership if the loop is closed.
    fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>>;
}

/// Operations available while an event loop is active.
pub trait ActiveContext<T> {
    /// Creates a window.
    fn create_window(
        &mut self,
        attributes: WindowAttributes,
    ) -> Result<crate::Window, PlatformError>;
    /// Changes the control flow.
    fn set_control_flow(&mut self, control_flow: ControlFlow);
    /// Returns the current control flow.
    fn control_flow(&self) -> ControlFlow;
    /// Enumerates monitors.
    fn available_monitors(&self) -> Vec<Monitor>;
    /// Returns the primary monitor.
    fn primary_monitor(&self) -> Option<Monitor>;
    /// Creates a proxy.
    fn event_loop_proxy(&self) -> crate::EventLoopProxy<T>;
    /// Returns the process clipboard handle.
    fn clipboard(&self) -> ClipboardHandle;
    /// Requests exit.
    fn exit(&mut self);
}
