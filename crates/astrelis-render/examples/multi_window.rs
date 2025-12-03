//! Multi-window example demonstrating multiple RenderableWindows.
//!
//! This example creates 3 windows, each with a different clear color:
//! - Red window
//! - Green window
//! - Blue window
//!
//! Each window shares the same GraphicsContext and renders independently
//! using the RenderPassBuilder API.

use astrelis_core::logging;
use astrelis_render::{
    GraphicsContext, RenderPassBuilder, RenderableWindow, WindowContextDescriptor,
};
use astrelis_winit::{
    WindowId,
    app::run_app,
    event::PhysicalSize,
    window::{WindowBackend, WindowDescriptor},
};
use std::collections::HashMap;

struct App {
    context: &'static GraphicsContext,
    windows: HashMap<WindowId, (RenderableWindow, wgpu::Color)>,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_sync();

        let mut windows = HashMap::new();

        // Create 3 windows with different colors
        let colors = [
            wgpu::Color {
                r: 0.8,
                g: 0.2,
                b: 0.2,
                a: 1.0,
            },
            wgpu::Color {
                r: 0.2,
                g: 0.8,
                b: 0.2,
                a: 1.0,
            },
            wgpu::Color {
                r: 0.2,
                g: 0.2,
                b: 0.8,
                a: 1.0,
            },
        ];

        for (i, color) in colors.iter().enumerate() {
            let window = ctx
                .create_window(WindowDescriptor {
                    title: format!("Window {} - Multi-Window Example", i + 1),
                    size: Some(PhysicalSize::new(400.0, 300.0)),
                    ..Default::default()
                })
                .expect("Failed to create window");

            let renderable_window = RenderableWindow::new_with_descriptor(
                window,
                graphics_ctx,
                WindowContextDescriptor {
                    format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                    ..Default::default()
                },
            );

            let window_id = renderable_window.id();
            windows.insert(window_id, (renderable_window, *color));
        }

        Box::new(App {
            context: graphics_ctx,
            windows,
        })
    });
}

impl astrelis_winit::app::App for App {
    fn update(&mut self, _ctx: &mut astrelis_winit::app::AppCtx) {
        // Global logic - called once per frame
        // (none needed for this example)
    }

    fn render(
        &mut self,
        _ctx: &mut astrelis_winit::app::AppCtx,
        window_id: WindowId,
        events: &mut astrelis_winit::event::EventBatch,
    ) {
        // Get the window and color for this specific window
        let Some((window, color)) = self.windows.get_mut(&window_id) else {
            return;
        };

        // Handle window-specific resize events
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                window.resized(*size);
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        // Render this specific window
        let mut frame = window.begin_drawing();

        {
            let _render_pass = RenderPassBuilder::new()
                .label("Multi-Window Render Pass")
                .color_attachment(
                    None,
                    None,
                    wgpu::Operations {
                        load: wgpu::LoadOp::Clear(*color),
                        store: wgpu::StoreOp::Store,
                    },
                )
                .build(&mut frame);
        }

        frame.finish();
    }
}
