//! Render plugin for graphics context management.

use std::collections::HashMap;
use std::sync::Arc;

use astrelis_core::geometry::LogicalSize;
use astrelis_render::{GraphicsContext, WindowContext, WindowContextDescriptor, WindowManager};
use astrelis_winit::WindowId;
use astrelis_winit::window::Window;

use crate::plugin::Plugin;
use crate::resource::Resources;

/// Manages render contexts for multiple windows.
pub struct RenderContexts {
    graphics: Option<Arc<GraphicsContext>>,
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
    pub fn with_graphics(graphics: Arc<GraphicsContext>) -> Self {
        Self {
            graphics: Some(graphics),
            contexts: HashMap::new(),
        }
    }

    /// Create a render context for a window.
    ///
    /// The window is consumed and owned by the WindowContext.
    pub fn create_for_window(
        &mut self,
        window: Window,
    ) -> Result<&mut WindowContext, astrelis_render::GraphicsError> {
        let graphics = self
            .graphics
            .as_ref()
            .expect("RenderContexts must be initialized with a GraphicsContext")
            .clone();
        self.create_for_window_with(window, graphics, WindowContextDescriptor::default())
    }

    /// Create a render context for a window with custom settings.
    pub fn create_for_window_with(
        &mut self,
        window: Window,
        graphics: Arc<GraphicsContext>,
        descriptor: WindowContextDescriptor,
    ) -> Result<&mut WindowContext, astrelis_render::GraphicsError> {
        let window_id = window.id();

        if let std::collections::hash_map::Entry::Vacant(e) = self.contexts.entry(window_id) {
            let context = WindowContext::new(window, graphics, descriptor)?;
            e.insert(context);
        }

        Ok(self.contexts.get_mut(&window_id).unwrap())
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
    pub fn resized(&mut self, window_id: WindowId, new_size: LogicalSize<u32>) {
        if let Some(context) = self.contexts.get_mut(&window_id) {
            context.resized(new_size);
        }
    }

    /// Get the graphics context.
    pub fn graphics(&self) -> Option<&GraphicsContext> {
        self.graphics.as_deref()
    }
}

/// Plugin that provides GPU rendering capabilities.
///
/// This plugin creates the graphics context and manages render
/// contexts for windows.
///
/// # Resources Provided
///
/// - `Arc<GraphicsContext>` - The main GPU context (shared ownership)
/// - `RenderContexts` - Manager for window render contexts (legacy)
/// - `WindowManager` - High-level window management with automatic event handling (recommended)
///
/// # Example (Using WindowManager - Recommended)
///
/// ```ignore
/// use astrelis::prelude::*;
///
/// struct MyApp {
///     window_manager: Option<WindowManager>,
/// }
///
/// impl App for MyApp {
///     fn on_start(&mut self, ctx: &mut AppCtx) {
///         // Get WindowManager from engine resources
///         let window_manager = self.engine.get::<WindowManager>().unwrap().clone();
///         self.window_manager = Some(window_manager);
///     }
///
///     fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
///         let wm = self.window_manager.as_mut().unwrap();
///         wm.render_window(window_id, events, |window, _events| {
///             // Resize handled automatically!
///             let mut frame = window.begin_drawing();
///             frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
///                 // Your rendering
///             });
///             frame.finish();
///         });
///     }
/// }
/// ```
///
/// # Example (Using RenderContexts - Legacy)
///
/// ```ignore
/// use astrelis::prelude::*;
///
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
    type Dependencies = ();
    fn name(&self) -> &'static str {
        "RenderPlugin"
    }

    fn build(&self, resources: &mut Resources) {
        // Create graphics context with Arc (no memory leak)
        let graphics =
            GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        tracing::info!(
            "RenderPlugin: GraphicsContext created (backend: {:?})",
            graphics.info().backend
        );

        resources.insert(graphics.clone());
        resources.insert(RenderContexts::with_graphics(graphics.clone()));
        resources.insert(WindowManager::new(graphics));

        tracing::debug!(
            "RenderPlugin: Registered GraphicsContext, RenderContexts, and WindowManager"
        );
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
