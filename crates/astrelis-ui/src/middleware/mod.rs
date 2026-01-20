//! Middleware system for UI pipeline extensibility.
//!
//! The middleware system provides hooks into the UI rendering pipeline,
//! enabling debug overlays, inspector integration, and custom extensions.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                         UiSystem                                     │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌──────────────┐    ┌───────────────┐    ┌──────────────────────┐  │
//! │  │   UiCore     │    │ UiRenderer    │    │  MiddlewareManager   │  │
//! │  └──────────────┘    └───────────────┘    └──────────────────────┘  │
//! │         │                   │                        │               │
//! │         ▼                   ▼                        ▼               │
//! │  ┌──────────────────────────────────────────────────────────────┐   │
//! │  │                    Render Pipeline                            │   │
//! │  │  1. pre_layout()    → Can PAUSE layout                       │   │
//! │  │  2. compute_layout() (if not paused)                         │   │
//! │  │  3. post_layout()                                            │   │
//! │  │  4. pre_render()                                             │   │
//! │  │  5. render_ui()                                              │   │
//! │  │  6. post_render()   → Draw overlays (dirty flags, bounds)    │   │
//! │  │  7. render_overlays() (OverlayRenderer)                      │   │
//! │  └──────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::middleware::{UiMiddleware, MiddlewareContext, OverlayContext};
//!
//! struct MyDebugMiddleware {
//!     enabled: bool,
//! }
//!
//! impl UiMiddleware for MyDebugMiddleware {
//!     fn name(&self) -> &'static str { "my_debug" }
//!
//!     fn post_render(&mut self, ctx: &MiddlewareContext, overlay: &mut OverlayContext) {
//!         if !self.enabled { return; }
//!         overlay.draw_text(Vec2::new(10.0, 10.0), "Debug Active", Color::GREEN, 16.0);
//!     }
//! }
//! ```

mod context;
mod inspector;
mod keybind;
mod manager;
mod overlay_draw_list;
mod overlay_renderer;

pub use context::{MiddlewareContext, OverlayContext};
pub use inspector::InspectorMiddleware;
pub use keybind::{Keybind, KeybindRegistry, Modifiers};
pub use manager::MiddlewareManager;
pub use overlay_draw_list::{OverlayCommand, OverlayDrawList, OverlayLine, OverlayQuadCmd, OverlayText};
pub use overlay_renderer::OverlayRenderer;

use crate::tree::UiTree;

/// Trait for UI middleware that can hook into the rendering pipeline.
///
/// Middlewares receive callbacks at various points in the render cycle and
/// can optionally draw debug overlays on top of the UI.
pub trait UiMiddleware: Send + Sync {
    /// Unique name for this middleware.
    fn name(&self) -> &'static str;

    /// Priority for ordering (higher = later/renders on top). Default: 0
    fn priority(&self) -> i32 {
        0
    }

    /// Called before layout computation.
    ///
    /// Return `true` to PAUSE/SKIP layout computation this frame.
    /// This is useful for debugging dirty flags in their pre-layout state.
    fn pre_layout(&mut self, _ctx: &MiddlewareContext) -> bool {
        false
    }

    /// Called after layout computation completes.
    fn post_layout(&mut self, _ctx: &MiddlewareContext) {}

    /// Called before main UI rendering begins.
    fn pre_render(&mut self, _ctx: &MiddlewareContext) {}

    /// Called after main UI rendering completes.
    ///
    /// This is the primary hook for drawing debug overlays.
    fn post_render(&mut self, _ctx: &MiddlewareContext, _overlay: &mut OverlayContext) {}

    /// Handle a keybind that was triggered.
    ///
    /// Return `true` if the keybind was consumed and should not be passed
    /// to other middlewares or the UI.
    fn handle_keybind(&mut self, _keybind: &Keybind, _ctx: &MiddlewareContext) -> bool {
        false
    }

    /// Handle a keyboard event directly.
    ///
    /// Called for all key events, not just registered keybinds.
    /// Return `true` if the event was consumed.
    fn handle_key_event(
        &mut self,
        _key: astrelis_winit::event::KeyCode,
        _modifiers: Modifiers,
        _pressed: bool,
        _ctx: &MiddlewareContext,
    ) -> bool {
        false
    }

    /// Update middleware state.
    ///
    /// Called every frame before layout/render.
    fn update(&mut self, _ctx: &MiddlewareContext, _tree: &UiTree) {}

    /// Check if this middleware is enabled.
    fn is_enabled(&self) -> bool {
        true
    }

    /// Enable or disable this middleware.
    fn set_enabled(&mut self, _enabled: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestMiddleware {
        enabled: bool,
        pre_layout_called: bool,
        should_pause: bool,
    }

    impl UiMiddleware for TestMiddleware {
        fn name(&self) -> &'static str {
            "test"
        }

        fn priority(&self) -> i32 {
            100
        }

        fn pre_layout(&mut self, _ctx: &MiddlewareContext) -> bool {
            self.pre_layout_called = true;
            self.should_pause
        }

        fn is_enabled(&self) -> bool {
            self.enabled
        }

        fn set_enabled(&mut self, enabled: bool) {
            self.enabled = enabled;
        }
    }

    #[test]
    fn test_middleware_trait() {
        let mut middleware = TestMiddleware {
            enabled: true,
            pre_layout_called: false,
            should_pause: false,
        };

        assert_eq!(middleware.name(), "test");
        assert_eq!(middleware.priority(), 100);
        assert!(middleware.is_enabled());

        middleware.set_enabled(false);
        assert!(!middleware.is_enabled());
    }
}
