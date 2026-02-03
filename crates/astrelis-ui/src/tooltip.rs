//! Tooltip management system for hover-based information display.
//!
//! The tooltip system provides:
//! - Delay-based tooltip appearance
//! - Configurable positioning (follow cursor, anchor to widget)
//! - Rich content support (text, custom widgets)
//! - Automatic dismissal on mouse leave
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::tooltip::{TooltipManager, TooltipConfig, TooltipContent};
//!
//! let mut tooltips = TooltipManager::new(TooltipConfig::default());
//!
//! // Register a tooltip for a widget
//! tooltips.register(button_node, TooltipContent::text("Click to submit"));
//!
//! // In update loop:
//! tooltips.update(&mut overlays, &mut tree, hovered_node, delta_time);
//! ```

use std::time::{Duration, Instant};

use astrelis_core::alloc::HashMap;
use astrelis_core::math::Vec2;
use astrelis_render::Color;

use crate::overlay::{OverlayConfig, OverlayId, OverlayManager, OverlayPosition, ZLayer};
use crate::tree::{NodeId, UiTree};
use crate::widgets::Container;

/// Configuration for the tooltip system.
#[derive(Debug, Clone)]
pub struct TooltipConfig {
    /// Delay before showing tooltip after hover starts.
    pub show_delay: Duration,
    /// Delay before hiding tooltip after mouse leaves.
    pub hide_delay: Duration,
    /// Offset from cursor position.
    pub cursor_offset: Vec2,
    /// Maximum width for text tooltips.
    pub max_width: f32,
    /// Default background color.
    pub background_color: Color,
    /// Default text color.
    pub text_color: Color,
    /// Border radius.
    pub border_radius: f32,
    /// Padding inside tooltip.
    pub padding: f32,
    /// Whether tooltips follow the cursor.
    pub follow_cursor: bool,
}

impl Default for TooltipConfig {
    fn default() -> Self {
        Self {
            show_delay: Duration::from_millis(500),
            hide_delay: Duration::from_millis(100),
            cursor_offset: Vec2::new(12.0, 12.0),
            max_width: 300.0,
            background_color: Color::rgba(0.15, 0.15, 0.15, 0.95),
            text_color: Color::WHITE,
            border_radius: 4.0,
            padding: 8.0,
            follow_cursor: true,
        }
    }
}

/// Content to display in a tooltip.
#[derive(Debug, Clone)]
pub enum TooltipContent {
    /// Simple text content.
    Text(String),
    /// Rich text with formatting (future).
    RichText {
        text: String,
        font_size: f32,
        color: Color,
    },
    /// Custom widget node (pre-built in the tree).
    CustomWidget(NodeId),
    /// Builder function that creates the widget.
    Builder(TooltipBuilder),
}

impl TooltipContent {
    /// Create a simple text tooltip.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Create a rich text tooltip.
    pub fn rich_text(text: impl Into<String>, font_size: f32, color: Color) -> Self {
        Self::RichText {
            text: text.into(),
            font_size,
            color,
        }
    }
}

/// Builder for custom tooltip content.
#[derive(Debug, Clone)]
pub struct TooltipBuilder {
    /// Identifier for the builder type.
    pub id: &'static str,
    /// Associated data (serialized or type-erased).
    pub data: Option<String>,
}

impl TooltipBuilder {
    pub fn new(id: &'static str) -> Self {
        Self { id, data: None }
    }

    pub fn with_data(mut self, data: impl Into<String>) -> Self {
        self.data = Some(data.into());
        self
    }
}

/// Registration for a widget's tooltip.
#[derive(Debug, Clone)]
struct TooltipRegistration {
    /// Widget that has the tooltip.
    _widget_node: NodeId,
    /// Content to show.
    content: TooltipContent,
    /// Per-widget configuration overrides.
    config_override: Option<TooltipConfigOverride>,
}

/// Per-widget tooltip configuration overrides.
#[derive(Debug, Clone, Default)]
pub struct TooltipConfigOverride {
    pub show_delay: Option<Duration>,
    pub hide_delay: Option<Duration>,
    pub cursor_offset: Option<Vec2>,
    pub position: Option<TooltipPosition>,
}

/// Tooltip position strategy.
#[derive(Debug, Clone)]
pub enum TooltipPosition {
    /// Follow the cursor with offset.
    FollowCursor { offset: Vec2 },
    /// Anchor below the widget.
    BelowWidget { offset: Vec2 },
    /// Anchor above the widget.
    AboveWidget { offset: Vec2 },
    /// Anchor to the right of the widget.
    RightOfWidget { offset: Vec2 },
    /// Anchor to the left of the widget.
    LeftOfWidget { offset: Vec2 },
}

