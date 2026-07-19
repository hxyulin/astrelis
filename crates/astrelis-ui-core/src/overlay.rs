//! Focus-scope and overlay/portal placement options.

use astrelis_core::geometry::LogicalPoint;

/// Keyboard behavior of a focus scope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusScopeOptions {
    /// Keep Tab traversal inside this scope.
    pub trapped: bool,
    /// Focus the first eligible descendant when mounted.
    pub autofocus: bool,
    /// Restore the prior focus when removed.
    pub restore_focus: bool,
}

/// Side of an anchor used to place an overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlaySide {
    /// Below the anchor.
    #[default]
    Below,
    /// Above the anchor.
    Above,
    /// Left of the anchor.
    Left,
    /// Right of the anchor.
    Right,
    /// Centered over the anchor.
    Center,
}

/// Alignment along an overlay anchor's side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlayAlignment {
    /// Align leading edges.
    #[default]
    Start,
    /// Center along the side.
    Center,
    /// Align trailing edges.
    End,
}

/// Viewport-hosted overlay configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayOptions {
    /// Preferred side of the anchor.
    pub side: OverlaySide,
    /// Alignment along the chosen side.
    pub alignment: OverlayAlignment,
    /// Additional logical offset.
    pub offset: LogicalPoint,
    /// Keep the overlay inside the viewport: when the preferred side lacks
    /// room, the overlay first flips to the opposite side of the anchor if
    /// that side fits, and is otherwise slid along the overflowing axis.
    pub clamp_to_viewport: bool,
    /// Top-layer ordering.
    pub z_index: i32,
    /// Optional focus-scope behavior.
    pub focus: FocusScopeOptions,
}

impl Default for OverlayOptions {
    fn default() -> Self {
        Self {
            side: OverlaySide::Below,
            alignment: OverlayAlignment::Start,
            offset: LogicalPoint::ZERO,
            clamp_to_viewport: true,
            z_index: 0,
            focus: FocusScopeOptions::default(),
        }
    }
}
