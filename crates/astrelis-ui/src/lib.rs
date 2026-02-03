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
//! # let graphics_context = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
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
//!
//! ## API Conventions
//!
//! This crate follows consistent method naming conventions:
//!
//! ### Mutation Methods
//! - **`set_*()`** - Full replacement that may trigger complete rebuild/re-layout
//!   - Example: `set_text()` replaces text and triggers text shaping
//! - **`update_*()`** - Incremental update optimized with dirty flags
//!   - Example: `update_text()` only marks TEXT_SHAPING dirty, skipping layout
//! - **`add_*()`** - Append to a collection
//!   - Example: `add_widget()` appends to widget tree
//!
//! ### Accessor Methods
//! - **`get_*()`** - Returns `Option<&T>` for possibly-missing values
//!   - Example: `get_widget(id)` returns `Option<&Widget>`
//! - **`*()` (no prefix)** - Returns `&T`, panics if unavailable (use when required)
//!   - Example: `widget(id)` returns `&Widget` or panics
//! - **`try_*()`** - Fallible operation returning `Result`
//!   - Example: `try_layout()` returns `Result<(), LayoutError>`
//! - **`has_*()`** - Boolean check for existence
//!   - Example: `has_widget(id)` returns `bool`
//!
//! ### Computation Methods
//! - **`compute_*()`** - Expensive computation (results often cached)
//!   - Example: `compute_layout()` runs Taffy layout solver
//! - **`calculate_*()`** - Mathematical calculation
//!   - Example: `calculate_bounds()` computes widget bounds
//!
//! ### Builder Methods
//! - **`with_*(value)`** - Builder method returning `Self` for chaining
//!   - Example: `with_padding(20.0)` sets padding and returns builder
//! - **`build()`** - Finalizes builder and consumes it
//!   - Example: `widget.build()` adds widget to tree

pub mod animation;
pub mod builder;
pub mod clip;
pub mod constraint;
pub mod constraint_builder;
pub mod constraint_resolver;
pub mod culling;
pub mod debug;
pub mod dirty;
pub mod draw_list;
pub mod event;
pub mod focus;
pub mod glyph_atlas;
pub mod gpu_types;
pub mod inspector;
pub mod instance_buffer;
pub mod layout;
pub mod layout_engine;
pub mod length;
pub mod menu;
pub mod metrics;
pub mod metrics_collector;
pub mod middleware;
pub mod overlay;
pub mod plugin;
pub mod renderer;
pub mod scroll_plugin;
pub mod style;
pub use style::Overflow;
pub mod theme;
pub mod tooltip;
pub mod tree;
pub mod viewport_context;
pub mod virtual_scroll;
pub mod widget_id;
pub mod widgets;

pub use animation::{
    AnimatableProperty, Animation, AnimationState, AnimationSystem, EasingFunction,
    WidgetAnimations, bounce, fade_in, fade_out, scale, slide_in_left, slide_in_top,
};
use astrelis_core::geometry::Size;
pub use clip::{ClipRect, PhysicalClipRect};
pub use debug::DebugOverlay;
pub use dirty::{DirtyFlags, DirtyRanges, Versioned};
pub use draw_list::{DrawCommand, DrawList, ImageCommand, QuadCommand, TextCommand};
pub use glyph_atlas::{
    GlyphBatch, atlas_entry_uv_coords, create_glyph_batches, glyph_to_instance, glyphs_to_instances,
};
pub use gpu_types::{ImageInstance, QuadInstance, QuadVertex, TextInstance};
pub use instance_buffer::InstanceBuffer;
pub use length::{Length, LengthAuto, LengthPercentage, auto, length, percent, vh, vmax, vmin, vw};
use std::sync::Arc;
// Re-export constraint system for advanced responsive layouts
pub use astrelis_render::ImageSampling;
pub use astrelis_text::{SyncTextShaper, TextPipeline, TextShapeRequest, TextShaper};
pub use constraint::{CalcExpr, Constraint};
pub use constraint_builder::{calc, clamp, max_of, max2, min_of, min2, px};
pub use constraint_resolver::{ConstraintResolver, ResolveContext};
pub use metrics::UiMetrics;
pub use viewport_context::ViewportContext;
pub use widget_id::{WidgetId, WidgetIdRegistry};
pub use widgets::{ScrollAxis, ScrollContainer, ScrollbarVisibility};
pub use widgets::{HScrollbar, ScrollbarOrientation, ScrollbarTheme, VScrollbar};
pub use widgets::{Image, ImageFit, ImageTexture, ImageUV};

