//! Astrelis UI - Taffy-based UI system with WGPU rendering
//!
//! This crate provides a flexible UI system built on Taffy layout engine:
//! - Declarative widget API
//! - Flexbox and Grid layouts via Taffy
//! - GPU-accelerated rendering
//! - Event handling system
//! - Composable widget tree
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! # use astrelis_ui::UiSystem;
//! # use astrelis_render::{Color, GraphicsContext};
//! # let graphics_context = GraphicsContext::new_sync();
//! let mut ui = UiSystem::new(graphics_context);
//!
//! ui.build(|root| {
//!     root.container()
//!         .width(800.0)
//!         .height(600.0)
//!         .padding(20.0)
//!         .child(|container| {
//!             container.text("Hello, World!")
//!                 .size(24.0)
//!                 .color(Color::WHITE)
//!                 .build();
//!             container.button("Click Me").build();
//!             container.container().build()
//!         })
//!         .build();
//! });
//!
//! // In render loop:
//! // ui.update(delta_time);
//! // ui.handle_events(&mut event_batch);
//! // ui.render(&mut render_pass, viewport_size);
//! ```

pub mod auto_dirty;
pub mod builder;
pub mod debug;
pub mod dirty;
pub mod dirty_ranges;
pub mod draw_list;
pub mod event;
pub mod glyph_atlas;
pub mod gpu_types;
pub mod instance_buffer;
pub mod layout;
pub mod length;
pub mod metrics;
pub mod renderer;
pub mod style;
pub mod tree;
pub mod widget_id;
pub mod widgets;

use astrelis_core::geometry::Size;
pub use auto_dirty::{NumericValue, TextValue, Value};
pub use debug::DebugOverlay;
pub use dirty::DirtyFlags;
pub use dirty_ranges::DirtyRanges;
pub use draw_list::{DrawCommand, DrawList, QuadCommand, TextCommand};
pub use glyph_atlas::{
    GlyphBatch, atlas_entry_uv_coords, create_glyph_batches, glyph_to_instance, glyphs_to_instances,
};
pub use gpu_types::{QuadInstance, QuadVertex, TextInstance};
pub use instance_buffer::InstanceBuffer;
pub use length::{Length, LengthAuto, LengthPercentage, auto, length, percent};
pub use metrics::UiMetrics;
pub use astrelis_text::{TextPipeline, TextShapeRequest, TextShaper, SyncTextShaper};
pub use widget_id::{WidgetId, WidgetIdRegistry};

// Re-export main types
pub use builder::{UiBuilder, WidgetBuilder};
pub use event::{UiEvent, UiEventSystem};
pub use layout::LayoutCache;
pub use renderer::UiRenderer;
pub use style::Style;
pub use tree::{NodeId, UiTree};
pub use widgets::Widget;

// Re-export common types from dependencies
pub use astrelis_core::math::{Vec2, Vec4};
pub use astrelis_render::Color;
pub use taffy::{
    AlignContent, AlignItems, Display, FlexDirection, FlexWrap, JustifyContent, Position,
};

use astrelis_render::{GraphicsContext, Viewport};
use astrelis_winit::event::EventBatch;

/// Render-agnostic UI core managing tree, layout, and logic.
///
/// This is the inner layer that doesn't depend on graphics context.
/// Use this for benchmarks, tests, and headless UI processing.
pub struct UiCore {
    tree: UiTree,
    event_system: UiEventSystem,
    viewport_size: Size<f32>,
    widget_registry: WidgetIdRegistry,
    viewport: Viewport,
}

impl UiCore {
    /// Create a new render-agnostic UI core.
    pub fn new() -> Self {
        Self {
            tree: UiTree::new(),
            event_system: UiEventSystem::new(),
            viewport_size: Size::new(800.0, 600.0),
            widget_registry: WidgetIdRegistry::new(),
            viewport: Viewport::default(),
        }
    }

    /// Build the UI tree using a declarative builder API.
    pub fn build<F>(&mut self, build_fn: F)
    where
        F: FnOnce(&mut UiBuilder),
    {
        self.widget_registry.clear();
        let mut builder = UiBuilder::new(&mut self.tree, &mut self.widget_registry);
        build_fn(&mut builder);
        builder.finish();
    }

