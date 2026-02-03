//! Inspector middleware for UI debugging.
//!
//! Wraps the existing UiInspector and integrates it with the middleware system,
//! providing keybind-based controls and layout freeze functionality.

use astrelis_core::math::Vec2;
use astrelis_render::Color;
use astrelis_winit::event::KeyCode;

use crate::inspector::{InspectorConfig, UiInspector};
use crate::tree::UiTree;

use super::UiMiddleware;
use super::context::{MiddlewareContext, OverlayContext};
use super::keybind::{Keybind, Modifiers};

/// Inspector middleware for UI debugging and visualization.
///
/// Provides:
/// - Widget bounds visualization
/// - Dirty flag overlay
/// - Layout freeze functionality (pause layout to inspect dirty state)
/// - Widget selection and property inspection
///
/// # Default Keybinds
///
/// | Key    | Action                        |
/// |--------|-------------------------------|
/// | F12    | Toggle inspector on/off       |
/// | F5     | Toggle layout freeze          |
/// | F6     | Toggle dirty flag overlay     |
/// | F7     | Toggle bounds overlay         |
/// | Escape | Deselect current widget       |
pub struct InspectorMiddleware {
    /// The underlying inspector implementation.
    inspector: UiInspector,
    /// Whether the inspector is enabled.
    enabled: bool,
    /// Whether layout computation is frozen/paused.
    layout_frozen: bool,
    /// Frame number when layout was frozen (for display).
    frozen_at_frame: Option<u64>,
}

impl InspectorMiddleware {
    /// Create a new inspector middleware with default configuration.
    pub fn new() -> Self {
        Self::with_config(InspectorConfig::default())
    }

    /// Create a new inspector middleware with custom configuration.
    pub fn with_config(config: InspectorConfig) -> Self {
        Self {
            inspector: UiInspector::new(config),
            enabled: false,
            layout_frozen: false,
            frozen_at_frame: None,
        }
    }

    /// Toggle the inspector on/off.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if self.enabled {
            self.inspector.enable();
        } else {
            self.inspector.disable();
            // Also unfreeze when disabling
            self.layout_frozen = false;
            self.frozen_at_frame = None;
        }
    }

    /// Toggle layout freeze (pause layout computation).
    pub fn toggle_freeze(&mut self) {
        self.layout_frozen = !self.layout_frozen;
        if self.layout_frozen {
            // Will be set properly in pre_layout
            self.frozen_at_frame = Some(0);
        } else {
            self.frozen_at_frame = None;
        }
    }

    /// Check if layout is frozen.
    pub fn is_layout_frozen(&self) -> bool {
        self.layout_frozen
    }

    /// Get the underlying inspector for advanced configuration.
    pub fn inspector(&self) -> &UiInspector {
        &self.inspector
    }

    /// Get mutable access to the underlying inspector.
    pub fn inspector_mut(&mut self) -> &mut UiInspector {
        &mut self.inspector
    }

    /// Get the inspector configuration.
    pub fn config(&self) -> &InspectorConfig {
        self.inspector.config()
    }

    /// Get mutable access to the inspector configuration.
    pub fn config_mut(&mut self) -> &mut InspectorConfig {
        self.inspector.config_mut()
    }

    /// Register default keybinds for the inspector.
    pub fn register_keybinds(&self, registry: &mut super::KeybindRegistry) {
        let priority = self.priority();

        registry.register(
            self.name(),
            Keybind::key(KeyCode::F12, "Toggle inspector"),
            priority,
        );
        registry.register(
            self.name(),
            Keybind::key(KeyCode::F5, "Toggle layout freeze"),
            priority,
        );
        registry.register(
            self.name(),
            Keybind::key(KeyCode::F6, "Toggle dirty flags"),
            priority,
        );
        registry.register(
            self.name(),
            Keybind::key(KeyCode::F7, "Toggle bounds"),
            priority,
        );
        registry.register(
            self.name(),
            Keybind::key(KeyCode::Escape, "Deselect widget"),
            priority,
        );
    }
}