// Re-export main types
pub use builder::{
    ContainerNodeBuilder, IntoNodeBuilder, LeafNodeBuilder, UiBuilder, WidgetBuilder,
    // Legacy aliases
    ImageBuilder,
};
#[cfg(feature = "docking")]
pub use builder::{DockSplitterNodeBuilder, DockTabsNodeBuilder};
pub use event::{UiEvent, UiEventSystem};
pub use focus::{FocusDirection, FocusEvent, FocusManager, FocusPolicy, FocusScopeId};
pub use layout::LayoutCache;
pub use renderer::UiRenderer;
pub use style::Style;
pub use theme::{ColorPalette, ColorRole, Shapes, Spacing, Theme, ThemeBuilder, Typography};
pub use tree::{NodeId, UiTree};
pub use widgets::Widget;

// Re-export new architecture modules
pub use culling::{AABB, CullingStats, CullingTree};
pub use inspector::{
    EditableProperty, InspectorConfig, InspectorGraphs, PropertyEditor, SearchState, TreeViewState,
    UiInspector, WidgetIdRegistryExt, WidgetKind,
};
pub use layout_engine::{LayoutEngine, LayoutMode, LayoutRequest};
pub use menu::{ContextMenu, MenuBar, MenuItem, MenuStyle};
pub use metrics_collector::{
    FrameTimingMetrics, MemoryMetrics, MetricsCollector, MetricsConfig, PerformanceWarning,
    WidgetMetrics,
};
pub use middleware::{
    InspectorMiddleware, Keybind, KeybindRegistry, MiddlewareContext, MiddlewareManager, Modifiers,
    OverlayContext, OverlayDrawList, OverlayRenderer, UiMiddleware,
};
pub use overlay::{
    AnchorAlignment, Overlay, OverlayConfig, OverlayId, OverlayManager, OverlayPosition, ZLayer,
};
pub use tooltip::{TooltipConfig, TooltipContent, TooltipManager, TooltipPosition};
pub use virtual_scroll::{
    ItemHeight, MountedItem, VirtualScrollConfig, VirtualScrollState, VirtualScrollStats,
    VirtualScrollUpdate, VirtualScrollView,
};

// Docking system re-exports
#[cfg(feature = "docking")]
pub use widgets::docking::{
    DRAG_THRESHOLD, DockSplitter, DockTabs, DockZone, DockingStyle, DragManager, DragState,
    DragType, PanelConstraints, SplitDirection, TabScrollIndicator, TabScrollbarPosition,
};

// Plugin system re-exports
pub use plugin::{
    CorePlugin, PluginHandle, PluginManager, TraversalBehavior, UiPlugin, WidgetOverflow,
    WidgetRenderContext, WidgetTypeDescriptor, WidgetTypeRegistry,
};

// Re-export common types from dependencies
pub use astrelis_core::math::{Vec2, Vec4};
pub use astrelis_render::Color;
pub use taffy::{
    AlignContent, AlignItems, Display, FlexDirection, FlexWrap, JustifyContent, Position,
};

use astrelis_core::profiling::profile_function;
use astrelis_render::{GraphicsContext, Viewport};
use astrelis_winit::event::EventBatch;

/// Render-agnostic UI core managing tree, layout, and logic.
///
/// This is the inner layer that doesn't depend on graphics context.
/// Use this for benchmarks, tests, and headless UI processing.
pub struct UiCore {
    tree: UiTree,
    event_system: UiEventSystem,
    plugin_manager: PluginManager,
    viewport_size: Size<f32>,
    widget_registry: WidgetIdRegistry,
    viewport: Viewport,
    theme: Theme,
}

