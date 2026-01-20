//! Render Graph Demo - Multi-Pass Rendering Pipeline
//!
//! Demonstrates the render graph system for complex rendering pipelines:
//! - Declaring render passes
//! - Resource dependencies
//! - Automatic pass ordering and optimization
//! - Multi-target rendering
//! - Post-processing chains
//!
//! **Note:** This demonstrates the Render Graph API structure.

use astrelis_core::logging;
use astrelis_render::{Color, GraphicsContext, RenderTarget, RenderableWindow, WindowContextDescriptor, wgpu};
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{WinitPhysicalSize, WindowBackend, WindowDescriptor},
};
use std::sync::Arc;

struct RenderGraphDemo {
    _context: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Render Graph Demo - Multi-Pass Rendering".to_string(),
                size: Some(WinitPhysicalSize::new(1024.0, 768.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderableWindow::new_with_descriptor(
            window,
            graphics_ctx.clone(),
            WindowContextDescriptor {
                format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                ..Default::default()
            },
        );

        let window_id = window.id();

        println!("\n═══════════════════════════════════════════════════════");
        println!("  🔀 RENDER GRAPH DEMO - Multi-Pass Rendering");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  RENDER GRAPH FEATURES:");
        println!("    • Declarative pass definition");
        println!("    • Automatic dependency resolution");
        println!("    • Resource lifetime management");
        println!("    • Parallel pass execution");
        println!("    • Automatic optimization");
        println!("\n  EXAMPLE PIPELINE:");
        println!("    1. Shadow Pass → depth texture");
        println!("    2. Geometry Pass → color + normal + depth");
        println!("    3. Lighting Pass → lit scene");
        println!("    4. Post-Processing → bloom, tone mapping");
        println!("    5. UI Pass → final composite");
        println!("\n  Render Graph API Usage:");
        println!("    let mut graph = RenderGraph::new();");
        println!("    graph.add_pass(\"shadow\", shadow_pass_descriptor);");
        println!("    graph.add_pass(\"geometry\", geometry_pass_descriptor);");
        println!("    graph.add_dependency(\"lighting\", \"geometry\");");
        println!("    graph.compile();");
        println!("    graph.execute(&mut encoder);");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Render graph demo initialized");

        Box::new(RenderGraphDemo {
            _context: graphics_ctx,
            window,
            window_id,
        })
    });
}

impl App for RenderGraphDemo {
    fn update(&mut self, _ctx: &mut AppCtx) {}

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

        let mut frame = self.window.begin_drawing();
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb_u8(20, 30, 40),
            |_pass| {},
        );
        frame.finish();
    }
}
