//! Middleware manager for coordinating multiple middlewares.
//!
//! Handles middleware lifecycle, ordering by priority, and dispatch of
//! callbacks to all registered middlewares.

use astrelis_winit::event::KeyCode;

use super::{
    context::{MiddlewareContext, OverlayContext},
    keybind::{KeybindRegistry, Modifiers},
    overlay_draw_list::OverlayDrawList,
    UiMiddleware,
};
use crate::tree::UiTree;

/// Entry for a registered middleware.
struct MiddlewareEntry {
    middleware: Box<dyn UiMiddleware>,
    priority: i32,
}

/// Manages middleware lifecycle and dispatch.
pub struct MiddlewareManager {
    /// Registered middlewares (sorted by priority).
    middlewares: Vec<MiddlewareEntry>,
    /// Keybind registry.
    keybind_registry: KeybindRegistry,
    /// Overlay draw list for collecting overlay commands.
    overlay_draw_list: OverlayDrawList,
    /// Whether layout is currently frozen/paused.
    layout_frozen: bool,
}

impl Default for MiddlewareManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddlewareManager {
    /// Create a new middleware manager.
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
            keybind_registry: KeybindRegistry::new(),
            overlay_draw_list: OverlayDrawList::new(),
            layout_frozen: false,
        }
    }

    /// Add a middleware to the manager.
    ///
    /// Middlewares are automatically sorted by priority (higher = renders on top).
    pub fn add<M: UiMiddleware + 'static>(&mut self, middleware: M) {
        let priority = middleware.priority();
        self.middlewares.push(MiddlewareEntry {
            middleware: Box::new(middleware),
            priority,
        });

        // Sort by priority ascending (lower priority runs first, higher renders on top)
        self.middlewares.sort_by_key(|e| e.priority);
    }

    /// Remove a middleware by name.
    ///
    /// Returns `true` if a middleware was removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let len_before = self.middlewares.len();
        self.middlewares.retain(|e| e.middleware.name() != name);
        self.keybind_registry.unregister(name);
        self.middlewares.len() < len_before
    }

    /// Get a reference to a middleware by name.
    pub fn get(&self, name: &str) -> Option<&dyn UiMiddleware> {
        for entry in &self.middlewares {
            if entry.middleware.name() == name {
                return Some(entry.middleware.as_ref());
            }
        }
        None
    }

    /// Get a mutable reference to a middleware by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut dyn UiMiddleware> {
        for entry in &mut self.middlewares {
            if entry.middleware.name() == name {
                return Some(entry.middleware.as_mut());
            }
        }
        None
    }

    /// Get the keybind registry.
    pub fn keybind_registry(&self) -> &KeybindRegistry {
        &self.keybind_registry
    }

    /// Get mutable access to the keybind registry.
    pub fn keybind_registry_mut(&mut self) -> &mut KeybindRegistry {
        &mut self.keybind_registry
    }

    /// Check if layout is currently frozen.
    pub fn is_layout_frozen(&self) -> bool {
        self.layout_frozen
    }

    /// Check if any middlewares are registered.
    pub fn has_middlewares(&self) -> bool {
        !self.middlewares.is_empty()
    }

    /// Get the number of registered middlewares.
    pub fn middleware_count(&self) -> usize {
        self.middlewares.len()
    }

    /// Get names of all registered middlewares.
    pub fn middleware_names(&self) -> Vec<&str> {
        self.middlewares.iter().map(|e| e.middleware.name()).collect()
    }

    /// Update all middlewares.
    pub fn update(&mut self, ctx: &MiddlewareContext, tree: &UiTree) {
        for entry in &mut self.middlewares {
            if entry.middleware.is_enabled() {
                entry.middleware.update(ctx, tree);
            }
        }
    }

    /// Call pre_layout on all middlewares.
    ///
    /// Returns `true` if layout should be skipped (any middleware requested pause).
    pub fn pre_layout(&mut self, ctx: &MiddlewareContext) -> bool {
        let mut skip_layout = false;

        for entry in &mut self.middlewares {
            if entry.middleware.is_enabled() && entry.middleware.pre_layout(ctx) {
                skip_layout = true;
            }
        }

        self.layout_frozen = skip_layout;
        skip_layout
    }

    /// Call post_layout on all middlewares.
    pub fn post_layout(&mut self, ctx: &MiddlewareContext) {
        for entry in &mut self.middlewares {
            if entry.middleware.is_enabled() {
                entry.middleware.post_layout(ctx);
            }
        }
    }

    /// Call pre_render on all middlewares.
    pub fn pre_render(&mut self, ctx: &MiddlewareContext) {
        for entry in &mut self.middlewares {
            if entry.middleware.is_enabled() {
                entry.middleware.pre_render(ctx);
            }
        }
    }

    /// Call post_render on all middlewares and collect overlay commands.
    ///
    /// Returns the draw list containing all overlay commands.
    pub fn post_render(&mut self, ctx: &MiddlewareContext) -> &OverlayDrawList {
        self.overlay_draw_list.clear();

        for entry in &mut self.middlewares {
            if entry.middleware.is_enabled() {
                let mut overlay_ctx = OverlayContext::new(&mut self.overlay_draw_list);
                entry.middleware.post_render(ctx, &mut overlay_ctx);
            }
        }

        &self.overlay_draw_list
    }

    /// Handle a keyboard event.
    ///
    /// First checks registered keybinds, then passes to middlewares.
    /// Returns `true` if the event was consumed.
    pub fn handle_key_event(
        &mut self,
        key: KeyCode,
        modifiers: Modifiers,
        pressed: bool,
        ctx: &MiddlewareContext,
    ) -> bool {
        // Only handle on key press
        if !pressed {
            return false;
        }

        // Check registered keybinds first
        let matches = self.keybind_registry.find_matches(key, modifiers);

        for (middleware_name, keybind) in matches {
            // Find the middleware and call its handler
            if let Some(entry) = self.middlewares.iter_mut().find(|e| e.middleware.name() == middleware_name)
                && entry.middleware.is_enabled()
                && entry.middleware.handle_keybind(keybind, ctx)
            {
                return true;
            }
        }

        // Pass to middlewares for direct key handling (in priority order)
        for entry in self.middlewares.iter_mut().rev() {
            if entry.middleware.handle_key_event(key, modifiers, pressed, ctx) {
                return true;
            }
        }

        false
    }

    /// Get the overlay draw list (for testing/inspection).
    pub fn overlay_draw_list(&self) -> &OverlayDrawList {
        &self.overlay_draw_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::keybind::Keybind;
    use crate::event::UiEventSystem;
    use crate::widget_id::WidgetIdRegistry;
    use astrelis_render::Viewport;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;

    struct TestMiddleware {
        name: &'static str,
        priority: i32,
        enabled: bool,
        pre_layout_called: Arc<AtomicBool>,
        post_render_called: Arc<AtomicBool>,
        update_count: Arc<AtomicU32>,
        should_pause: bool,
    }

    impl TestMiddleware {
        fn new(name: &'static str, priority: i32) -> Self {
            Self {
                name,
                priority,
                enabled: true,
                pre_layout_called: Arc::new(AtomicBool::new(false)),
                post_render_called: Arc::new(AtomicBool::new(false)),
                update_count: Arc::new(AtomicU32::new(0)),
                should_pause: false,
            }
        }
    }

    impl UiMiddleware for TestMiddleware {
        fn name(&self) -> &'static str {
            self.name
        }

        fn priority(&self) -> i32 {
            self.priority
        }

        fn pre_layout(&mut self, _ctx: &MiddlewareContext) -> bool {
            self.pre_layout_called.store(true, Ordering::SeqCst);
            self.should_pause
        }

        fn post_render(&mut self, _ctx: &MiddlewareContext, overlay: &mut OverlayContext) {
            self.post_render_called.store(true, Ordering::SeqCst);
            overlay.draw_rect(
                astrelis_core::math::Vec2::ZERO,
                astrelis_core::math::Vec2::new(10.0, 10.0),
                astrelis_render::Color::RED,
            );
        }

        fn update(&mut self, _ctx: &MiddlewareContext, _tree: &UiTree) {
            self.update_count.fetch_add(1, Ordering::SeqCst);
        }

        fn is_enabled(&self) -> bool {
            self.enabled
        }

        fn set_enabled(&mut self, enabled: bool) {
            self.enabled = enabled;
        }
    }

    fn create_test_context() -> (UiTree, UiEventSystem, WidgetIdRegistry) {
        (UiTree::new(), UiEventSystem::new(), WidgetIdRegistry::new())
    }

    #[test]
    fn test_manager_creation() {
        let manager = MiddlewareManager::new();
        assert!(!manager.has_middlewares());
        assert_eq!(manager.middleware_count(), 0);
    }

    #[test]
    fn test_add_middleware() {
        let mut manager = MiddlewareManager::new();
        manager.add(TestMiddleware::new("test1", 100));
        manager.add(TestMiddleware::new("test2", 50));

        assert!(manager.has_middlewares());
        assert_eq!(manager.middleware_count(), 2);

        // Should be sorted by priority
        let names = manager.middleware_names();
        assert_eq!(names[0], "test2"); // Lower priority first
        assert_eq!(names[1], "test1"); // Higher priority second
    }

    #[test]
    fn test_remove_middleware() {
        let mut manager = MiddlewareManager::new();
        manager.add(TestMiddleware::new("test1", 100));
        manager.add(TestMiddleware::new("test2", 50));

        assert!(manager.remove("test1"));
        assert_eq!(manager.middleware_count(), 1);
        assert!(!manager.remove("nonexistent"));
    }

    #[test]
    fn test_get_middleware() {
        let mut manager = MiddlewareManager::new();
        manager.add(TestMiddleware::new("test", 100));

        assert!(manager.get("test").is_some());
        assert!(manager.get("nonexistent").is_none());
        assert!(manager.get_mut("test").is_some());
    }

    #[test]
    fn test_pre_layout_callback() {
        let mut manager = MiddlewareManager::new();
        let pre_layout_called = Arc::new(AtomicBool::new(false));

        let mut middleware = TestMiddleware::new("test", 100);
        middleware.pre_layout_called = pre_layout_called.clone();
        manager.add(middleware);

        let (tree, events, registry) = create_test_context();
        let ctx = MiddlewareContext::new(&tree, &events, &registry, Viewport::default());

        let paused = manager.pre_layout(&ctx);
        assert!(!paused);
        assert!(pre_layout_called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_pre_layout_pause() {
        let mut manager = MiddlewareManager::new();

        let mut middleware = TestMiddleware::new("test", 100);
        middleware.should_pause = true;
        manager.add(middleware);

        let (tree, events, registry) = create_test_context();
        let ctx = MiddlewareContext::new(&tree, &events, &registry, Viewport::default());

        let paused = manager.pre_layout(&ctx);
        assert!(paused);
        assert!(manager.is_layout_frozen());
    }

    #[test]
    fn test_post_render_overlay() {
        let mut manager = MiddlewareManager::new();
        manager.add(TestMiddleware::new("test", 100));

        let (tree, events, registry) = create_test_context();
        let ctx = MiddlewareContext::new(&tree, &events, &registry, Viewport::default());

        let draw_list = manager.post_render(&ctx);
        assert!(!draw_list.is_empty());
        assert_eq!(draw_list.quads().count(), 1);
    }

    #[test]
    fn test_update_callback() {
        let mut manager = MiddlewareManager::new();
        let update_count = Arc::new(AtomicU32::new(0));

        let mut middleware = TestMiddleware::new("test", 100);
        middleware.update_count = update_count.clone();
        manager.add(middleware);

        let (tree, events, registry) = create_test_context();
        let ctx = MiddlewareContext::new(&tree, &events, &registry, Viewport::default());

        manager.update(&ctx, &tree);
        manager.update(&ctx, &tree);
        assert_eq!(update_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_disabled_middleware_skipped() {
        let mut manager = MiddlewareManager::new();
        let update_count = Arc::new(AtomicU32::new(0));

        let mut middleware = TestMiddleware::new("test", 100);
        middleware.enabled = false;
        middleware.update_count = update_count.clone();
        manager.add(middleware);

        let (tree, events, registry) = create_test_context();
        let ctx = MiddlewareContext::new(&tree, &events, &registry, Viewport::default());

        manager.update(&ctx, &tree);
        assert_eq!(update_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_keybind_registry() {
        let mut manager = MiddlewareManager::new();

        manager.keybind_registry_mut().register(
            "test",
            Keybind::key(KeyCode::F12, "Toggle"),
            100,
        );

        let matches = manager.keybind_registry().find_matches(KeyCode::F12, Modifiers::NONE);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_middleware_priority_order() {
        let mut manager = MiddlewareManager::new();

        // Add in random order
        manager.add(TestMiddleware::new("medium", 50));
        manager.add(TestMiddleware::new("high", 100));
        manager.add(TestMiddleware::new("low", 10));

        let names = manager.middleware_names();
        assert_eq!(names[0], "low");
        assert_eq!(names[1], "medium");
        assert_eq!(names[2], "high");
    }
}
