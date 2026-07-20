//! Ergonomic Layer-5 facade over `astrelis-ui-core`.
//!
//! `astrelis-ui-core` is the retained low-level engine: every element is
//! created and configured through fallible calls, layout is a fifteen-field
//! struct, and listeners take a phase, a filter, and a raw routed event. That
//! surface is correct and stays the escape hatch, but on the construction path
//! it is verbose. This crate adds, without changing the core's shape:
//!
//! - [`Build`] — infallible, chainable node creation that commits a node's
//!   layout, flex, style, wrapping, and enablement in one chain
//!   ([`Node::finish`]);
//! - [`LayoutExt`] with [`px`]/[`percent`]/[`layout`] — fluent
//!   [`LayoutStyle`](astrelis_ui_core::LayoutStyle);
//! - [`On`] — intent-named listeners (`on_click`, `on_checked`, `on_slider`,
//!   `on_text_changed`, `on_text_submitted`);
//! - [`widget_any`] — stamps the `as_any`/`as_any_mut` boilerplate every
//!   [`Widget`](astrelis_ui_core::Widget) must otherwise hand-write;
//! - a [`prelude`] gathering the names a typical screen needs.
//!
//! ```ignore
//! use astrelis_ui::prelude::*;
//!
//! let scroll = ui.padding(root, Insets::all(28.0)).grow(1.0)
//!     .scroll_view().grow(1.0).finish();
//! let toggle = ui.checkbox(scroll, true).finish();
//! ui.on_checked(toggle, |ctx, on| ctx.emit(Message::Toggled(on)));
//! ```

#![warn(missing_docs)]

mod builder;
mod events;
mod layout;

pub use builder::{Build, Node};
pub use events::On;
pub use layout::{LayoutExt, layout, percent, px};

// Re-export the core and its companion crates so a facade user needs one import
// root. The core stays fully usable directly for anything the facade omits.
pub use astrelis_ui_core;

/// Implements the `as_any`/`as_any_mut` methods required by
/// [`astrelis_ui_core::Widget`].
///
/// Every custom widget writes the same two identity casts. Place this at the
/// top of a `Widget` impl and provide only the behaviour that differs:
///
/// ```ignore
/// impl Widget<Message> for Gauge {
///     astrelis_ui::widget_any!();
///     fn paint(&self, painter: &mut Painter, bounds: LogicalRect, theme: &Theme)
///         -> Result<(), UiError> { /* ... */ Ok(()) }
/// }
/// ```
#[macro_export]
macro_rules! widget_any {
    () => {
        fn as_any(&self) -> &dyn ::core::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn ::core::any::Any {
            self
        }
    };
}

/// The names a typical screen needs, in one glob import.
pub mod prelude {
    pub use crate::{Build, LayoutExt, Node, On, layout, percent, px, widget_any};

    pub use astrelis_ui_core::{
        Alignment, Button, Checkbox, Column, Edges, ElementHandle, EventContext, FlexStyle,
        FlexWrap, FocusScope, FocusScopeOptions, Insets, Justification, Label, LayoutStyle, Length,
        MountContext, Overflow, Overlay, OverlayOptions, Padding, Positioning, Row, ScrollView,
        SemanticRole, Slider, Stack, TextField, Theme, Ui, UiError, Visibility, Widget,
        WidgetStyle,
    };

    pub use astrelis_core::{
        color::Color,
        geometry::{LogicalPoint, LogicalRect, LogicalSize, Point},
        math::Affine2,
    };

    pub use astrelis_platform::CursorIcon;
}
