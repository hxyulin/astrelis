///! WindowManager example demonstrating simplified multi-window management.
///!
///! This example creates 3 windows using the WindowManager abstraction:
///! - Red window
///! - Green window
///! - Blue window
///!
///! WindowManager automatically handles:
///! - Window resize events
///! - Graphics context sharing
///! - HashMap boilerplate elimination
///!
///! Compare this to multi_window.rs to see the boilerplate reduction!

use astrelis_core::logging;
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, WindowContextDescriptor, WindowManager,
};
use astrelis_winit::{
    FrameTime,
    WindowId,
    app::{run_app, App, AppCtx},
    event::EventBatch,
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
};
use std::collections::HashMap;

struct WindowManagerApp {
    window_manager: WindowManager,
    window_colors: HashMap<WindowId, Color>,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync_or_panic();
        let mut window_manager = WindowManager::new(graphics_ctx);
        let mut window_colors = HashMap::new();

        // Create 3 windows with different colors
        let colors = [
            Color::rgb(0.8, 0.2, 0.2), // Red
            Color::rgb(0.2, 0.8, 0.2), // Green
            Color::rgb(0.2, 0.2, 0.8), // Blue
        ];

        for (i, color) in colors.iter().enumerate() {
            let window_id = window_manager.create_window_with_descriptor(
                ctx,
                WindowDescriptor {
                    title: format!("Window {} - WindowManager Demo", i + 1),
                    size: Some(WinitPhysicalSize::new(400.0, 300.0)),
                    ..Default::default()
                },
                WindowContextDescriptor {
                    format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                    ..Default::default()
                },
            ).expect("Failed to create window");

            window_colors.insert(window_id, *color);
        }

        Box::new(WindowManagerApp {
            window_manager,
            window_colors,
        })
    });
}

impl App for WindowManagerApp {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        // Global logic - called once per frame
        // (none needed for this example)
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Get the color for this window
        let Some(&color) = self.window_colors.get(&window_id) else {
            return;
        };

        // WindowManager automatically handles:
        // 1. Window lookup (no manual HashMap.get_mut)
        // 2. Resize events (automatic)
        // 3. Event dispatching
        self.window_manager
            .render_window(window_id, events, |window, _events| {
                // No need to manually handle resize events!
                // WindowManager already did that for us

                // Just render!
                let mut frame = window.begin_drawing();

                frame.clear_and_render(RenderTarget::Surface, color, |_pass| {
                    // Additional rendering would go here
                });

                frame.finish();
            });
    }
}
