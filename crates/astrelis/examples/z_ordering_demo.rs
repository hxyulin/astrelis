//! Z-ordering test — proves that draw order respects z-index, not submission order.
//!
//! Draws overlapping shapes at different z-indices. The visual result
//! must be consistent regardless of the order `draw_*` calls are made.
//! If z-ordering is broken, shapes will appear in submission order
//! instead of z-index order.
//!
//! Expected result:
//! - Left group: three overlapping squares, blue on top (z=2), green
//!   in the middle (z=1), red at the bottom (z=0) — submitted in
//!   REVERSE order (red last) to prove sorting works.
//! - Right group: same squares but submitted in FORWARD order to
//!   confirm the result is identical.
//! - Bottom: circle (z=1) partially overlapping a rectangle (z=0),
//!   circle should be on top.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis --example z_ordering_demo
//! ```

use astrelis::prelude::*;
use astrelis::gpu::{Gpu, GpuError, Surface};

struct ZOrderPlugin;

impl Plugin for ZOrderPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup(|resources, _ctx| {
            let gpu = resources.get::<Gpu>();
            let surface = resources.get::<Surface>();
            let renderer = Renderer2D::new(&gpu, surface.preferred_format());
            let camera = Camera2D::new(1280, 720);
            drop(gpu);
            drop(surface);
            resources.insert(renderer);
            resources.insert(camera);
        });

        app.add_system(Phase::Render, render);
    }
}

fn render(resources: &Resources) {
    let gpu = resources.get::<Gpu>();
    let mut surface = resources.get_mut::<Surface>();
    let mut renderer = resources.get_mut::<Renderer2D>();
    let camera = resources.get::<Camera2D>();

    let frame = match surface.acquire() {
        Ok(f) => f,
        Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost | GpuError::Timeout) => return,
        Err(e) => panic!("surface error: {e}"),
    };

    let mut encoder = gpu.device().create_command_encoder(Some("frame"));

    // Clear.
    {
        let _pass = encoder.begin_render_pass(&astrelis::gpu::command::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[astrelis::gpu::command::ColorAttachment {
                view: frame.view(),
                resolve_target: None,
                load_op: astrelis::gpu::types::LoadOp::Clear(Color::new(0.15, 0.15, 0.15, 1.0)),
                store_op: astrelis::gpu::types::StoreOp::Store,
            }],
            depth_stencil_attachment: None,
        });
    }

    renderer.begin(&camera);

    // === Left group: submitted in REVERSE z-order (red last) ===
    // If sorting works, blue (z=2) is on top regardless.
    let base_x = 100.0;
    let base_y = 100.0;

    // Submit blue FIRST (z=2, should appear on TOP).
    renderer.set_z_index(2);
    renderer.draw_rect_filled(
        Vec2::new(base_x + 60.0, base_y + 60.0),
        Vec2::new(120.0, 120.0),
        Color::new(0.2, 0.3, 0.9, 1.0),
    );

    // Submit green SECOND (z=1, should appear in MIDDLE).
    renderer.set_z_index(1);
    renderer.draw_rect_filled(
        Vec2::new(base_x + 30.0, base_y + 30.0),
        Vec2::new(120.0, 120.0),
        Color::new(0.2, 0.8, 0.3, 1.0),
    );

    // Submit red LAST (z=0, should appear at BOTTOM).
    renderer.set_z_index(0);
    renderer.draw_rect_filled(
        Vec2::new(base_x, base_y),
        Vec2::new(120.0, 120.0),
        Color::new(0.9, 0.2, 0.2, 1.0),
    );

    // === Right group: submitted in FORWARD z-order (for comparison) ===
    let base_x = 500.0;

    renderer.set_z_index(0);
    renderer.draw_rect_filled(
        Vec2::new(base_x, base_y),
        Vec2::new(120.0, 120.0),
        Color::new(0.9, 0.2, 0.2, 1.0),
    );

    renderer.set_z_index(1);
    renderer.draw_rect_filled(
        Vec2::new(base_x + 30.0, base_y + 30.0),
        Vec2::new(120.0, 120.0),
        Color::new(0.2, 0.8, 0.3, 1.0),
    );

    renderer.set_z_index(2);
    renderer.draw_rect_filled(
        Vec2::new(base_x + 60.0, base_y + 60.0),
        Vec2::new(120.0, 120.0),
        Color::new(0.2, 0.3, 0.9, 1.0),
    );

    // === Bottom: circle over rectangle ===
    let base_x = 300.0;
    let base_y = 400.0;

    renderer.set_z_index(0);
    renderer.draw_rect_filled(
        Vec2::new(base_x, base_y),
        Vec2::new(200.0, 150.0),
        Color::new(0.6, 0.3, 0.1, 1.0),
    );

    renderer.set_z_index(1);
    renderer.draw_circle_filled(
        Vec2::new(base_x + 180.0, base_y + 60.0),
        70.0,
        Color::new(0.1, 0.5, 0.7, 0.9),
    );

    // === Labels: draw small indicator rectangles ===
    // A tiny red/green/blue legend bar at top.
    renderer.set_z_index(10);
    renderer.draw_rect_filled(Vec2::new(100.0, 50.0), Vec2::new(15.0, 15.0), Color::RED);
    renderer.draw_rect_filled(Vec2::new(120.0, 50.0), Vec2::new(15.0, 15.0), Color::GREEN);
    renderer.draw_rect_filled(Vec2::new(140.0, 50.0), Vec2::new(15.0, 15.0), Color::BLUE);

    renderer.draw_rect_filled(Vec2::new(500.0, 50.0), Vec2::new(15.0, 15.0), Color::RED);
    renderer.draw_rect_filled(Vec2::new(520.0, 50.0), Vec2::new(15.0, 15.0), Color::GREEN);
    renderer.draw_rect_filled(Vec2::new(540.0, 50.0), Vec2::new(15.0, 15.0), Color::BLUE);

    renderer.end(&gpu, &mut encoder, frame.view(), &camera);

    gpu.submit(std::iter::once(encoder));
    frame.present();
}

fn main() {
    astrelis::core::logging::init_default();
    App::new()
        .add_default_plugins()
        .add_plugin(ZOrderPlugin)
        .run();
}
