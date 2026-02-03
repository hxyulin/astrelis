//! Event types for the plugin system.
//!
//! These types allow plugins to receive and handle UI input events
//! before per-widget-type dispatch.

use crate::tree::{NodeId, UiTree};
use astrelis_core::math::Vec2;

/// A processed UI input event passed to plugins.
#[derive(Debug, Clone)]
pub enum UiInputEvent {
    /// Mouse moved to a new position.
    MouseMove { position: Vec2 },
    /// Mouse button pressed.
    MouseDown {
        position: Vec2,
        button: MouseButtonKind,
    },
    /// Mouse button released.
    MouseUp {
        position: Vec2,
        button: MouseButtonKind,
    },
    /// Mouse wheel scrolled.
    Scroll { position: Vec2, delta: Vec2 },
    /// Keyboard key pressed.
    KeyDown { key: KeyEventData },
    /// Keyboard key released.
    KeyUp { key: KeyEventData },
    /// Character typed.
    CharInput { ch: char },
}

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButtonKind {
    Left,
    Right,
    Middle,
    Other(u16),
}

/// Keyboard event data.
#[derive(Debug, Clone)]
pub struct KeyEventData {
    /// The physical key code.
    pub physical_key: astrelis_winit::event::PhysicalKey,
    /// Whether this is a repeat event.
    pub is_repeat: bool,
}

/// Context provided to plugins during event handling.
///
/// Gives read/write access to the UI tree and relevant state
/// so plugins can query layout, modify widgets, etc.
pub struct PluginEventContext<'a> {
    /// The UI tree (read/write).
    pub tree: &'a mut UiTree,
    /// Current mouse position.
    pub mouse_position: Vec2,
    /// The node under the cursor (if any).
    pub hovered_node: Option<NodeId>,
}