impl Default for TooltipPosition {
    fn default() -> Self {
        Self::FollowCursor {
            offset: Vec2::new(12.0, 12.0),
        }
    }
}

/// State of the currently active tooltip.
#[derive(Debug)]
struct ActiveTooltip {
    /// Widget the tooltip is for.
    widget_node: NodeId,
    /// Overlay ID for the tooltip.
    overlay_id: OverlayId,
    /// Node ID of the tooltip content in the tree.
    _content_node: NodeId,
    /// When the tooltip was shown.
    _shown_at: Instant,
}

/// Hover state tracking.
#[derive(Debug)]
struct HoverState {
    /// Currently hovered widget.
    widget: NodeId,
    /// When hover started.
    started_at: Instant,
    /// Whether tooltip should show.
    ready_to_show: bool,
}

/// Leave state tracking.
#[derive(Debug)]
struct LeaveState {
    /// When mouse left.
    left_at: Instant,
    /// Widget that was left.
    widget: NodeId,
}

/// Tooltip manager for handling tooltip display.
pub struct TooltipManager {
    /// Global configuration.
    config: TooltipConfig,
    /// Registered tooltips by widget node.
    registrations: HashMap<NodeId, TooltipRegistration>,
    /// Currently active tooltip.
    active: Option<ActiveTooltip>,
    /// Current hover state.
    hover_state: Option<HoverState>,
    /// Leave state for hide delay.
    leave_state: Option<LeaveState>,
    /// Current mouse position.
    mouse_position: Vec2,
    /// Whether the tooltip system is enabled.
    enabled: bool,
}

impl TooltipManager {
    /// Create a new tooltip manager.
    pub fn new(config: TooltipConfig) -> Self {
        Self {
            config,
            registrations: HashMap::new(),
            active: None,
            hover_state: None,
            leave_state: None,
            mouse_position: Vec2::ZERO,
            enabled: true,
        }
    }

