//! Scroll plugin providing ScrollContainer widget type and scroll state management.
//!
//! This plugin owns the scrollbar drag state and handles post-layout content/viewport
//! size computation for all ScrollContainer widgets in the tree.

use crate::plugin::UiPlugin;
use crate::plugin::registry::{WidgetOverflow, WidgetTypeDescriptor, WidgetTypeRegistry};
use crate::style::Overflow;
use crate::tree::{NodeId, UiTree};
use crate::widgets::scroll_container::ScrollContainer;
use astrelis_core::math::Vec2;
use std::any::Any;

/// Plugin providing the ScrollContainer widget type and cross-widget scroll state.
///
/// Owns:
/// - Scrollbar drag state (which node, vertical vs horizontal)
///
/// Post-layout:
/// - Computes content/viewport sizes for all ScrollContainers
/// - Clamps scroll offsets when content shrinks
pub struct ScrollPlugin {
    /// ScrollContainer node whose scrollbar thumb is being dragged,
    /// with a flag indicating vertical (`true`) or horizontal (`false`).
    pub scroll_container_drag: Option<(NodeId, bool)>,
}

impl ScrollPlugin {
    /// Create a new scroll plugin with default state.
    pub fn new() -> Self {
        Self {
            scroll_container_drag: None,
        }
    }

    /// Invalidate any references to nodes that no longer exist.
    pub fn invalidate_removed_nodes(&mut self, tree: &UiTree) {
        if let Some((id, _)) = self.scroll_container_drag
            && !tree.node_exists(id)
        {
            self.scroll_container_drag = None;
        }
    }
}

impl Default for ScrollPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl UiPlugin for ScrollPlugin {
    fn name(&self) -> &str {
        "scroll"
    }

    fn register_widgets(&self, registry: &mut WidgetTypeRegistry) {
        use crate::plugin::core_widgets::{
            render_scroll_container, scroll_container_clips, scroll_container_offset,
            scroll_container_overflow,
        };

        registry.register::<ScrollContainer>(
            WidgetTypeDescriptor::new("ScrollContainer")
                .with_render(render_scroll_container)
                .with_scroll_offset(scroll_container_offset)
                .with_clips_children(scroll_container_clips)
                .with_overflow(scroll_container_overflow),
        );
    }

    fn post_layout(&mut self, tree: &mut UiTree) {
        update_scroll_container_sizes(tree);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Update cached content and viewport sizes for all ScrollContainers.
///
/// This runs after layout computation, before draw list generation.
/// For each ScrollContainer it computes the maximum extent of its children
/// and updates `content_size` / `viewport_size`. Scroll offsets are clamped
/// if the content shrunk.
pub fn update_scroll_container_sizes(tree: &mut UiTree) {
    // Find all ScrollContainer nodes
    let sc_nodes: Vec<NodeId> = tree
        .iter()
        .filter_map(|(id, node)| {
            node.widget
                .as_any()
                .downcast_ref::<ScrollContainer>()
                .map(|_| id)
        })
        .collect();

    for sc_id in sc_nodes {
        let viewport = {
            let Some(layout) = tree.get_layout(sc_id) else {
                continue;
            };
            Vec2::new(layout.width, layout.height)
        };

        // Compute content extent from children
        let content = {
            let Some(widget) = tree.get_widget(sc_id) else {
                continue;
            };
            let children = widget.children().to_vec();
            let mut max = Vec2::ZERO;
            for child_id in children {
                if let Some(child_layout) = tree.get_layout(child_id) {
                    max.x = max.x.max(child_layout.x + child_layout.width);
                    max.y = max.y.max(child_layout.y + child_layout.height);
                }
            }
            max
        };

        // Update the ScrollContainer widget
        if let Some(widget) = tree.get_widget_mut(sc_id)
            && let Some(sc) = widget.as_any_mut().downcast_mut::<ScrollContainer>()
        {
            sc.content_size = content;
            sc.viewport_size = viewport;
            sc.clamp_scroll();
        }
    }
}

/// Render function for ScrollContainer — delegates to core_widgets.
pub use crate::plugin::core_widgets::render_scroll_container;

/// Overflow handler for ScrollContainer.
pub fn scroll_container_overflow_handler(_widget: &dyn Any) -> WidgetOverflow {
    WidgetOverflow {
        overflow_x: Overflow::Hidden,
        overflow_y: Overflow::Hidden,
    }
}
