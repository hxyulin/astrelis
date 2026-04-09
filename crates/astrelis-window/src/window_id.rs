//! Window identification types.

/// Marker type for window IDs.
pub struct WindowIdMarker;

/// A unique identifier for a window.
///
/// Backed by [`astrelis_core::id::Id`] with a [`WindowIdMarker`] phantom type,
/// so window IDs cannot be mixed with other ID domains at compile time.
pub type WindowId = astrelis_core::id::Id<WindowIdMarker>;
