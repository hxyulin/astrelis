//! Example demonstrating the Astrelis engine integrated with the windowing system.
//!
//! This example shows how to:
//! - Create an Engine with RenderPlugin
//! - Use the engine within an App
//! - Render to a window using the plugin-provided resources
//!
//! Run with: cargo run -p astrelis --example window_app

use astrelis::prelude::*;
use astrelis::render::{RenderTarget, RenderableWindow, WindowContextDescriptor};
use astrelis::winit::window::WindowBackend;
use std::sync::Arc;

struct WindowApp {
    #[allow(dead_code)]
    engine: Engine,
    renderable: Option<RenderableWindow>,
}

impl App for WindowApp {
    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Get the renderable window
        let renderable = match &mut self.renderable {
            Some(r) if r.id() == window_id => r,
            _ => return,
        };

        // Handle events
        events.dispatch(|event| {
            match event {
                Event::WindowResized(new_size) => {
                    renderable.resized(*new_size);
                    HandleStatus::consumed()
                }
                Event::CloseRequested => {
                    HandleStatus::ignored() // Let default handling close the window
                }
                _ => HandleStatus::ignored(),
            }
        });

        // Begin drawing
        let mut frame = renderable.begin_drawing();

        // Clear with automatic scoping (no manual {} block needed)
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::rgb(0.1, 0.1, 0.15), // Dark blue-gray
            |_pass| {
                // In a real app, you would draw here
            },
        );

        // Frame is automatically submitted when dropped
    }
}

fn main() {
    println!("Window App Example");
    println!("==================");
    println!();
    println!("This example demonstrates:");
    println!("  - Creating an Engine with RenderPlugin");
    println!("  - Using RenderableWindow for window rendering");
    println!("  - Handling window events (resize, close)");
    println!("  - Rendering a simple clear pass");
    println!();
    println!("Press Ctrl+C or close the window to exit.");
    println!();

    // Run the app
    run_app(|ctx| {
        // Create a window
        let window = ctx
            .create_window(WindowDescriptor {
                title: "Astrelis Window App".to_string(),
                ..Default::default()
            })
            .expect("Failed to create window");

        // Create the engine with render plugin
        let engine = Engine::builder().add_plugin(RenderPlugin).build();

        // Get the graphics context from the engine
        let graphics = engine.get::<Arc<GraphicsContext>>().unwrap();

        // Create a renderable window
        let renderable = RenderableWindow::new_with_descriptor(
            window,
            graphics.clone(),
            WindowContextDescriptor::default(),
        )
        .expect("Failed to create renderable window");

        Box::new(WindowApp {
            engine,
            renderable: Some(renderable),
        })
    });
}
