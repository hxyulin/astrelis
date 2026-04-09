//! Fullscreen mode types.

use crate::monitor::{MonitorId, VideoMode};

/// Fullscreen mode configuration.
#[derive(Clone, Debug, PartialEq)]
pub enum FullscreenMode {
    /// Borderless fullscreen on a specific monitor.
    /// If `None`, uses the monitor the window is currently on.
    Borderless(Option<MonitorId>),
    /// Exclusive fullscreen with a specific video mode on a specific monitor.
    Exclusive {
        /// The monitor to use.
        monitor: MonitorId,
        /// The video mode to set.
        video_mode: VideoMode,
    },
}
