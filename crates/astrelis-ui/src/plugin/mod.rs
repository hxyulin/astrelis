//! Plugin system for modular UI widget registration and event handling.
//!
//! The plugin architecture replaces hardcoded downcast chains with a registry-based
//! dispatch system. Each plugin registers its widget types and their handler functions,
//! enabling O(1) dispatch instead of O(N) downcast chains.
//!
//! # Architecture
//!
//! - [`UiPlugin`]: Trait for implementing widget plugins (core, docking, scrolling, etc.)
//! - [`PluginHandle`]: Zero-cost proof token that a plugin is registered
//! - [`PluginManager`]: Owns plugins and the widget type registry
//! - [`registry::WidgetTypeRegistry`]: Maps `TypeId` → handler functions for O(1) dispatch
//!
//! # Example
//!
//! ```ignore
//! let mut ui = UiSystem::new(graphics);
//! // CorePlugin is auto-added in UiCore::new()
//!
//! // Add optional plugins:
//! let docking = ui.add_plugin(DockingPlugin::new());
//!
//! // Use plugin handle for typed access:
//! let plugin: &DockingPlugin = ui.plugin(&docking);
//! ```

pub mod core_widgets;
pub mod event_types;
pub mod registry;

pub use event_types::{KeyEventData, MouseButtonKind, PluginEventContext, UiInputEvent};
pub use registry::{
    EventResponse, TraversalBehavior, WidgetOverflow, WidgetRenderContext, WidgetTypeDescriptor,
    WidgetTypeRegistry,
};

use crate::tree::UiTree;
use std::any::Any;
use std::marker::PhantomData;

/// Trait for UI plugins that register widget types and handle events.
///
/// Plugins provide:
/// - Widget type registration (render, measure, traversal handlers)
/// - Cross-widget stateful event handling (drags, gestures, etc.)
/// - Post-layout processing
/// - Per-frame updates (animations, state cleanup)
pub trait UiPlugin: Any + 'static {
    /// Plugin name for debugging and logging.
    fn name(&self) -> &str;

    /// Register widget type descriptors this plugin provides.
    ///
    /// Called once when the plugin is added to the manager.
    fn register_widgets(&self, registry: &mut WidgetTypeRegistry);

    /// Handle a UI input event. Return `true` if consumed.
    ///
    /// Called before per-widget-type dispatch, in plugin registration order.
    /// Use this for cross-widget stateful interactions (drags, etc.)
    fn handle_event(&mut self, _event: &UiInputEvent, _ctx: &mut PluginEventContext<'_>) -> bool {
        false
    }

    /// Called after Taffy layout for custom post-processing.
    fn post_layout(&mut self, _tree: &mut UiTree) {}

    /// Per-frame update (animations, state cleanup, etc.)
    fn update(&mut self, _dt: f32, _tree: &mut UiTree) {}

    /// Downcast support — return `self` as `&dyn Any`.
    fn as_any(&self) -> &dyn Any;

    /// Downcast support — return `self` as `&mut dyn Any`.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Zero-cost proof token that a plugin of type `P` has been registered.
///
/// Only [`PluginManager::add_plugin`] can construct a handle. The handle gates
/// builder methods and provides typed access to plugin state.
///
/// ```ignore
/// let handle = ui.add_plugin(MyPlugin::new());
/// // handle proves MyPlugin is registered — gates builder methods
/// root.my_widget(&handle).build();
/// // typed access to plugin state
/// let plugin: &MyPlugin = ui.plugin(&handle);
/// ```
pub struct PluginHandle<P: UiPlugin> {
    _private: (),
    _marker: PhantomData<P>,
}

impl<P: UiPlugin> Clone for PluginHandle<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: UiPlugin> Copy for PluginHandle<P> {}

impl<P: UiPlugin> std::fmt::Debug for PluginHandle<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginHandle")
            .field("type", &std::any::type_name::<P>())
            .finish()
    }
}

/// Manages UI plugins and the widget type registry.
///
/// Plugins are stored in registration order. The widget type registry
/// is populated during [`add_plugin`](PluginManager::add_plugin) and
/// provides O(1) dispatch for render/measure/event operations.
pub struct PluginManager {
    plugins: Vec<Box<dyn UiPlugin>>,
    widget_registry: WidgetTypeRegistry,
}

