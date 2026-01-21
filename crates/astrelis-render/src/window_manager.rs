///! WindowManager - Manages multiple windows and eliminates boilerplate
///!
///! This module provides a high-level abstraction for managing multiple windows,
///! automatically handling common events like resizing and providing a clean API
///! for rendering.

use std::collections::HashMap;
use std::sync::Arc;

use astrelis_winit::{
    WindowId,
    app::AppCtx,
    event::{Event, EventBatch, HandleStatus},
    window::WindowDescriptor,
};

use crate::{
    context::GraphicsContext,
    window::{RenderableWindow, WindowContextDescriptor},
};

/// Manages multiple renderable windows with automatic event handling.
///
/// The WindowManager eliminates the boilerplate of manually managing a
/// `HashMap<WindowId, RenderableWindow>` and handling common events like resizing.
///
/// # Example
///
/// ```no_run
/// use astrelis_render::{WindowManager, GraphicsContext, RenderTarget, Color};
/// use astrelis_winit::app::{App, AppCtx};
/// use astrelis_winit::{WindowId, FrameTime};
/// use astrelis_winit::window::WindowBackend;
/// use astrelis_winit::event::EventBatch;
///
/// struct MyApp {
///     window_manager: WindowManager,
/// }
///
/// impl App for MyApp {
///     fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {}
///     fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
///         self.window_manager.render_window(window_id, events, |window, _events| {
///             // Resize already handled automatically!
///             let mut frame = window.begin_drawing();
///             frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |_pass| {
///                 // Your rendering here
///             });
///             frame.finish();
///         });
///     }
/// }
/// ```
pub struct WindowManager {
    graphics: Arc<GraphicsContext>,
    windows: HashMap<WindowId, RenderableWindow>,
}

impl WindowManager {
    /// Creates a new WindowManager with the given graphics context.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use astrelis_render::{WindowManager, GraphicsContext};
    /// use std::sync::Arc;
    ///
    /// let graphics = GraphicsContext::new_owned_sync_or_panic();
    /// let window_manager = WindowManager::new(graphics);
    /// ```
    pub fn new(graphics: Arc<GraphicsContext>) -> Self {
        Self {
            graphics,
            windows: HashMap::new(),
        }
    }

    /// Creates a new window and adds it to the manager.
    ///
    /// Returns the WindowId of the created window.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use astrelis_render::WindowManager;
    /// use astrelis_winit::window::{WindowDescriptor, WindowBackend};
    ///
    /// # fn example(window_manager: &mut WindowManager, ctx: &mut astrelis_winit::app::AppCtx) {
    /// let window_id = window_manager.create_window(
    ///     ctx,
    ///     WindowDescriptor {
    ///         title: "My Window".to_string(),
    ///         ..Default::default()
    ///     },
    /// );
    /// # }
    /// ```
    pub fn create_window(&mut self, ctx: &mut AppCtx, descriptor: WindowDescriptor) -> Result<WindowId, crate::context::GraphicsError> {
        self.create_window_with_descriptor(ctx, descriptor, WindowContextDescriptor::default())
    }

    /// Creates a new window with a custom rendering context descriptor.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use astrelis_render::{WindowManager, WindowContextDescriptor, wgpu};
    /// use astrelis_winit::window::{WindowDescriptor, WindowBackend};
    ///
    /// # fn example(window_manager: &mut WindowManager, ctx: &mut astrelis_winit::app::AppCtx) {
    /// let window_id = window_manager.create_window_with_descriptor(
    ///     ctx,
    ///     WindowDescriptor::default(),
    ///     WindowContextDescriptor {
    ///         present_mode: Some(wgpu::PresentMode::Mailbox),
    ///         ..Default::default()
    ///     },
    /// );
    /// # }
    /// ```
    pub fn create_window_with_descriptor(
        &mut self,
        ctx: &mut AppCtx,
        descriptor: WindowDescriptor,
        window_descriptor: WindowContextDescriptor,
    ) -> Result<WindowId, crate::context::GraphicsError> {
        let window = ctx.create_window(descriptor).expect("Failed to create window");
        let id = window.id();
        let renderable = RenderableWindow::new_with_descriptor(window, self.graphics.clone(), window_descriptor)?;
        self.windows.insert(id, renderable);
        Ok(id)
    }

    /// Gets a reference to a window by its ID.
    ///
    /// Returns `None` if the window doesn't exist.
    pub fn get_window(&self, id: WindowId) -> Option<&RenderableWindow> {
        self.windows.get(&id)
    }

    /// Gets a mutable reference to a window by its ID.
    ///
    /// Returns `None` if the window doesn't exist.
    pub fn get_window_mut(&mut self, id: WindowId) -> Option<&mut RenderableWindow> {
        self.windows.get_mut(&id)
    }

