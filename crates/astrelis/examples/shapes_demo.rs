//! Shapes showcase — visual proof that all shape primitives render correctly.
//!
//! Draws a grid of shapes: filled rects, outlined rects, filled circles,
//! and lines with varying sizes, colors, and thicknesses.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis --example shapes_demo
//! ```

use astrelis::prelude::*;
use astrelis::gpu::{Gpu, GpuError, Surface};

struct ShapesPlugin;

impl Plugin for ShapesPlugin {
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
    let time = resources.get::<Time>();

    let frame = match surface.acquire() {
        Ok(f) => f,
        Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost | GpuError::Timeout) => return,
        Err(e) => panic!("surface error: {e}"),
    };

    let mut encoder = gpu.device().create_command_encoder(Some("frame"));

    // Clear to dark background.
    {
        let _pass = encoder.begin_render_pass(&astrelis::gpu::command::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[astrelis::gpu::command::ColorAttachment {
                view: frame.view(),
                resolve_target: None,
                load_op: astrelis::gpu::types::LoadOp::Clear(Color::new(0.08, 0.08, 0.12, 1.0)),
                store_op: astrelis::gpu::types::StoreOp::Store,
            }],
            depth_stencil_attachment: None,
        });
    }

    renderer.begin(&camera);
    let t = time.elapsed_secs() as f32;

    // === Row 1: Filled rectangles ===
    let y = 80.0;
    renderer.draw_rect_filled(Vec2::new(50.0, y), Vec2::new(80.0, 60.0), Color::RED);
    renderer.draw_rect_filled(Vec2::new(180.0, y), Vec2::new(100.0, 100.0), Color::GREEN);
    renderer.draw_rect_filled(Vec2::new(330.0, y), Vec2::new(60.0, 120.0), Color::BLUE);
    // Semi-transparent.
    renderer.draw_rect_filled(
        Vec2::new(440.0, y),
        Vec2::new(120.0, 80.0),
        Color::new(1.0, 1.0, 0.0, 0.5),
    );

    // === Row 2: Outlined rectangles ===
    let y = 250.0;
    renderer.draw_rect(Vec2::new(50.0, y), Vec2::new(80.0, 60.0), Color::RED, 2.0);
    renderer.draw_rect(Vec2::new(180.0, y), Vec2::new(100.0, 100.0), Color::GREEN, 3.0);
    renderer.draw_rect(Vec2::new(330.0, y), Vec2::new(60.0, 120.0), Color::BLUE, 5.0);
    renderer.draw_rect(
        Vec2::new(440.0, y),
        Vec2::new(120.0, 80.0),
        Color::WHITE,
        1.0,
    );

    // === Row 3: Filled circles ===
    let y = 460.0;
    renderer.draw_circle_filled(Vec2::new(90.0, y), 40.0, Color::RED);
    renderer.draw_circle_filled(Vec2::new(220.0, y), 50.0, Color::GREEN);
    renderer.draw_circle_filled(Vec2::new(360.0, y), 30.0, Color::BLUE);
    // Animated pulsing circle.
    let pulse_r = 20.0 + (t * 3.0).sin().abs() * 30.0;
    renderer.draw_circle_filled(
        Vec2::new(500.0, y),
        pulse_r,
        Color::new(1.0, 0.5, 0.0, 0.8),
    );

    // === Row 4: Lines ===
    let y = 580.0;
    // Horizontal.
    renderer.draw_line(Vec2::new(50.0, y), Vec2::new(250.0, y), 2.0, Color::RED);
    // Diagonal.
    renderer.draw_line(
        Vec2::new(300.0, y - 30.0),
        Vec2::new(450.0, y + 30.0),
        3.0,
        Color::GREEN,
    );
    // Thick.
    renderer.draw_line(
        Vec2::new(500.0, y - 20.0),
        Vec2::new(650.0, y + 20.0),
        8.0,
        Color::BLUE,
    );

    // === Right side: Combined shapes ===
    let x = 750.0;
    // Filled rect with circle on top.
    renderer.set_z_index(0);
    renderer.draw_rect_filled(Vec2::new(x, 80.0), Vec2::new(200.0, 150.0), Color::new(0.2, 0.2, 0.3, 1.0));
    renderer.set_z_index(1);
    renderer.draw_circle_filled(Vec2::new(x + 100.0, 155.0), 40.0, Color::new(0.9, 0.3, 0.3, 1.0));

    // Concentric circles.
    renderer.set_z_index(0);
    renderer.draw_circle_filled(Vec2::new(x + 100.0, 380.0), 80.0, Color::new(0.15, 0.15, 0.3, 1.0));
    renderer.set_z_index(1);
    renderer.draw_circle_filled(Vec2::new(x + 100.0, 380.0), 55.0, Color::new(0.25, 0.25, 0.5, 1.0));
    renderer.set_z_index(2);
    renderer.draw_circle_filled(Vec2::new(x + 100.0, 380.0), 30.0, Color::new(0.4, 0.4, 0.8, 1.0));

    // Animated rotating line fan.
    renderer.set_z_index(0);
    let cx = x + 100.0;
    let cy = 580.0;
    for i in 0..8 {
        let angle = t * 0.5 + (i as f32) * std::f32::consts::TAU / 8.0;
        let len = 60.0;
        let end = Vec2::new(cx + angle.cos() * len, cy + angle.sin() * len);
        let hue = i as f32 / 8.0;
        renderer.draw_line(
            Vec2::new(cx, cy),
            end,
            2.0,
            Color::new(hue, 1.0 - hue, 0.5, 1.0),
        );
    }

    renderer.end(&gpu, &mut encoder, frame.view(), &camera);

    // Log batch stats once per second.
    let stats = renderer.stats();
    let frame_num = time.frame_count();
    if frame_num.is_multiple_of(60) {
        println!(
            "Render stats: instances={}, draw_calls={}, tex_switches={}",
            stats.instance_count, stats.draw_calls, stats.texture_switches
        );
    }

    gpu.submit(std::iter::once(encoder));
    frame.present();
}

fn main() {
    astrelis::core::logging::init_default();
    App::new()
        .add_default_plugins()
        .add_plugin(ShapesPlugin)
        .run();
}
