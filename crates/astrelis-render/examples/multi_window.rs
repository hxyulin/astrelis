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
    app::run_app,
    event::PhysicalSize,
    window::{WindowBackend, WindowDescriptor},
};

struct App {
    context: &'static GraphicsContext,
    windows: Vec<(RenderableWindow, wgpu::Color)>,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_sync();

        let mut windows = Vec::new();

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
                },
            );

            windows.push((renderable_window, *color));
        }

        Box::new(App {
            context: graphics_ctx,
            windows,
        })
    });
}

impl astrelis_winit::app::App for App {
    fn update(
        &mut self,
        _ctx: &mut astrelis_winit::app::AppCtx,
        _events: &mut astrelis_winit::event::EventBatch,
    ) {
        for (window, color) in &mut self.windows {
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
}
