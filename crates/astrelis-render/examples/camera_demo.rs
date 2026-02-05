//! Camera Demo - Orthographic and Perspective Cameras
//!
//! Demonstrates camera systems for 3D rendering:
//! - Orthographic cameras (2D/UI, isometric games)
//! - Perspective cameras (3D scenes)
//! - View matrix construction
//! - Projection matrix construction
//! - Screen-to-world conversion
//! - Camera movement and controls
//!
//! **Note:** This is a placeholder example demonstrating the Camera API structure.
//! Full interactive camera controls are in development.

use astrelis_core::logging;
use astrelis_render::{Color, GraphicsContext, RenderWindow, RenderWindowBuilder, wgpu};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
};
use std::sync::Arc;

struct CameraDemo {
    _context: Arc<GraphicsContext>,
    window: RenderWindow,
    window_id: WindowId,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx =
            GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Camera Demo - View & Projection".to_string(),
                size: Some(WinitPhysicalSize::new(1024.0, 768.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderWindowBuilder::new()
            .color_format(wgpu::TextureFormat::Bgra8UnormSrgb)
            .with_depth_default()
            .build(window, graphics_ctx.clone())
            .expect("Failed to create render window");

        let window_id = window.id();

        println!("\n═══════════════════════════════════════════════════════");
        println!("  📹 CAMERA DEMO - View & Projection");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  CAMERA API FEATURES:");
        println!("    • Orthographic cameras (2D, UI, isometric)");
        println!("    • Perspective cameras (3D scenes)");
        println!("    • View matrix (position, rotation, look-at)");
        println!("    • Projection matrix (FOV, aspect, near/far planes)");
        println!("    • Screen-to-world coordinate conversion");
        println!("    • Camera movement helpers");
        println!("\n  CAMERA TYPES:");
        println!("    • OrthographicCamera - 2D games, UI overlays");
        println!("      camera.orthographic(left, right, bottom, top, near, far)");
        println!("    • PerspectiveCamera - 3D scenes");
        println!("      camera.perspective(fov, aspect, near, far)");
        println!("\n  Camera API Usage:");
        println!("    let camera = Camera::new()");
        println!("        .position(Vec3::new(0.0, 5.0, 10.0))");
        println!("        .look_at(Vec3::ZERO)");
        println!("        .perspective(60.0, aspect, 0.1, 100.0);");
        println!("    let view_proj = camera.view_projection_matrix();");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Camera demo initialized");

        Box::new(CameraDemo {
            _context: graphics_ctx,
            window,
            window_id,
        })
    });
}

impl App for CameraDemo {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {}

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        let Some(frame) = self.window.begin_frame() else {
            return; // Surface not available
        };
        {
            let _pass = frame
                .render_pass()
                .clear_color(Color::from_rgb_u8(20, 30, 40))
                .label("camera_demo_pass")
                .build();
        }
        // Frame auto-submits on drop
    }
}