impl UiCore {
    /// Create a new render-agnostic UI core.
    ///
    /// Automatically adds [`CorePlugin`] and [`ScrollPlugin`](scroll_plugin::ScrollPlugin)
    /// to register all built-in widget types.
    /// When the `docking` feature is enabled, also adds [`DockingPlugin`](widgets::docking::plugin::DockingPlugin).
    pub fn new() -> Self {
        let mut plugin_manager = PluginManager::new();
        plugin_manager.add_plugin(CorePlugin);
        plugin_manager.add_plugin(scroll_plugin::ScrollPlugin::new());
        #[cfg(feature = "docking")]
        plugin_manager.add_plugin(widgets::docking::plugin::DockingPlugin::new());

        Self {
            tree: UiTree::new(),
            event_system: UiEventSystem::new(),
            plugin_manager,
            viewport_size: Size::new(800.0, 600.0),
            widget_registry: WidgetIdRegistry::new(),
            viewport: Viewport::default(),
            theme: Theme::dark(),
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
    ///
    /// When the viewport size changes, any constraints using viewport units
    /// (vw, vh, vmin, vmax) or complex expressions will be automatically
    /// re-resolved during the next layout computation.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        let new_size = viewport.to_logical().into();
        let size_changed = self.viewport_size != new_size;

        self.viewport_size = new_size;
        self.viewport = viewport;

        // If size changed, mark viewport-constrained nodes as dirty
        if size_changed {
            self.tree.mark_viewport_dirty();
        }
    }

    /// Get the current viewport size.
    pub fn viewport_size(&self) -> Size<f32> {
        self.viewport_size
    }

    /// Compute layout without font rendering (uses approximate text sizing).
    pub fn compute_layout(&mut self) {
        #[cfg(feature = "docking")]
        {
            let padding = self
                .plugin_manager
                .get::<widgets::docking::plugin::DockingPlugin>()
                .map(|p| p.docking_context.style().content_padding)
                .unwrap_or(0.0);
            self.tree.set_docking_content_padding(padding);
        }
        let widget_registry = self.plugin_manager.widget_registry();
        self.tree
            .compute_layout(self.viewport_size, None, widget_registry);
        // Invalidate docking cache so stale layout coordinates are refreshed
        #[cfg(feature = "docking")]
        if let Some(dp) = self.plugin_manager.get_mut::<widgets::docking::plugin::DockingPlugin>() {
            dp.invalidate_cache();
        }
    }

    /// Run plugin post-layout hooks.
    ///
    /// This dispatches `post_layout` to all registered plugins, allowing them to
    /// perform custom post-processing (e.g., ScrollPlugin updating content/viewport sizes).
    pub fn run_post_layout_plugins(&mut self) {
        self.plugin_manager.post_layout(&mut self.tree);
    }

    /// Compute layout with instrumentation for performance metrics.
    pub fn compute_layout_instrumented(&mut self) -> UiMetrics {
        let widget_registry = self.plugin_manager.widget_registry();
        let metrics = self
            .tree
            .compute_layout_instrumented(self.viewport_size, None, widget_registry);
        #[cfg(feature = "docking")]
        if let Some(dp) = self.plugin_manager.get_mut::<widgets::docking::plugin::DockingPlugin>() {
            dp.invalidate_cache();
        }
        metrics
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
        if let Some(node_id) = self.widget_registry.get_node(widget_id)
            && let Some(node) = self.tree.get_node_mut(node_id)
            && let Some(button) = node.widget.as_any_mut().downcast_mut::<widgets::Button>()
        {
            let changed = button.set_label(new_label);
            if changed {
                self.tree
                    .mark_dirty_flags(node_id, DirtyFlags::TEXT_SHAPING);
            }
            return changed;
        }
        false
    }

    /// Update text input value by ID with automatic dirty marking.
    ///
    /// Returns true if the value changed.
    pub fn update_text_input(&mut self, widget_id: WidgetId, new_value: impl Into<String>) -> bool {
        if let Some(node_id) = self.widget_registry.get_node(widget_id)
            && let Some(node) = self.tree.get_node_mut(node_id)
            && let Some(input) = node
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

    /// Get reference to the event system.
    pub fn events(&self) -> &UiEventSystem {
        &self.event_system
    }

    /// Get mutable access to the event system.
    pub fn event_system_mut(&mut self) -> &mut UiEventSystem {
        &mut self.event_system
    }

    /// Get reference to the widget registry.
    pub fn widget_registry(&self) -> &WidgetIdRegistry {
        &self.widget_registry
    }

    /// Set the theme, marking all widget colors dirty.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.tree.mark_all_dirty(DirtyFlags::COLOR);
    }

    /// Get a reference to the current theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Get a reference to the docking style.
    #[cfg(feature = "docking")]
    pub fn docking_style(&self) -> &widgets::docking::DockingStyle {
        self.plugin_manager
            .get::<widgets::docking::plugin::DockingPlugin>()
            .expect("DockingPlugin is auto-added when docking feature is enabled")
            .docking_context
            .style()
    }

    /// Replace the docking style.
    #[cfg(feature = "docking")]
    pub fn set_docking_style(&mut self, style: widgets::docking::DockingStyle) {
        self.plugin_manager
            .get_mut::<widgets::docking::plugin::DockingPlugin>()
            .expect("DockingPlugin is auto-added when docking feature is enabled")
            .docking_context
            .set_style(style);
    }

    /// Add a plugin to the UI core and return a handle for typed access.
    ///
    /// The plugin's widget types are registered immediately.
    ///
    /// # Panics
    ///
    /// Panics if a plugin of the same concrete type is already registered.
    pub fn add_plugin<P: UiPlugin>(&mut self, plugin: P) -> PluginHandle<P> {
        self.plugin_manager.add_plugin(plugin)
    }

    /// Get a handle for an already-registered plugin.
    ///
    /// Returns `Some(PluginHandle)` if the plugin is registered, `None` otherwise.
    /// Useful for obtaining handles to auto-registered plugins.
    pub fn plugin_handle<P: UiPlugin>(&self) -> Option<PluginHandle<P>> {
        self.plugin_manager.handle::<P>()
    }

    /// Get a reference to a registered plugin by type, using a handle as proof.
    pub fn plugin<P: UiPlugin>(&self, _handle: &PluginHandle<P>) -> &P {
        self.plugin_manager
            .get::<P>()
            .expect("plugin handle guarantees registration")
    }

    /// Get a mutable reference to a registered plugin by type, using a handle as proof.
    pub fn plugin_mut<P: UiPlugin>(&mut self, _handle: &PluginHandle<P>) -> &mut P {
        self.plugin_manager
            .get_mut::<P>()
            .expect("plugin handle guarantees registration")
    }

    /// Get a reference to the plugin manager.
    pub fn plugin_manager(&self) -> &PluginManager {
        &self.plugin_manager
    }

    /// Get a mutable reference to the plugin manager.
    pub fn plugin_manager_mut(&mut self) -> &mut PluginManager {
        &mut self.plugin_manager
    }

    /// Handle events from the event batch.
    pub fn handle_events(&mut self, events: &mut EventBatch) {
        self.event_system
            .handle_events_with_plugins(events, &mut self.tree, &mut self.plugin_manager);
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
    pub fn new(context: Arc<GraphicsContext>) -> Self {
        profile_function!();
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
        // Clear the draw list since we're rebuilding the tree
        // This prevents stale draw commands from accumulating
        self.renderer.clear_draw_list();
        self.core.build(build_fn);
    }

    /// Update UI state (animations, hover, etc.).
    ///
    /// Note: This no longer marks the entire tree dirty - only changed widgets are marked.
    pub fn update(&mut self, delta_time: f32) {
        // Update plugin animations (docking ghost tabs, panel transitions, etc.)
        #[cfg(feature = "docking")]
        if let Some(dp) = self
            .core
            .plugin_manager
            .get_mut::<widgets::docking::plugin::DockingPlugin>()
        {
            dp.update_animations(delta_time);
        }

        let _ = delta_time;
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
        #[cfg(feature = "docking")]
        {
            let padding = self
                .core
                .plugin_manager
                .get::<widgets::docking::plugin::DockingPlugin>()
                .map(|p| p.docking_context.style().content_padding)
                .unwrap_or(0.0);
            self.core.tree.set_docking_content_padding(padding);
        }
        let widget_registry = self.core.plugin_manager.widget_registry();
        self.core
            .tree
            .compute_layout(viewport_size, Some(font_renderer), widget_registry);
        // Invalidate docking cache so stale layout coordinates are refreshed
        #[cfg(feature = "docking")]
        if let Some(dp) = self
            .core
            .plugin_manager
            .get_mut::<widgets::docking::plugin::DockingPlugin>()
        {
            dp.invalidate_cache();
        }
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
    /// # let context = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
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
    ///
    /// Note: This automatically computes layout. If you need to control
    /// layout computation separately (e.g., for middleware freeze functionality),
    /// use `compute_layout()` + `render_without_layout()` instead.
    pub fn render(&mut self, render_pass: &mut astrelis_render::wgpu::RenderPass) {
        profile_function!();
        let logical_size = self.core.viewport_size();
        // Sync docking content padding before layout
        #[cfg(feature = "docking")]
        {
            let padding = self
                .core
                .plugin_manager
                .get::<widgets::docking::plugin::DockingPlugin>()
                .map(|p| p.docking_context.style().content_padding)
                .unwrap_or(0.0);
            self.core.tree.set_docking_content_padding(padding);
        }
        // Compute layout if dirty (clears layout-related dirty flags)
        let font_renderer = self.renderer.font_renderer();
        let widget_registry = self.core.plugin_manager.widget_registry();
        self.core
            .tree
            .compute_layout(logical_size, Some(font_renderer), widget_registry);

        // Invalidate docking cache so stale layout coordinates are refreshed
        #[cfg(feature = "docking")]
        if let Some(dp) = self
            .core
            .plugin_manager
            .get_mut::<widgets::docking::plugin::DockingPlugin>()
        {
            dp.invalidate_cache();
        }

        // Compute accurate tab widths using text shaping (docking only)
        #[cfg(feature = "docking")]
        {
            let font_renderer = self.renderer.font_renderer();
            crate::widgets::docking::compute_all_tab_widths(self.core.tree_mut(), font_renderer);
        }

        // Run plugin post-layout hooks (ScrollPlugin updates content/viewport sizes)
        self.core.run_post_layout_plugins();

        // Clean up draw commands for nodes removed since last frame
        let removed = self.core.tree_mut().drain_removed_nodes();
        if !removed.is_empty() {
            self.renderer.remove_stale_nodes(&removed);
        }

        // Render using retained mode (processes paint-only dirty flags)
        #[cfg(feature = "docking")]
        {
            // Get cross-container preview and animations from DockingPlugin
            let (preview, animations) = self
                .core
                .plugin_manager
                .get::<widgets::docking::plugin::DockingPlugin>()
                .map(|dp| {
                    (
                        dp.cross_container_preview,
                        &dp.dock_animations,
                    )
                })
                .unzip();
            let widget_registry = self.core.plugin_manager.widget_registry();

            self.renderer.render_instanced_with_preview(
                self.core.tree(),
                render_pass,
                self.core.viewport,
                preview.flatten().as_ref(),
                animations,
                widget_registry,
            );
        }

        #[cfg(not(feature = "docking"))]
        {
            let widget_registry = self.core.plugin_manager.widget_registry();
            self.renderer
                .render_instanced(self.core.tree(), render_pass, self.core.viewport, widget_registry);
        }

        // Clear all dirty flags after rendering
        // (layout computation no longer clears flags - renderer owns this)
        self.core.tree_mut().clear_dirty_flags();
    }

    /// Render the UI without computing layout.
    ///
    /// Use this when you want to manually control layout computation,
    /// for example when implementing layout freeze functionality with middleware.
    ///
    /// # Parameters
    /// - `render_pass`: The WGPU render pass to render into
    /// - `clear_dirty_flags`: Whether to clear dirty flags after rendering.
    ///   Set to `false` when layout is frozen to preserve dirty state for inspection.
    ///
    /// Typical usage:
    /// ```ignore
    /// // Check if middleware wants to freeze layout
    /// let skip_layout = middlewares.pre_layout(&ctx);
    /// if !skip_layout {
    ///     ui.compute_layout();
    /// }
    /// // Don't clear flags when frozen so inspector can keep showing them
    /// ui.render_without_layout(render_pass, !skip_layout);
    /// ```
    pub fn render_without_layout(
        &mut self,
        render_pass: &mut astrelis_render::wgpu::RenderPass,
        clear_dirty_flags: bool,
    ) {
        profile_function!();
        // Run plugin post-layout hooks (ScrollPlugin updates content/viewport sizes)
        self.core.run_post_layout_plugins();

        // Clean up draw commands for nodes removed since last frame
        let removed = self.core.tree_mut().drain_removed_nodes();
        if !removed.is_empty() {
            self.renderer.remove_stale_nodes(&removed);
        }

        // Render using retained mode (processes paint-only dirty flags)
        let widget_registry = self.core.plugin_manager.widget_registry();
        self.renderer
            .render_instanced(self.core.tree(), render_pass, self.core.viewport, widget_registry);

        // Clear dirty flags unless we're in a frozen state
        if clear_dirty_flags {
            self.core.tree_mut().clear_dirty_flags();
        }
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

    /// Set the theme, marking all widget colors dirty.
    pub fn set_theme(&mut self, theme: Theme) {
        self.renderer.set_theme_colors(theme.colors.clone());
        self.core.set_theme(theme);
    }

    /// Get a reference to the current theme.
    pub fn theme(&self) -> &Theme {
        self.core.theme()
    }

    /// Get a reference to the docking style.
    #[cfg(feature = "docking")]
    pub fn docking_style(&self) -> &widgets::docking::DockingStyle {
        self.core.docking_style()
    }

    /// Replace the docking style.
    #[cfg(feature = "docking")]
    pub fn set_docking_style(&mut self, style: widgets::docking::DockingStyle) {
        self.core.set_docking_style(style);
    }

    /// Add a plugin to the UI system and return a handle for typed access.
    ///
    /// The plugin's widget types are registered immediately.
    ///
    /// # Panics
    ///
    /// Panics if a plugin of the same concrete type is already registered.
    pub fn add_plugin<P: UiPlugin>(&mut self, plugin: P) -> PluginHandle<P> {
        self.core.add_plugin(plugin)
    }

    /// Get a handle for an already-registered plugin.
    ///
    /// Returns `Some(PluginHandle)` if the plugin is registered, `None` otherwise.
    /// Useful for obtaining handles to auto-registered plugins.
    pub fn plugin_handle<P: UiPlugin>(&self) -> Option<PluginHandle<P>> {
        self.core.plugin_handle::<P>()
    }

    /// Get a reference to a registered plugin by type, using a handle as proof.
    pub fn plugin<P: UiPlugin>(&self, handle: &PluginHandle<P>) -> &P {
        self.core.plugin(handle)
    }

    /// Get a mutable reference to a registered plugin by type, using a handle as proof.
    pub fn plugin_mut<P: UiPlugin>(&mut self, handle: &PluginHandle<P>) -> &mut P {
        self.core.plugin_mut(handle)
    }

    /// Get a reference to the plugin manager.
    pub fn plugin_manager(&self) -> &PluginManager {
        self.core.plugin_manager()
    }
}
