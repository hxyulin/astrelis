//! Widget system for UI components.
//!
//! This module provides core widget types and builders for declarative UI construction.
//! Widgets are defined using a fluent builder API and rendered via GPU-accelerated
//! instanced rendering.
//!
//! # Core Widgets
//!
//! - **Text widgets** - Text rendering with font styling, alignment, and wrapping
//! - **Rectangle widgets** - Colored rectangles with borders and rounded corners
//! - **Image widgets** - Texture-based rendering with tinting
//! - **Container widgets** - Layout containers (Flexbox, Grid via Taffy)
//! - **Scroll containers** - Scrollable content with customizable scrollbars
//! - **Docking widgets** - Panel layout system (requires `docking` feature)
//!
//! # Example
//!
//! ```rust,no_run
//! # use astrelis_ui::Color;
//! # fn example(ui: &mut astrelis_ui::UiCore) {
//! ui.build(|root| {
//!     root.column()
//!         .child(|c| c.text("Hello World").color(Color::WHITE).size(24.0).build())
//!         .child(|c| c.button("Click me").width(100.0).height(50.0).build())
//!         .build();
//! });
//! # }
//! ```

pub mod base;
#[cfg(feature = "docking")]
pub mod docking;
pub mod scroll_container;
pub mod scrollbar;

pub use base::*;
pub use scroll_container::*;
pub use scrollbar::*;

use crate::style::Style;
use crate::tree::NodeId;
use astrelis_core::math::Vec2;
use astrelis_text::FontRenderer;
use std::any::Any;

/// Base trait for all UI widgets.
pub trait Widget: Any {
    /// Get widget type as Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Get mutable widget type as Any for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Get the style for this widget.
    fn style(&self) -> &Style;

    /// Get mutable style for this widget.
    fn style_mut(&mut self) -> &mut Style;

    /// Get child widgets.
    fn children(&self) -> &[NodeId] {
        &[]
    }

    /// Get mutable child widgets.
    ///
    /// Returns `None` for widgets that don't support children.
    /// Widgets with children (Container, Row, Column) override this to return `Some`.
    fn children_mut(&mut self) -> Option<&mut Vec<NodeId>> {
        None
    }

    /// Measure content size for layout (for intrinsic sizing).
    /// Returns (width, height) in pixels.
    fn measure(&self, _available_space: Vec2, _font_renderer: Option<&FontRenderer>) -> Vec2 {
        Vec2::ZERO
    }

    /// Clone the widget into a box.
    fn clone_box(&self) -> Box<dyn Widget>;
}

impl Clone for Box<dyn Widget> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