impl PluginManager {
    /// Create a new empty plugin manager.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            widget_registry: WidgetTypeRegistry::new(),
        }
    }

    /// Add a plugin, register its widgets, and return a proof handle.
    ///
    /// # Panics
    ///
    /// Panics if a plugin of the same concrete type is already registered.
    pub fn add_plugin<P: UiPlugin>(&mut self, plugin: P) -> PluginHandle<P> {
        // Check for duplicate registration
        let type_id = std::any::TypeId::of::<P>();
        for existing in &self.plugins {
            if existing.as_any().type_id() == type_id {
                panic!(
                    "Plugin '{}' is already registered",
                    std::any::type_name::<P>()
                );
            }
        }

        // Register widget types
        plugin.register_widgets(&mut self.widget_registry);

        // Store the plugin
        self.plugins.push(Box::new(plugin));

        PluginHandle {
            _private: (),
            _marker: PhantomData,
        }
    }

    /// Get a handle for an already-registered plugin by type.
    ///
    /// Returns `Some(PluginHandle)` if the plugin is registered, `None` otherwise.
    /// This is useful for obtaining handles to auto-registered plugins
    /// (e.g., `DockingPlugin`, `ScrollPlugin`).
    pub fn handle<P: UiPlugin>(&self) -> Option<PluginHandle<P>> {
        if self.get::<P>().is_some() {
            Some(PluginHandle {
                _private: (),
                _marker: PhantomData,
            })
        } else {
            None
        }
    }

    /// Get a reference to a registered plugin by type.
    pub fn get<P: UiPlugin>(&self) -> Option<&P> {
        let type_id = std::any::TypeId::of::<P>();
        for plugin in &self.plugins {
            if plugin.as_any().type_id() == type_id {
                return plugin.as_any().downcast_ref::<P>();
            }
        }
        None
    }

    /// Get a mutable reference to a registered plugin by type.
    pub fn get_mut<P: UiPlugin>(&mut self) -> Option<&mut P> {
        let type_id = std::any::TypeId::of::<P>();
        for plugin in &mut self.plugins {
            if plugin.as_any().type_id() == type_id {
                return plugin.as_any_mut().downcast_mut::<P>();
            }
        }
        None
    }

    /// Get a reference to the widget type registry.
    pub fn widget_registry(&self) -> &WidgetTypeRegistry {
        &self.widget_registry
    }

    /// Dispatch `post_layout` to all plugins in registration order.
    pub fn post_layout(&mut self, tree: &mut UiTree) {
        for plugin in &mut self.plugins {
            plugin.post_layout(tree);
        }
    }

    /// Dispatch `update` to all plugins in registration order.
    pub fn update(&mut self, dt: f32, tree: &mut UiTree) {
        for plugin in &mut self.plugins {
            plugin.update(dt, tree);
        }
    }

    /// Dispatch an event to all plugins in order. Returns `true` if consumed.
    pub fn handle_event(&mut self, event: &UiInputEvent, ctx: &mut PluginEventContext<'_>) -> bool {
        for plugin in &mut self.plugins {
            if plugin.handle_event(event, ctx) {
                return true;
            }
        }
        false
    }

    /// Number of registered plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Core plugin providing built-in widget types.
///
/// This plugin is automatically added in `UiCore::new()` — users never
/// need to add it manually. It registers descriptors for:
/// Container, Text, Button, TextInput, Image, Row, Column, Tooltip,
/// HScrollbar, VScrollbar.
pub struct CorePlugin;

impl UiPlugin for CorePlugin {
    fn name(&self) -> &str {
        "core"
    }

    fn register_widgets(&self, registry: &mut WidgetTypeRegistry) {
        use crate::widgets::*;
        use core_widgets::*;

        registry.register::<Container>(
            WidgetTypeDescriptor::new("Container")
                .with_render(render_container)
                .with_overflow(container_overflow),
        );
        registry.register::<Text>(
            WidgetTypeDescriptor::new("Text")
                .with_render(render_text)
                .with_caches_measurement(),
        );
        registry.register::<Button>(
            WidgetTypeDescriptor::new("Button")
                .with_render(render_button)
                .with_on_hover(button_hover)
                .with_on_press(button_press)
                .with_on_click(button_click),
        );
        // TextInput has no custom render — uses default style-based rendering
        registry.register::<TextInput>(
            WidgetTypeDescriptor::new("TextInput")
                .with_on_click(text_input_click)
                .with_on_key_input(text_input_key)
                .with_on_char_input(text_input_char),
        );
        registry.register::<Image>(WidgetTypeDescriptor::new("Image").with_render(render_image));
        // Row and Column are layout-only — no visual rendering in current code
        registry.register::<Row>(WidgetTypeDescriptor::new("Row"));
        registry.register::<Column>(WidgetTypeDescriptor::new("Column"));
        registry
            .register::<Tooltip>(WidgetTypeDescriptor::new("Tooltip").with_render(render_tooltip));
        registry.register::<HScrollbar>(
            WidgetTypeDescriptor::new("HScrollbar").with_render(render_hscrollbar),
        );
        registry.register::<VScrollbar>(
            WidgetTypeDescriptor::new("VScrollbar").with_render(render_vscrollbar),
        );

        // ScrollContainer is registered by ScrollPlugin (see scroll_plugin.rs)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlugin {
        initialized: bool,
    }

