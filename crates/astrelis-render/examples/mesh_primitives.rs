//! Mesh Primitives Demo - MeshBuilder API
//!
//! Demonstrates the Mesh abstraction for geometry management:
//! - MeshBuilder API for custom geometry
//! - Primitive generation (cube, sphere, plane, cylinder)
//! - Vertex formats (position, normal, UV, color)
//! - Index buffer management
//! - Instanced rendering
//!
//! **Note:** This is a placeholder example demonstrating the Mesh API structure.
//! Full rendering integration is in development.

use astrelis_core::logging;
use astrelis_render::{Color, GraphicsContext, RenderWindow, RenderWindowBuilder, wgpu};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
};
use std::sync::Arc;

struct MeshPrimitivesDemo {
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
                title: "Mesh Primitives Demo - Geometry API".to_string(),
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
        println!("  📐 MESH PRIMITIVES DEMO - Geometry API");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  MESH API FEATURES:");
        println!("    • MeshBuilder for custom geometry");
        println!("    • Primitive generation (cube, sphere, plane, etc.)");
        println!("    • Flexible vertex formats (Position, Normal, UV, Color)");
        println!("    • Index buffer optimization");
        println!("    • Instanced rendering support");
        println!("\n  EXAMPLE PRIMITIVES:");
        println!("    • Cube - box with 24 vertices (6 faces × 4 vertices)");
        println!("    • Sphere - tessellated sphere with UV mapping");
        println!("    • Plane - quad with optional subdivisions");
        println!("    • Cylinder - sides + caps");
        println!("    • Custom - arbitrary vertex/index data");
        println!("\n  Mesh API Usage:");
        println!("    let mesh = MeshBuilder::new()");
        println!("        .with_positions(vertices)");
        println!("        .with_normals(normals)");
        println!("        .with_uvs(uvs)");
        println!("        .with_indices(indices)");
        println!("        .build(&ctx);");
        println!("    mesh.draw(&mut pass);");
        println!("    mesh.draw_instanced(&mut pass, instance_count);");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Mesh primitives demo initialized");

        Box::new(MeshPrimitivesDemo {
            _context: graphics_ctx,
            window,
            window_id,
        })
    });
}

impl App for MeshPrimitivesDemo {
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
                .label("mesh_primitives_pass")
                .build();
        }
        // Frame auto-submits on drop
    }
}
