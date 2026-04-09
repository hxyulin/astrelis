//! Monitor and video mode types.

use astrelis_core::geometry::{Physical, Point, Size};

/// An opaque identifier for a monitor/display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MonitorId(pub(crate) u64);

impl MonitorId {
    /// Creates a new monitor ID from a raw value.
    ///
    /// This is intended for backend implementations only.
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the raw value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// Information about a connected monitor/display.
#[derive(Clone, Debug)]
pub struct MonitorInfo {
    /// Opaque identifier for this monitor.
    pub id: MonitorId,
    /// Human-readable name (e.g., "DELL U2720Q"). `None` if unknown.
    pub name: Option<String>,
    /// Position of the monitor's top-left corner in the virtual screen
    /// coordinate space, in physical pixels.
    pub position: Point<Physical>,
    /// Physical size of the monitor's display area in pixels.
    pub size: Size<Physical>,
    /// The scale factor (DPI scaling) for this monitor.
    /// 1.0 = standard DPI; 2.0 = Retina / 200% scaling.
    pub scale_factor: f32,
    /// Available video modes for fullscreen exclusive use.
    pub video_modes: Vec<VideoMode>,
}

/// A video mode (resolution + refresh rate + bit depth) for exclusive fullscreen.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VideoMode {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
    /// Refresh rate in millihertz (e.g., 60000 = 60 Hz).
    pub refresh_rate_millihertz: u32,
    /// Bit depth per color channel.
    pub bit_depth: u16,
}
