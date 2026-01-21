//! Material System Demo - Shader Parameter Management
//!
//! This example demonstrates the Material API for managing shader parameters:
//! - Setting shader parameters (floats, vectors, matrices, colors)
//! - Binding textures to materials
//! - Material instancing
//! - Parameter updates and hot-reloading
//!
//! Materials provide a high-level API for shader parameter management without
//! dealing with low-level buffer binding and layout details.
//!
//! **Note:** This demonstrates the Material API structure. Full rendering integration
//! with custom shaders is in development.

use astrelis_core::logging;
use astrelis_render::{Color, GraphicsContext, RenderTarget, RenderableWindow, WindowContextDescriptor, wgpu};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{WinitPhysicalSize, WindowBackend, WindowDescriptor},
};
use std::sync::Arc;

struct MaterialSystemDemo {
    _context: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    time: f32,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync_or_panic();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Material System Demo - Shader Parameters".to_string(),
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
        ).expect("Failed to create renderable window");

        let window_id = window.id();

        // Demonstration of Material API usage
        // In actual use, you would create materials with shaders:
        //
        // let material = Material::new(shader, graphics_ctx.clone());
        // material.set_parameter("base_color", MaterialParameter::Color(Color::RED));
        // material.set_parameter("roughness", MaterialParameter::Float(0.5));
        // material.set_parameter("view_matrix", MaterialParameter::Matrix4(view_matrix));
        // material.set_texture("albedo", texture_handle);
        // material.bind(&mut render_pass);

        println!("\n═══════════════════════════════════════════════════════");
        println!("  🎨 MATERIAL SYSTEM DEMO - Shader Parameters");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  MATERIAL API FEATURES:");
        println!("    • Type-safe parameter setting (float, vec, matrix, color)");
        println!("    • Texture binding and management");
        println!("    • Automatic buffer creation and updates");
        println!("    • Material instancing for performance");
        println!("    • Hot-reloadable shader parameters");
        println!("\n  EXAMPLE MATERIAL TYPES:");
        println!("    1. Color Material - PBR properties (color, roughness, metallic)");
        println!("    2. Textured Material - UV transforms (offset, scale, tint)");
        println!("    3. Animated Material - Time-based effects (frequency, amplitude)");
        println!("    4. Transform Material - View/projection matrices");
        println!("\n  Material API Usage:");
        println!("    material.set_parameter(\"color\", MaterialParameter::Color(..))");
        println!("    material.set_parameter(\"time\", MaterialParameter::Float(..))");
        println!("    material.set_parameter(\"matrix\", MaterialParameter::Matrix4(..))");
        println!("    material.set_texture(\"albedo\", texture_handle)");
        println!("    material.bind(&mut render_pass)");
        println!("\n  Materials abstract shader parameter management,");
        println!("  eliminating manual buffer binding boilerplate.");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Material system demo initialized");

        Box::new(MaterialSystemDemo {
            _context: graphics_ctx,
            window,
            window_id,
            time: 0.0,
        })
    });
}

impl App for MaterialSystemDemo {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        self.time += 0.016; // 60 FPS
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle resize
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        // In a real application, materials would be bound during rendering:
        // material.bind(&mut render_pass);
        // draw_mesh(&mesh);

        // Begin frame
        let mut frame = self.window.begin_drawing();

        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb_u8(20, 30, 40),
            |_pass| {
                // Materials would be applied here in actual rendering
                // This is a conceptual demonstration
            },
        );

        frame.finish();
    }
}