    /// Removes a window from the manager.
    ///
    /// Returns the removed window if it existed.
    pub fn remove_window(&mut self, id: WindowId) -> Option<RenderableWindow> {
        self.windows.remove(&id)
    }

    /// Returns the number of windows being managed.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Returns an iterator over all window IDs.
    pub fn window_ids(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.windows.keys().copied()
    }

    /// Renders a window with automatic event handling.
    ///
    /// This method:
    /// 1. Automatically handles common events (resize, etc.)
    /// 2. Calls your render closure with the window and remaining events
    /// 3. Returns immediately if the window doesn't exist
    ///
    /// # Example
    ///
    /// ```no_run
    /// use astrelis_render::{WindowManager, RenderTarget, Color};
    /// use astrelis_winit::window::WindowBackend;
    ///
    /// # fn example(window_manager: &mut WindowManager, window_id: astrelis_winit::WindowId, events: &mut astrelis_winit::event::EventBatch) {
    /// window_manager.render_window(window_id, events, |window, events| {
    ///     // Handle custom events if needed
    ///     events.dispatch(|_event| {
    ///         // Your event handling
    ///         astrelis_winit::event::HandleStatus::ignored()
    ///     });
    ///
    ///     // Render
    ///     let mut frame = window.begin_drawing();
    ///     frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |_pass| {
    ///         // Your rendering
    ///     });
    ///     frame.finish();
    /// });
    /// # }
    /// ```
    pub fn render_window<F>(&mut self, id: WindowId, events: &mut EventBatch, mut render_fn: F)
    where
        F: FnMut(&mut RenderableWindow, &mut EventBatch),
    {
        let Some(window) = self.windows.get_mut(&id) else {
            return;
        };

        // Handle common events automatically
        events.dispatch(|event| {
            match event {
                Event::WindowResized(size) => {
                    window.resized(*size);
                    HandleStatus::consumed()
                }
                _ => HandleStatus::ignored(),
            }
        });

        // Call user's render function with remaining events
        render_fn(window, events);
    }

    /// Renders a window with automatic event handling, passing a closure that returns a result.
    ///
    /// This is useful when rendering might fail and you want to propagate errors.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use astrelis_render::{WindowManager, RenderTarget, Color};
    /// use astrelis_winit::window::WindowBackend;
    ///
    /// # fn example(window_manager: &mut WindowManager, window_id: astrelis_winit::WindowId, events: &mut astrelis_winit::event::EventBatch) -> Result<(), String> {
    /// window_manager.render_window_result(window_id, events, |window, _events| {
    ///     let mut frame = window.begin_drawing();
    ///     frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |_pass| {
    ///         // Rendering that might fail
    ///     });
    ///     frame.finish();
    ///     Ok(())
    /// })
    /// # }
    /// ```
    pub fn render_window_result<F, E>(&mut self, id: WindowId, events: &mut EventBatch, mut render_fn: F) -> Result<(), E>
    where
        F: FnMut(&mut RenderableWindow, &mut EventBatch) -> Result<(), E>,
    {
        let Some(window) = self.windows.get_mut(&id) else {
            // Window doesn't exist - not an error, just skip
            return Ok(());
        };

        // Handle common events automatically
        events.dispatch(|event| {
            match event {
                Event::WindowResized(size) => {
                    window.resized(*size);
                    HandleStatus::consumed()
                }
                _ => HandleStatus::ignored(),
            }
        });

        // Call user's render function with remaining events
        render_fn(window, events)
    }

    /// Gets the shared graphics context.
    pub fn graphics(&self) -> &Arc<GraphicsContext> {
        &self.graphics
    }

    /// Iterates over all windows with their IDs.
    pub fn iter(&self) -> impl Iterator<Item = (WindowId, &RenderableWindow)> {
        self.windows.iter().map(|(&id, window)| (id, window))
    }

    /// Iterates mutably over all windows with their IDs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (WindowId, &mut RenderableWindow)> {
        self.windows.iter_mut().map(|(&id, window)| (id, window))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_manager_creation() {
        let graphics = GraphicsContext::new_owned_sync_or_panic();
        let manager = WindowManager::new(graphics.clone());

        assert_eq!(manager.window_count(), 0);
        assert_eq!(manager.graphics().as_ref() as *const _, graphics.as_ref() as *const _);
    }

    #[test]
    fn test_window_manager_window_count() {
        let graphics = GraphicsContext::new_owned_sync_or_panic();
        let manager = WindowManager::new(graphics);

        assert_eq!(manager.window_count(), 0);

        // Note: We can't actually create windows without a running event loop,
        // so this test just verifies the count starts at 0
    }

    #[test]
    fn test_window_manager_window_ids_empty() {
        let graphics = GraphicsContext::new_owned_sync_or_panic();
        let manager = WindowManager::new(graphics);

        assert_eq!(manager.window_ids().count(), 0);
    }
}
