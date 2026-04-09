//! Window error types.

use crate::capability::Capability;
use crate::window_id::WindowId;

/// Errors that can occur during window operations.
#[derive(Debug)]
pub enum WindowError {
    /// The windowing backend failed to initialize.
    BackendInitFailed(String),
    /// A window could not be created.
    WindowCreationFailed(String),
    /// The requested operation is not supported on this platform.
    Unsupported(Capability),
    /// The specified window ID does not correspond to any open window.
    InvalidWindowId(WindowId),
    /// The event loop encountered an error.
    EventLoopError(String),
    /// A monitor or video mode operation failed.
    MonitorError(String),
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendInitFailed(msg) => write!(f, "backend initialization failed: {msg}"),
            Self::WindowCreationFailed(msg) => write!(f, "window creation failed: {msg}"),
            Self::Unsupported(cap) => write!(f, "unsupported capability: {cap:?}"),
            Self::InvalidWindowId(id) => write!(f, "invalid window id: {id}"),
            Self::EventLoopError(msg) => write!(f, "event loop error: {msg}"),
            Self::MonitorError(msg) => write!(f, "monitor error: {msg}"),
        }
    }
}

impl std::error::Error for WindowError {}