    /// Enable or disable the tooltip system.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            // Clear any active state
            self.hover_state = None;
            self.leave_state = None;
            // Note: Don't clear active - let update() handle hiding
        }
    }

    /// Check if tooltips are enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get configuration.
    pub fn config(&self) -> &TooltipConfig {
        &self.config
    }

    /// Modify configuration.
    pub fn config_mut(&mut self) -> &mut TooltipConfig {
        &mut self.config
    }

    /// Register a tooltip for a widget.
    pub fn register(&mut self, widget: NodeId, content: TooltipContent) {
        self.registrations.insert(
            widget,
            TooltipRegistration {
                _widget_node: widget,
                content,
                config_override: None,
            },
        );
    }

    /// Register a tooltip with configuration overrides.
    pub fn register_with_config(
        &mut self,
        widget: NodeId,
        content: TooltipContent,
        config: TooltipConfigOverride,
    ) {
        self.registrations.insert(
            widget,
            TooltipRegistration {
                _widget_node: widget,
                content,
                config_override: Some(config),
            },
        );
    }

    /// Unregister a tooltip.
    pub fn unregister(&mut self, widget: NodeId) {
        self.registrations.remove(&widget);
    }

    /// Check if a widget has a registered tooltip.
    pub fn has_tooltip(&self, widget: NodeId) -> bool {
        self.registrations.contains_key(&widget)
    }

    /// Update mouse position.
    pub fn set_mouse_position(&mut self, pos: Vec2) {
        self.mouse_position = pos;
    }

    /// Update tooltip state based on current hover.
    ///
    /// Call this each frame with the currently hovered widget (if any).
    pub fn update(
        &mut self,
        overlays: &mut OverlayManager,
        tree: &mut UiTree,
        hovered: Option<NodeId>,
        delta_time: f32,
    ) {
        if !self.enabled {
            // Hide any active tooltip
            if let Some(active) = self.active.take() {
                overlays.hide(tree, active.overlay_id);
                // Clean up the tooltip node
                // Note: In a real implementation, we'd need proper cleanup
            }
            return;
        }

        let now = Instant::now();

        // Check if hover changed
        let hover_changed = match (&self.hover_state, hovered) {
            (Some(state), Some(hovered)) => state.widget != hovered,
            (Some(_), None) => true,
            (None, Some(_)) => true,
            (None, None) => false,
        };

        if hover_changed {
            // Handle leave from previous widget
            if let Some(state) = self.hover_state.take() {
                self.leave_state = Some(LeaveState {
                    left_at: now,
                    widget: state.widget,
                });
            }

            // Handle enter to new widget
            if let Some(hovered) = hovered
                && self.registrations.contains_key(&hovered)
            {
                self.hover_state = Some(HoverState {
                    widget: hovered,
                    started_at: now,
                    ready_to_show: false,
                });
            }
        }

        // Check show delay
        if let Some(state) = &mut self.hover_state
            && !state.ready_to_show
        {
            let reg = self.registrations.get(&state.widget);
            let show_delay = reg
                .and_then(|r| r.config_override.as_ref())
                .and_then(|c| c.show_delay)
                .unwrap_or(self.config.show_delay);

            if now.duration_since(state.started_at) >= show_delay {
                state.ready_to_show = true;
            }
        }

        // Check hide delay
        if let Some(leave) = &self.leave_state {
            let hide_delay = self.config.hide_delay;
            if now.duration_since(leave.left_at) >= hide_delay {
                // Time to hide
                if let Some(active) = &self.active
                    && active.widget_node == leave.widget
                {
                    // Hide the tooltip
                    overlays.hide(tree, active.overlay_id);
                    self.active = None;
                }
                self.leave_state = None;
            }
        }

        // Show tooltip if ready
        if let Some(state) = &self.hover_state
            && state.ready_to_show
            && self.active.is_none()
        {
            self.show_tooltip(overlays, tree, state.widget);
        }

        // Hide tooltip if no longer hovering the same widget
        if let Some(active) = &self.active {
            let should_hide = match &self.hover_state {
                Some(state) => state.widget != active.widget_node,
                None => true,
            };

            // But respect hide delay
            if should_hide && self.leave_state.is_none() {
                overlays.hide(tree, active.overlay_id);
                self.active = None;
            }
        }

        // Update tooltip position if following cursor
        if self.config.follow_cursor
            && let Some(_active) = &self.active
        {
            // Update overlay position
            overlays.set_mouse_position(self.mouse_position);
            overlays.update_positions(tree);
        }

        let _ = delta_time; // Would use for animations
    }

    /// Show tooltip for a widget.
    fn show_tooltip(&mut self, overlays: &mut OverlayManager, tree: &mut UiTree, widget: NodeId) {
        let Some(registration) = self.registrations.get(&widget) else {
            return;
        };

        // Create tooltip content node
        let content_node = self.create_tooltip_node(tree, &registration.content);

        // Calculate position
        let position = self.calculate_position(registration, widget, tree);

        // Show overlay
        let overlay_id = overlays.show(
            tree,
            content_node,
            OverlayConfig {
                layer: ZLayer::Tooltip,
                position,
                close_on_outside_click: false,
                close_on_escape: false,
                trap_focus: false,
                show_backdrop: false,
                backdrop_color: Color::TRANSPARENT,
                animate_in: false,
                animate_out: false,
                auto_dismiss: None,
            },
        );

        self.active = Some(ActiveTooltip {
            widget_node: widget,
            overlay_id,
            _content_node: content_node,
            _shown_at: Instant::now(),
        });
    }

    /// Create a node for tooltip content.
    fn create_tooltip_node(&self, tree: &mut UiTree, content: &TooltipContent) -> NodeId {
        match content {
            TooltipContent::Text(text) => {
                // Create a container with text
                let mut container = Container::new();
                container.style.background_color = Some(self.config.background_color);
                container.style.border_radius = self.config.border_radius;

                // Set padding via Taffy style
                let padding = taffy::LengthPercentage::Length(self.config.padding);
                container.style.layout.padding = taffy::Rect {
                    left: padding,
                    right: padding,
                    top: padding,
                    bottom: padding,
                };

                let container_id = tree.add_widget(Box::new(container));

                // Create text widget
                let text_widget = crate::widgets::Text::new(text.clone())
                    .color(self.config.text_color)
                    .size(14.0);
                let text_id = tree.add_widget(Box::new(text_widget));

                tree.add_child(container_id, text_id);

                container_id
            }

            TooltipContent::RichText {
                text,
                font_size,
                color,
            } => {
                let mut container = Container::new();
                container.style.background_color = Some(self.config.background_color);
                container.style.border_radius = self.config.border_radius;

                let padding = taffy::LengthPercentage::Length(self.config.padding);
                container.style.layout.padding = taffy::Rect {
                    left: padding,
                    right: padding,
                    top: padding,
                    bottom: padding,
                };

                let container_id = tree.add_widget(Box::new(container));

                let text_widget = crate::widgets::Text::new(text.clone())
                    .color(*color)
                    .size(*font_size);
                let text_id = tree.add_widget(Box::new(text_widget));

                tree.add_child(container_id, text_id);

                container_id
            }

            TooltipContent::CustomWidget(node_id) => {
                // Use the pre-existing node
                *node_id
            }

            TooltipContent::Builder(_builder) => {
                // For custom builders, create a placeholder container
                // Real implementation would call the builder
                let container = Container::new();
                tree.add_widget(Box::new(container))
            }
        }
    }

    /// Calculate tooltip position.
    fn calculate_position(
        &self,
        registration: &TooltipRegistration,
        widget: NodeId,
        _tree: &UiTree,
    ) -> OverlayPosition {
        let tooltip_pos = registration
            .config_override
            .as_ref()
            .and_then(|c| c.position.clone());

        match tooltip_pos {
            Some(TooltipPosition::FollowCursor { offset }) => OverlayPosition::AtCursor { offset },
            Some(TooltipPosition::BelowWidget { offset }) => OverlayPosition::AnchorTo {
                anchor_node: widget,
                alignment: crate::overlay::AnchorAlignment::BelowLeft,
                offset,
            },
            Some(TooltipPosition::AboveWidget { offset }) => OverlayPosition::AnchorTo {
                anchor_node: widget,
                alignment: crate::overlay::AnchorAlignment::AboveLeft,
                offset,
            },
            Some(TooltipPosition::RightOfWidget { offset }) => OverlayPosition::AnchorTo {
                anchor_node: widget,
                alignment: crate::overlay::AnchorAlignment::RightTop,
                offset,
            },
            Some(TooltipPosition::LeftOfWidget { offset }) => OverlayPosition::AnchorTo {
                anchor_node: widget,
                alignment: crate::overlay::AnchorAlignment::LeftTop,
                offset,
            },
            None => {
                if self.config.follow_cursor {
                    OverlayPosition::AtCursor {
                        offset: self.config.cursor_offset,
                    }
                } else {
                    OverlayPosition::AnchorTo {
                        anchor_node: widget,
                        alignment: crate::overlay::AnchorAlignment::BelowLeft,
                        offset: Vec2::new(0.0, 4.0),
                    }
                }
            }
        }
    }

    /// Get the currently active tooltip's widget (if any).
    pub fn active_widget(&self) -> Option<NodeId> {
        self.active.as_ref().map(|a| a.widget_node)
    }

    /// Get the currently active tooltip's overlay ID (if any).
    pub fn active_overlay(&self) -> Option<OverlayId> {
        self.active.as_ref().map(|a| a.overlay_id)
    }

    /// Force hide any active tooltip.
    pub fn hide(&mut self, overlays: &mut OverlayManager, tree: &mut UiTree) {
        if let Some(active) = self.active.take() {
            overlays.hide(tree, active.overlay_id);
        }
        self.hover_state = None;
        self.leave_state = None;
    }

    /// Clear all registrations.
    pub fn clear(&mut self) {
        self.registrations.clear();
    }
}

