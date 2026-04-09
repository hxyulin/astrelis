//! Window stacking level types.

/// The stacking level of a window relative to other windows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WindowLevel {
    /// Below normal windows (behind everything).
    AlwaysOnBottom,
    /// Normal stacking order.
    #[default]
    Normal,
    /// Above normal windows (always on top).
    AlwaysOnTop,
}
