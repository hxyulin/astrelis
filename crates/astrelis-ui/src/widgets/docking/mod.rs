//! Docking system for resizable split panels and tabbed containers.
//!
//! This module provides a minimal docking system with:
//! - **DockSplitter**: Resizable split container with a draggable separator
//! - **DockTabs**: Tabbed container showing one panel at a time
//! - **Drag System**: Extended event handling for drag operations
//!
//! # Quick Start
//!
//! ```ignore
//! use astrelis_ui::UiSystem;
//!
//! ui.build(|root| {
//!     root.hsplit()
//!         .width(800.0)
//!         .height(600.0)
//!         .split_ratio(0.3)
//!         .first(|left| {
//!             left.dock_tabs()
//!                 .tab("Explorer", |t| {
//!                     t.text("File tree...").build()
//!                 })
//!                 .tab("Search", |t| {
//!                     t.text("Search panel...").build()
//!                 })
//!                 .build()
//!         })
//!         .second(|right| {
//!             right.vsplit()
//!                 .split_ratio(0.6)
//!                 .first(|top| top.text("Top panel").build())
//!                 .second(|bottom| bottom.text("Bottom panel").build())
//!                 .build()
//!         })
//!         .build();
//! });
//! ```

pub mod animation;
pub mod context;
pub mod dock_node;
pub mod drag;
pub mod drop_zone;
pub mod operations;
pub mod plugin;
pub mod preview;
pub mod splitter;
pub mod tabs;
pub mod types;

// Re-export main types
pub use animation::{
    DockAnimationState, DropPreviewAnimation, GhostGroupAnimation, GhostTabAnimation,
    PanelTransition, SeparatorEase, TabReorderAnimation,
};
pub use context::{CachedContainerInfo, DockingContext, DockingStyle};
pub use dock_node::{DockNode, HitTestLayer};
pub use drag::DragManager;
pub use drop_zone::{DEFAULT_EDGE_THRESHOLD, DropTarget, DropZoneDetector};
pub use operations::{
    DockError, DockOperation, DockResult, MergeTabGroupOperation, MoveTabGroupOperation,
    SplitContainerOperation, TransferTabOperation, collapse_empty_container,
};
pub use preview::{
    DropPreview, DropPreviewStyle, default_preview_border_color, default_preview_color,
};
pub use splitter::DockSplitter;
pub use tabs::{
    DEFAULT_CLOSE_BUTTON_SIZE, DEFAULT_TAB_PADDING, DockTabs, TabScrollIndicator,
    TabScrollbarPosition, compute_all_tab_widths,
};
pub use types::{
    DRAG_THRESHOLD, DockZone, DragState, DragType, PanelConstraints, SplitDirection,
    calculate_panel_layouts, calculate_separator_bounds,
};