impl Default for TooltipManager {
    fn default() -> Self {
        Self::new(TooltipConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tooltip_content() {
        let text = TooltipContent::text("Hello");
        assert!(matches!(text, TooltipContent::Text(_)));

        let rich = TooltipContent::rich_text("Hello", 16.0, Color::RED);
        assert!(matches!(rich, TooltipContent::RichText { .. }));
    }

    #[test]
    fn test_tooltip_config_default() {
        let config = TooltipConfig::default();
        assert_eq!(config.show_delay, Duration::from_millis(500));
        assert_eq!(config.hide_delay, Duration::from_millis(100));
        assert!(config.follow_cursor);
    }

    #[test]
    fn test_tooltip_manager_registration() {
        let mut manager = TooltipManager::new(TooltipConfig::default());
        let node = NodeId(1);

        assert!(!manager.has_tooltip(node));

        manager.register(node, TooltipContent::text("Test tooltip"));
        assert!(manager.has_tooltip(node));

        manager.unregister(node);
        assert!(!manager.has_tooltip(node));
    }

    #[test]
    fn test_tooltip_position() {
        let pos = TooltipPosition::default();
        assert!(matches!(pos, TooltipPosition::FollowCursor { .. }));

        let below = TooltipPosition::BelowWidget {
            offset: Vec2::new(0.0, 4.0),
        };
        assert!(matches!(below, TooltipPosition::BelowWidget { .. }));
    }

    #[test]
    fn test_tooltip_manager_enable_disable() {
        let mut manager = TooltipManager::new(TooltipConfig::default());
        assert!(manager.is_enabled());

        manager.set_enabled(false);
        assert!(!manager.is_enabled());

        manager.set_enabled(true);
        assert!(manager.is_enabled());
    }

    #[test]
    fn test_tooltip_builder() {
        let builder = TooltipBuilder::new("custom").with_data("some data");
        assert_eq!(builder.id, "custom");
        assert_eq!(builder.data, Some("some data".to_string()));
    }
}