    /// Set the viewport size for layout calculations.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport_size = viewport.to_logical();
        self.viewport = viewport;
    }

    /// Get the current viewport size.
    pub fn viewport_size(&self) -> Size<f32> {
        self.viewport_size
    }

    /// Compute layout without font rendering (uses approximate text sizing).
    pub fn compute_layout(&mut self) {
        self.tree.compute_layout(self.viewport_size, None);
    }

    /// Compute layout with instrumentation for performance metrics.
    pub fn compute_layout_instrumented(&mut self) -> UiMetrics {
        self.tree
            .compute_layout_instrumented(self.viewport_size, None)
    }

    /// Get the node ID for a widget ID.
    pub fn get_node_id(&self, widget_id: WidgetId) -> Option<NodeId> {
        self.widget_registry.get_node(widget_id)
    }

    /// Register a widget ID to node ID mapping.
    pub fn register_widget(&mut self, widget_id: WidgetId, node_id: NodeId) {
        self.widget_registry.register(widget_id, node_id);
    }

    /// Update text content of a Text widget by ID with automatic dirty marking.
    ///
    /// Returns true if the content changed.
    pub fn update_text(&mut self, widget_id: WidgetId, new_content: impl Into<String>) -> bool {
        if let Some(node_id) = self.widget_registry.get_node(widget_id) {
            self.tree.update_text_content(node_id, new_content)
        } else {
            false
        }
    }

    /// Update button label by ID with automatic dirty marking.
    ///
    /// Returns true if the label changed.
    pub fn update_button_label(
        &mut self,
        widget_id: WidgetId,
        new_label: impl Into<String>,
    ) -> bool {
        if let Some(node_id) = self.widget_registry.get_node(widget_id) {
            if let Some(node) = self.tree.get_node_mut(node_id) {
                if let Some(button) = node.widget.as_any_mut().downcast_mut::<widgets::Button>() {
                    let changed = button.set_label(new_label);
                    if changed {
                        self.tree
                            .mark_dirty_flags(node_id, DirtyFlags::TEXT_SHAPING);
                    }
                    return changed;
                }
            }
        }
        false
    }

    /// Update text input value by ID with automatic dirty marking.
    ///
    /// Returns true if the value changed.
    pub fn update_text_input(&mut self, widget_id: WidgetId, new_value: impl Into<String>) -> bool {
        if let Some(node_id) = self.widget_registry.get_node(widget_id) {
            if let Some(node) = self.tree.get_node_mut(node_id) {
                if let Some(input) = node
                    .widget
                    .as_any_mut()
                    .downcast_mut::<widgets::TextInput>()
                {
                    let changed = input.set_value(new_value);
                    if changed {
                        self.tree
                            .mark_dirty_flags(node_id, DirtyFlags::TEXT_SHAPING);
                    }
                    return changed;
                }
            }
        }
        false
    }

    /// Update widget color by ID with automatic dirty marking.
    ///
    /// Returns true if the color changed.
    pub fn update_color(&mut self, widget_id: WidgetId, color: astrelis_render::Color) -> bool {
        if let Some(node_id) = self.widget_registry.get_node(widget_id) {
            self.tree.update_color(node_id, color)
        } else {
            false
        }
    }

    /// Get mutable access to the tree.
    pub fn tree_mut(&mut self) -> &mut UiTree {
        &mut self.tree
    }

    /// Get reference to the tree.
    pub fn tree(&self) -> &UiTree {
        &self.tree
    }

    /// Get mutable access to the event system.
    pub fn event_system_mut(&mut self) -> &mut UiEventSystem {
        &mut self.event_system
    }

    /// Get reference to the widget registry.
    pub fn widget_registry(&self) -> &WidgetIdRegistry {
        &self.widget_registry
    }

    /// Handle events from the event batch.
    pub fn handle_events(&mut self, events: &mut EventBatch) {
        self.event_system.handle_events(events, &mut self.tree);
    }
}

impl Default for UiCore {
    fn default() -> Self {
        Self::new()
    }
}

/// Main UI system managing tree, layout, rendering, and events.
///
/// This wraps UiCore and adds rendering capabilities.
pub struct UiSystem {
    core: UiCore,
    renderer: UiRenderer,
}

impl UiSystem {
    /// Create a new UI system with rendering support.
    pub fn new(context: &'static GraphicsContext) -> Self {
        Self {
            core: UiCore::new(),
            renderer: UiRenderer::new(context),
        }
    }

    /// Build the UI tree using a declarative builder API.
    ///
    /// Note: This does a full rebuild. For incremental updates, use update methods.
    pub fn build<F>(&mut self, build_fn: F)
    where
        F: FnOnce(&mut UiBuilder),
    {
        self.core.build(build_fn);
    }

    /// Update UI state (animations, hover, etc.).
    ///
    /// Note: This no longer marks the entire tree dirty - only changed widgets are marked.
    pub fn update(&mut self, _delta_time: f32) {
        // Animations and other updates would mark specific nodes dirty
    }

