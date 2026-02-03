//! Minimal Application Example - Using ApplicationBuilder
//!
//! This example demonstrates the ApplicationBuilder API that eliminates
//! 35-50 lines of boilerplate code. Compare this to the traditional approach
//! in the render examples.
//!
//! # Boilerplate Reduction
//!
//! Traditional approach requires (~50 lines):
//! - Manual GraphicsContext creation
//! - Manual window creation with descriptors
//! - Manual Engine building
//! - Manual HashMap management for windows
//! - Manual resize event handling
//!
//! ApplicationBuilder approach (~15 lines):
//! - Declarative builder pattern
//! - Automatic window creation
//! - Automatic plugin initialization
//! - Automatic WindowManager setup
//!
//! Run with: cargo run --example simple_app

use astrelis::prelude::*;

struct SimpleApp;

impl astrelis_winit::app::App for SimpleApp {
    fn on_start(&mut self, _ctx: &mut astrelis_winit::app::AppCtx) {
        println!("✓ App started!");
        println!("✓ Window created automatically by ApplicationBuilder");
        println!("✓ WindowManager initialized automatically");
    }

    fn update(&mut self, _ctx: &mut astrelis_winit::app::AppCtx, time: &astrelis_winit::FrameTime) {
        // Application logic here
        if time.frame_count % 60 == 0 {
            println!("Frame {}: {:.1} FPS", time.frame_count, 1.0 / time.delta.as_secs_f32());
        }
    }

    fn render(
        &mut self,
        _ctx: &mut astrelis_winit::app::AppCtx,
        window_id: astrelis_winit::WindowId,
        events: &mut astrelis_winit::event::EventBatch,
    ) {
        // For this simple example, we don't actually render anything
        // In a real app, you'd store the WindowManager reference
        // and use it to render
        let _ = (window_id, events);
    }

    fn on_exit(&mut self, _ctx: &mut astrelis_winit::app::AppCtx) {
        println!("✓ App exiting gracefully!");
    }
}

fn main() {
    astrelis_core::logging::init();

    // This is all the boilerplate you need!
    ApplicationBuilder::new()
        .with_title("Simple Application - ApplicationBuilder Demo")
        .with_size(800, 600)
        .add_plugins(DefaultPlugins)
        .run(|_ctx, _engine| {
            // Engine is already built with plugins
            // Window is already created and ready to use
            SimpleApp
        });
}

