use std::{error::Error, fmt};
use winit::window::WindowId;

/// Window creation, metrics, or presentation configuration failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum WindowError {
    /// The operating system rejected window creation.
    Native(winit::error::OsError),
    /// Astrelis rejected initialization or attachment configuration.
    Graphics(astrelis::Error),
    /// Native metrics cannot be represented as finite logical dimensions.
    Metrics(crate::InvalidWindowMetrics),
    /// Register windows during an active application lifecycle, after resume.
    Inactive,
    /// This native window is already registered.
    AlreadyManaged(WindowId),
}
impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native(e) => write!(f, "window creation failed: {e}"),
            Self::Graphics(e) => write!(f, "window graphics failed: {e}"),
            Self::Metrics(e) => e.fmt(f),
            Self::Inactive => f.write_str("window registration requires an active application"),
            Self::AlreadyManaged(id) => write!(f, "window {id:?} is already managed"),
        }
    }
}
impl Error for WindowError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Native(e) => Some(e),
            Self::Graphics(e) => Some(e),
            Self::Metrics(e) => Some(e),
            _ => None,
        }
    }
}
impl From<astrelis::Error> for WindowError {
    fn from(value: astrelis::Error) -> Self {
        Self::Graphics(value)
    }
}
impl From<crate::InvalidWindowMetrics> for WindowError {
    fn from(value: crate::InvalidWindowMetrics) -> Self {
        Self::Metrics(value)
    }
}
impl From<winit::error::OsError> for WindowError {
    fn from(value: winit::error::OsError) -> Self {
        Self::Native(value)
    }
}

/// A requested window is absent or has already been queued for close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownWindow(pub WindowId);
impl fmt::Display for UnknownWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown window {:?}", self.0)
    }
}
impl Error for UnknownWindow {}

/// Callback that returned an application error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Callback {
    /// Application resume.
    Resumed,
    /// Application suspension.
    Suspended,
    /// A newly registered window.
    WindowCreated,
    /// A native window event.
    WindowEvent,
    /// An application message.
    UserEvent,
    /// A device event.
    DeviceEvent,
    /// Preparation before acquisition.
    Prepare,
    /// Recording an acquired frame.
    Render,
    /// Notification after submission.
    Submitted,
    /// An OS close decision.
    CloseRequested,
    /// A window was removed.
    WindowClosed,
    /// Final application shutdown.
    Exiting,
}

/// Native runner failure, with the original error retained where available.
#[derive(Debug)]
#[non_exhaustive]
pub enum RunError {
    /// Invalid retry delay or recovery limit.
    InvalidOptions,
    /// Native event-loop failure.
    EventLoop(winit::error::EventLoopError),
    /// An application callback failed.
    Handler {
        /// Callback that failed.
        callback: Callback,
        /// Window associated with the callback, if any.
        window: Option<WindowId>,
        /// Original application error, boxed only on failure.
        source: Box<dyn Error>,
    },
    /// Window metrics, lifecycle, or surface recreation failed.
    Window {
        /// Associated native window.
        window: WindowId,
        /// Original window error.
        source: WindowError,
    },
    /// Acquisition returned a terminal error.
    Acquisition {
        /// Associated native window.
        window: WindowId,
        /// Original acquisition failure.
        source: astrelis::FrameError,
    },
    /// Consecutive surface losses exceeded the configured limit.
    RecoveryLimit {
        /// Associated native window.
        window: WindowId,
    },
    /// Finishing and submitting the recorded frame failed.
    Submission {
        /// Associated native window.
        window: WindowId,
        /// Original rendering failure.
        source: astrelis::Error,
    },
}
impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions => f.write_str(
                "retry delay must be positive and representable, and recovery limit nonzero",
            ),
            Self::EventLoop(e) => e.fmt(f),
            Self::Handler {
                callback,
                window,
                source,
            } => write!(f, "{callback:?} callback for {window:?} failed: {source}"),
            Self::Window { window, source } => write!(f, "window {window:?}: {source}"),
            Self::Acquisition { window, source } => {
                write!(f, "window {window:?} acquisition: {source}")
            }
            Self::RecoveryLimit { window } => write!(
                f,
                "window {window:?} exceeded consecutive surface recovery limit"
            ),
            Self::Submission { window, source } => {
                write!(f, "window {window:?} submission: {source}")
            }
        }
    }
}
impl Error for RunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::EventLoop(e) => Some(e),
            Self::Handler { source, .. } => Some(source.as_ref()),
            Self::Window { source, .. } => Some(source),
            Self::Acquisition { source, .. } => Some(source),
            Self::Submission { source, .. } => Some(source),
            _ => None,
        }
    }
}