    /// Set the viewport size for layout calculations.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.renderer.set_viewport(viewport);
        self.core.set_viewport(viewport);
    }

    /// Handle events from the event batch.
    pub fn handle_events(&mut self, events: &mut EventBatch) {
        self.core.handle_events(events);
    }

    /// Compute layout for all widgets.
    pub fn compute_layout(&mut self) {
        let viewport_size = self.core.viewport_size();
        let font_renderer = self.renderer.font_renderer();
        self.core
            .tree_mut()
            .compute_layout(viewport_size, Some(font_renderer));
    }

    /// Get the node ID for a widget ID.
    pub fn get_node_id(&self, widget_id: WidgetId) -> Option<tree::NodeId> {
        self.core.get_node_id(widget_id)
    }

    /// Register a widget ID to node ID mapping.
    pub fn register_widget(&mut self, widget_id: WidgetId, node_id: tree::NodeId) {
        self.core.register_widget(widget_id, node_id);
    }

    /// Update text content of a Text widget by ID with automatic dirty marking.
    ///
    /// This is much faster than rebuilding the entire UI tree.
    /// Returns true if the content changed.
    ///
    /// # Example
    /// ```no_run
    /// # use astrelis_ui::{UiSystem, WidgetId};
    /// # use astrelis_render::GraphicsContext;
    /// # let context = GraphicsContext::new_sync();
    /// # let mut ui = UiSystem::new(context);
    /// let counter_id = WidgetId::new("counter");
    /// ui.update_text(counter_id, "Count: 42");
    /// ```
    pub fn update_text(&mut self, widget_id: WidgetId, new_content: impl Into<String>) -> bool {
        self.core.update_text(widget_id, new_content)
    }

    /// Get text cache statistics from the renderer.
    pub fn text_cache_stats(&self) -> String {
        self.renderer.text_cache_stats()
    }

    /// Get text cache hit rate.
    pub fn text_cache_hit_rate(&self) -> f32 {
        self.renderer.text_cache_hit_rate()
    }

    /// Log text cache statistics.
    pub fn log_text_cache_stats(&self) {
        self.renderer.log_text_cache_stats();
    }

    /// Update button label by ID with automatic dirty marking.
    ///
    /// Returns true if the label changed.
    pub fn update_button_label(
        &mut self,
        widget_id: WidgetId,
        new_label: impl Into<String>,
    ) -> bool {
        self.core.update_button_label(widget_id, new_label)
    }

    /// Update text input value by ID with automatic dirty marking.
    ///
    /// Returns true if the value changed.
    pub fn update_text_input(&mut self, widget_id: WidgetId, new_value: impl Into<String>) -> bool {
        self.core.update_text_input(widget_id, new_value)
    }

    /// Update widget color by ID with automatic dirty marking.
    ///
    /// Returns true if the color changed.
    pub fn update_color(&mut self, widget_id: WidgetId, color: astrelis_render::Color) -> bool {
        self.core.update_color(widget_id, color)
    }

    /// Render the UI using retained mode instanced rendering.
    ///
    /// This is the high-performance path that only updates dirty nodes
    /// and uses GPU instancing for efficient rendering.
    pub fn render(
        &mut self,
        render_pass: &mut astrelis_render::wgpu::RenderPass,
    ) {
        let logical_size = self.core.viewport_size();
        // Compute layout if dirty (clears layout-related dirty flags)
        let font_renderer = self.renderer.font_renderer();
        self.core
            .tree_mut()
            .compute_layout(logical_size, Some(font_renderer));

        // Render using retained mode (processes paint-only dirty flags)
        self.renderer
            .render_instanced(self.core.tree(), render_pass, self.core.viewport);

        // Clear all dirty flags after rendering
        // (layout computation no longer clears flags - renderer owns this)
        self.core.tree_mut().clear_dirty_flags();
    }

    /// Get mutable access to the core for advanced usage.
    pub fn core_mut(&mut self) -> &mut UiCore {
        &mut self.core
    }

    /// Get reference to the core.
    pub fn core(&self) -> &UiCore {
        &self.core
    }

    /// Get mutable access to the tree for advanced usage.
    pub fn tree_mut(&mut self) -> &mut UiTree {
        self.core.tree_mut()
    }

    /// Get reference to the tree.
    pub fn tree(&self) -> &UiTree {
        self.core.tree()
    }

    /// Get mutable access to the event system.
    pub fn event_system_mut(&mut self) -> &mut UiEventSystem {
        self.core.event_system_mut()
    }

    /// Get reference to the font renderer.
    pub fn font_renderer(&self) -> &astrelis_text::FontRenderer {
        self.renderer.font_renderer()
    }
}
