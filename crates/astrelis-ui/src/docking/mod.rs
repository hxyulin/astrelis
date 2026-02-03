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

pub mod types;
pub mod splitter;
pub mod tabs;
pub mod drag;

// Re-export main types
pub use types::{
    DockZone, DragState, DragType, PanelConstraints, SplitDirection, DRAG_THRESHOLD,
    calculate_separator_bounds, calculate_panel_layouts,
};
pub use splitter::DockSplitter;
pub use tabs::{
    DockTabs,
    DEFAULT_TAB_PADDING,
    DEFAULT_CLOSE_BUTTON_SIZE,
};
pub use drag::DragManager;