    impl TestPlugin {
        fn new() -> Self {
            Self { initialized: true }
        }
    }

    impl UiPlugin for TestPlugin {
        fn name(&self) -> &str {
            "test"
        }

        fn register_widgets(&self, registry: &mut WidgetTypeRegistry) {
            // Register a dummy type for testing
            registry.register::<TestPlugin>(WidgetTypeDescriptor::new("TestWidget"));
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn test_plugin_manager_add_and_get() {
        let mut manager = PluginManager::new();
        let _handle = manager.add_plugin(TestPlugin::new());

        let plugin = manager.get::<TestPlugin>().unwrap();
        assert!(plugin.initialized);
        assert_eq!(plugin.name(), "test");
    }

    #[test]
    fn test_plugin_manager_get_mut() {
        let mut manager = PluginManager::new();
        let _handle = manager.add_plugin(TestPlugin::new());

        let plugin = manager.get_mut::<TestPlugin>().unwrap();
        plugin.initialized = false;

        let plugin = manager.get::<TestPlugin>().unwrap();
        assert!(!plugin.initialized);
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn test_plugin_manager_duplicate_panics() {
        let mut manager = PluginManager::new();
        manager.add_plugin(TestPlugin::new());
        manager.add_plugin(TestPlugin::new()); // should panic
    }

    #[test]
    fn test_plugin_handle_is_copy() {
        let mut manager = PluginManager::new();
        let handle = manager.add_plugin(TestPlugin::new());
        let _copy = handle; // Copy
        let _clone = handle; // still valid after copy
    }

    #[test]
    fn test_core_plugin_registers_widgets() {
        let mut manager = PluginManager::new();
        manager.add_plugin(CorePlugin);

        let registry = manager.widget_registry();
        assert!(registry.contains(std::any::TypeId::of::<crate::widgets::Container>()));
        assert!(registry.contains(std::any::TypeId::of::<crate::widgets::Text>()));
        assert!(registry.contains(std::any::TypeId::of::<crate::widgets::Button>()));
        assert!(registry.contains(std::any::TypeId::of::<crate::widgets::TextInput>()));
        assert!(registry.contains(std::any::TypeId::of::<crate::widgets::Image>()));
        assert!(registry.contains(std::any::TypeId::of::<crate::widgets::Row>()));
        assert!(registry.contains(std::any::TypeId::of::<crate::widgets::Column>()));
        assert!(registry.contains(std::any::TypeId::of::<crate::widgets::Tooltip>()));
        assert!(registry.contains(std::any::TypeId::of::<crate::widgets::HScrollbar>()));
        assert!(registry.contains(std::any::TypeId::of::<crate::widgets::VScrollbar>()));
        // ScrollContainer is registered by ScrollPlugin, not CorePlugin
    }

    #[test]
    fn test_scroll_plugin_registers_scroll_container() {
        let mut manager = PluginManager::new();
        manager.add_plugin(crate::scroll_plugin::ScrollPlugin::new());

        let registry = manager.widget_registry();
        assert!(registry.contains(std::any::TypeId::of::<
            crate::widgets::scroll_container::ScrollContainer,
        >()));
    }

    #[test]
    fn test_widget_type_descriptor_builder() {
        let desc = WidgetTypeDescriptor::new("Test").with_clips_children(|_| true);

        assert_eq!(desc.name, "Test");
        assert!(desc.clips_children.is_some());
        assert!(desc.measure.is_none());
        assert!(desc.traversal.is_none());
        assert!(desc.scroll_offset.is_none());
    }

    #[test]
    fn test_registry_len() {
        let mut registry = WidgetTypeRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register::<TestPlugin>(WidgetTypeDescriptor::new("Test"));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }
}
