//! Cursor types.

/// Standard system cursor icons.
///
/// A superset covering winit, SDL3, and GLFW cursor icons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CursorIcon {
    /// The platform-dependent default cursor (usually an arrow).
    #[default]
    Default,

    // Pointer
    /// A context menu is available.
    ContextMenu,
    /// Help information is available.
    Help,
    /// Pointer indicating a link (pointing hand).
    Pointer,
    /// The program is busy in the background but the user can still interact.
    Progress,
    /// The program is busy; user cannot interact.
    Wait,

    // Selection
    /// Table cell or set of cells can be selected.
    Cell,
    /// Text can be selected (I-beam).
    Text,
    /// Vertical text can be selected.
    VerticalText,
    /// A crosshair for fine selection.
    Crosshair,

    // Drag & drop
    /// An alias or shortcut is to be created.
    Alias,
    /// Something is to be copied.
    Copy,
    /// An item may be moved.
    Move,
    /// Something cannot be dropped here.
    NoDrop,
    /// The action is not allowed.
    NotAllowed,
    /// Something can be grabbed.
    Grab,
    /// Something is being grabbed.
    Grabbing,

    // Resize (edge)
    /// Resize east (right edge).
    EResize,
    /// Resize north (top edge).
    NResize,
    /// Resize northeast (top-right corner).
    NeResize,
    /// Resize northwest (top-left corner).
    NwResize,
    /// Resize south (bottom edge).
    SResize,
    /// Resize southeast (bottom-right corner).
    SeResize,
    /// Resize southwest (bottom-left corner).
    SwResize,
    /// Resize west (left edge).
    WResize,

    // Resize (bidirectional)
    /// Bidirectional east-west resize.
    EwResize,
    /// Bidirectional north-south resize.
    NsResize,
    /// Bidirectional northeast-southwest resize.
    NeswResize,
    /// Bidirectional northwest-southeast resize.
    NwseResize,

    // Scroll
    /// Column can be resized horizontally.
    ColResize,
    /// Row can be resized vertically.
    RowResize,
    /// Panning in all directions.
    AllScroll,

    // Zoom
    /// Zoom in.
    ZoomIn,
    /// Zoom out.
    ZoomOut,
}

/// How the cursor is constrained to the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorGrabMode {
    /// No grab: cursor moves freely.
    #[default]
    None,
    /// The cursor is confined within the window bounds but still visible.
    /// Falls back to `Locked` on platforms that don't support confinement.
    Confined,
    /// The cursor is locked in place and hidden; only deltas are reported.
    /// This is the mode for first-person camera control.
    Locked,
}
