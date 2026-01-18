//! New capability-based widget system.
//!
//! This module provides a modern, type-safe widget system with:
//! - Capability-based traits instead of downcasting
//! - Typed handles with generational safety
//! - Efficient storage with O(1) lookup
//!
//! # Architecture
//!
//! ## Capability Traits
//!
//! Widgets expose their capabilities through traits:
//! - `Widget` - Base trait for all widgets
//! - `TextWidget` - Widgets that display text
//! - `ParentWidget` - Widgets that contain children
//! - `ColorWidget` - Widgets with background color
//! - `SizedWidget` - Widgets with explicit size
//! - `ClickableWidget` - Widgets that can be clicked
//!
//! ## Typed Handles
//!
//! Handles are Copy types that reference widgets:
//! - `WidgetHandle<T>` - Type-safe handle to widget of type T
//! - Generational safety prevents use-after-free
//! - Can upcast to trait objects
//! - Compile-time type checking
//!
//! ## Storage
//!
//! `WidgetStorage` provides:
//! - Efficient O(1) lookup by handle
//! - Generational safety (stale handles return None)
//! - Iterator support for traversal
//!
//! # Example
//!
//! ```rust,ignore
//! use astrelis_ui::widget::*;
//!
//! let mut storage = WidgetStorage::new();
//!
//! // Add widgets with typed handles
//! let button: WidgetHandle<Button> = storage.add(Button::new("Click"));
//! let text: WidgetHandle<Text> = storage.add(Text::new("Hello"));
//!
//! // Upcast to trait for generic operations
//! let text_handle: WidgetHandle<dyn TextWidget> = button.upcast();
//!
//! // Type-safe access
//! if let Some(widget) = storage.get_mut(text_handle) {
//!     widget.as_text_widget_mut().unwrap().set_text("Updated");
//! }
//! ```

pub mod capability;
pub mod handle;
pub mod impls;
pub mod storage;

// Re-export main types
pub use capability::*;
pub use handle::*;
pub use impls::*;
pub use storage::*;
