//! Render plugin for graphics context management.

use std::collections::HashMap;

use astrelis_core::geometry::Size;
use astrelis_render::{GraphicsContext, WindowContext, WindowContextDescriptor};
use astrelis_winit::window::Window;
use astrelis_winit::WindowId;

use crate::plugin::Plugin;
use crate::resource::Resources;

/// Manages render contexts for multiple windows.
pub struct RenderContexts {
    graphics: Option<&'static GraphicsContext>,
    contexts: HashMap<WindowId, WindowContext>,
}

impl Default for RenderContexts {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderContexts {
    /// Create a new render context manager.
    pub fn new() -> Self {
        Self {
            graphics: None,
            contexts: HashMap::new(),
        }
    }

    /// Create a new render context manager with a graphics context.
    pub fn with_graphics(graphics: &'static GraphicsContext) -> Self {
        Self {
            graphics: Some(graphics),
            contexts: HashMap::new(),
        }
    }

    /// Create a render context for a window.
    ///
    /// The window is consumed and owned by the WindowContext.
    pub fn create_for_window(&mut self, window: Window) -> &mut WindowContext {
        let graphics = self
            .graphics
            .expect("RenderContexts must be initialized with a GraphicsContext");
        self.create_for_window_with(window, graphics, WindowContextDescriptor::default())
    }

    /// Create a render context for a window with custom settings.
    pub fn create_for_window_with(
        &mut self,
        window: Window,
        graphics: &'static GraphicsContext,
        descriptor: WindowContextDescriptor,
    ) -> &mut WindowContext {
        let window_id = window.id();

        if !self.contexts.contains_key(&window_id) {
            let context = WindowContext::new(window, graphics, descriptor);
            self.contexts.insert(window_id, context);
        }

        self.contexts.get_mut(&window_id).unwrap()
    }

    /// Get a render context for a window.
    pub fn get(&self, window_id: WindowId) -> Option<&WindowContext> {
        self.contexts.get(&window_id)
    }

    /// Get a mutable render context for a window.
    pub fn get_mut(&mut self, window_id: WindowId) -> Option<&mut WindowContext> {
        self.contexts.get_mut(&window_id)
    }

    /// Remove a render context for a window.
    pub fn remove(&mut self, window_id: WindowId) -> Option<WindowContext> {
        self.contexts.remove(&window_id)
    }

    /// Check if a context exists for a window.
    pub fn contains(&self, window_id: WindowId) -> bool {
        self.contexts.contains_key(&window_id)
    }

    /// Notify that a window has been resized.
    pub fn resized(&mut self, window_id: WindowId, new_size: Size<u32>) {
        if let Some(context) = self.contexts.get_mut(&window_id) {
            context.resized(new_size);
        }
    }

    /// Get the graphics context.
    pub fn graphics(&self) -> Option<&'static GraphicsContext> {
        self.graphics
    }
}

/// Plugin that provides GPU rendering capabilities.
///
/// This plugin creates the graphics context and manages render
/// contexts for windows.
///
/// # Resources Provided
///
/// - `&'static GraphicsContext` - The main GPU context (static lifetime)
/// - `RenderContexts` - Manager for window render contexts
///
/// # Example
///
/// ```ignore
/// use astrelis::prelude::*;
///
/// // In your App::render():
/// fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
///     let render_contexts = self.engine.get_mut::<RenderContexts>().unwrap();
///     
///     if let Some(render_ctx) = render_contexts.get_mut(window_id) {
///         let mut frame = render_ctx.begin_drawing();
///         // Render...
///     }
/// }
/// ```
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn name(&self) -> &'static str {
        "RenderPlugin"
    }

    fn build(&self, resources: &mut Resources) {
        // Create graphics context (returns 'static lifetime)
        let graphics: &'static GraphicsContext = GraphicsContext::new_sync();

        tracing::info!(
            "RenderPlugin: GraphicsContext created (backend: {:?})",
            graphics.info().backend
        );

        resources.insert(graphics);
        resources.insert(RenderContexts::with_graphics(graphics));
    }
}

#[cfg(test)]
mod tests {
    // Note: These tests require a GPU, so they're marked as ignored by default
    // Run with: cargo test --features render,winit -- --ignored

    #[test]
    #[ignore = "Requires GPU"]
    fn test_render_plugin() {
        use super::*;
        use crate::EngineBuilder;

        let engine = EngineBuilder::new().add_plugin(RenderPlugin).build();

        assert!(engine.get::<&'static GraphicsContext>().is_some());
        assert!(engine.get::<RenderContexts>().is_some());
    }
}
