//! Basic window example.
//!
//! Opens a single window with default settings and closes when the user
//! clicks the close button. Uses game mode (continuous polling).
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-window-winit --example basic_window
//! ```

use astrelis_window::backend::{AppHandler, EventLoopContext, WindowBackend};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::WindowEvent;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;
use astrelis_window::WindowBuilder;
use astrelis_window_winit::WinitBackend;

struct App {
    window_id: Option<WindowId>,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        match state {
            AppLifecycle::Resumed => {
                // Create a window when the application is ready.
                let attrs = WindowBuilder::new()
                    .with_title("Astrelis — Basic Window")
                    .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
                    .build();
                self.window_id = Some(ctx.create_window(attrs).expect("failed to create window"));

                // Game mode: poll continuously for maximum frame rate.
                ctx.set_control_flow(ControlFlow::Poll);
            }
            AppLifecycle::Suspended => {
                // On mobile, release GPU resources here.
            }
            AppLifecycle::Exiting => {
                println!("Goodbye!");
            }
        }
    }

    fn on_window_event(
        &mut self,
        ctx: &mut dyn EventLoopContext,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Close requested for window {window_id}");
                ctx.exit();
            }
            WindowEvent::Resized(size) => {
                let phys = size.physical();
                println!("Window resized to {}x{}", phys.width, phys.height);
            }
            WindowEvent::RedrawRequested => {
                // This is where you would issue draw calls.
                // For now, just request the next frame.
                if let Some(win) = ctx.window(window_id) {
                    win.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn on_events_cleared(&mut self, ctx: &mut dyn EventLoopContext) {
        // In game mode, request a redraw every frame.
        if let Some(id) = self.window_id
            && let Some(win) = ctx.window(id)
        {
            win.request_redraw();
        }
    }
}

fn main() {
    let backend = WinitBackend::new().expect("failed to create backend");
    let mut app = App { window_id: None };
    backend.run(&mut app).expect("event loop error");
}