impl Default for InspectorMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl UiMiddleware for InspectorMiddleware {
    fn name(&self) -> &'static str {
        "inspector"
    }

    fn priority(&self) -> i32 {
        1000 // Highest priority - renders on top of everything
    }

    fn pre_layout(&mut self, ctx: &MiddlewareContext) -> bool {
        if self.layout_frozen {
            if self.frozen_at_frame.is_none() {
                self.frozen_at_frame = Some(ctx.frame_number);
            }
            return true; // Skip layout computation
        }
        false
    }

    fn post_render(&mut self, ctx: &MiddlewareContext, overlay: &mut OverlayContext) {
        if !self.enabled {
            return;
        }

        // Draw overlay quads from the inspector
        let quads = self.inspector.generate_overlay_quads(ctx.tree);
        for quad in &quads {
            overlay.draw_overlay_quad(quad);
        }

        // Draw freeze indicator if layout is frozen
        if self.layout_frozen {
            let frozen_frame = self.frozen_at_frame.unwrap_or(0);
            let elapsed = ctx.frame_number.saturating_sub(frozen_frame);

            // Background for freeze indicator
            overlay.draw_rect_bordered_rounded(
                Vec2::new(10.0, 10.0),
                Vec2::new(320.0, 30.0),
                Color::rgba(0.2, 0.0, 0.0, 0.9),
                Color::RED,
                2.0,
                4.0,
            );

            // Freeze text
            overlay.draw_text(
                Vec2::new(18.0, 16.0),
                &format!("LAYOUT FROZEN (F5 to resume) - {} frames", elapsed),
                Color::RED,
                14.0,
            );
        }

        // Draw summary info at the top
        if self.enabled {
            let summary = self.inspector.generate_summary_text();

            // Background for summary
            let summary_width = 400.0;
            let summary_x = ctx.viewport_size().x - summary_width - 10.0;

            overlay.draw_rect_rounded(
                Vec2::new(summary_x, 10.0),
                Vec2::new(summary_width, 24.0),
                Color::rgba(0.0, 0.0, 0.0, 0.7),
                4.0,
            );

            overlay.draw_text(
                Vec2::new(summary_x + 8.0, 14.0),
                &summary,
                Color::WHITE,
                12.0,
            );
        }
    }

    fn handle_keybind(&mut self, keybind: &Keybind, _ctx: &MiddlewareContext) -> bool {
        match keybind.key {
            KeyCode::F12 => {
                // F12 always works to toggle on/off
                self.toggle();
                true
            }
            KeyCode::F5 => {
                if self.enabled {
                    self.toggle_freeze();
                    true
                } else {
                    false
                }
            }
            KeyCode::F6 => {
                if self.enabled {
                    self.config_mut().show_dirty_flags ^= true;
                    true
                } else {
                    false
                }
            }
            KeyCode::F7 => {
                if self.enabled {
                    self.config_mut().show_bounds ^= true;
                    true
                } else {
                    false
                }
            }
            KeyCode::Escape => {
                if self.enabled && self.inspector.selected().is_some() {
                    self.inspector.select(None);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn handle_key_event(
        &mut self,
        key: KeyCode,
        _modifiers: Modifiers,
        pressed: bool,
        _ctx: &MiddlewareContext,
    ) -> bool {
        if !pressed {
            return false;
        }

        // Handle direct key events (in addition to registered keybinds)
        // This allows the inspector to respond even if keybinds aren't registered
        match key {
            KeyCode::F12 => {
                // F12 always works to toggle on/off
                self.toggle();
                true
            }
            KeyCode::F5 => {
                if self.enabled {
                    self.toggle_freeze();
                    true
                } else {
                    false
                }
            }
            KeyCode::F6 => {
                if self.enabled {
                    self.config_mut().show_dirty_flags ^= true;
                    true
                } else {
                    false
                }
            }
            KeyCode::F7 => {
                if self.enabled {
                    self.config_mut().show_bounds ^= true;
                    true
                } else {
                    false
                }
            }
            KeyCode::Escape => {
                if self.enabled && self.inspector.selected().is_some() {
                    self.inspector.select(None);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn update(&mut self, ctx: &MiddlewareContext, tree: &UiTree) {
        if !self.enabled {
            return;
        }

        // Update the inspector's view of the tree
        self.inspector.update(tree, ctx.registry, ctx.metrics);

        // Update hover state based on mouse position
        if let Some(hovered) = self.inspector.hit_test(tree, ctx.mouse_position) {
            self.inspector.set_hovered(Some(hovered));
        } else {
            self.inspector.set_hovered(None);
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.inspector.enable();
        } else {
            self.inspector.disable();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspector_middleware_creation() {
        let middleware = InspectorMiddleware::new();
        assert!(!middleware.is_enabled());
        assert!(!middleware.is_layout_frozen());
        assert_eq!(middleware.name(), "inspector");
        assert_eq!(middleware.priority(), 1000);
    }

    #[test]
    fn test_toggle() {
        let mut middleware = InspectorMiddleware::new();

        middleware.toggle();
        assert!(middleware.is_enabled());

        middleware.toggle();
        assert!(!middleware.is_enabled());
    }

    #[test]
    fn test_freeze() {
        let mut middleware = InspectorMiddleware::new();

        middleware.toggle_freeze();
        assert!(middleware.is_layout_frozen());

        middleware.toggle_freeze();
        assert!(!middleware.is_layout_frozen());
    }

    #[test]
    fn test_disable_clears_freeze() {
        let mut middleware = InspectorMiddleware::new();

        middleware.toggle(); // Enable
        middleware.toggle_freeze(); // Freeze

        assert!(middleware.is_enabled());
        assert!(middleware.is_layout_frozen());

        middleware.toggle(); // Disable

        assert!(!middleware.is_enabled());
        assert!(!middleware.is_layout_frozen()); // Should also unfreeze
    }

    #[test]
    fn test_config_access() {
        let mut middleware = InspectorMiddleware::new();

        middleware.config_mut().show_bounds = false;
        assert!(!middleware.config().show_bounds);

        middleware.config_mut().show_dirty_flags = false;
        assert!(!middleware.config().show_dirty_flags);
    }

    #[test]
    fn test_default_config() {
        let middleware = InspectorMiddleware::default();
        let config = middleware.config();

        assert!(config.show_bounds);
        assert!(config.show_dirty_flags);
        assert!(config.show_graphs);
    }

    #[test]
    fn test_keybind_registration() {
        use super::super::KeybindRegistry;

        let middleware = InspectorMiddleware::new();
        let mut registry = KeybindRegistry::new();

        middleware.register_keybinds(&mut registry);

        // Should have registered 5 keybinds
        let keybinds: Vec<_> = registry.all_keybinds().collect();
        assert_eq!(keybinds.len(), 5);

        // Check F12 is registered
        let f12_matches = registry.find_matches(KeyCode::F12, Modifiers::NONE);
        assert_eq!(f12_matches.len(), 1);
        assert_eq!(f12_matches[0].0, "inspector");
    }
}
