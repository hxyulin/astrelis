//! Batching stress test — draws many shapes and reports draw call stats.
//!
//! Proves that the batch renderer merges consecutive same-texture draws
//! into single draw calls. Logs instance count vs draw call count each
//! second so you can verify batching is working.
//!
//! Expected: hundreds of instances but very few draw calls (ideally 1
//! for all shapes since they share the white pixel texture).
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis --example batching_demo
//! ```

use astrelis::prelude::*;
use astrelis::gpu::{Gpu, GpuError, Surface};

struct LogTimer(f32);

struct BatchingPlugin;

impl Plugin for BatchingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LogTimer(0.0));

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
    let mut log_timer = resources.get_mut::<LogTimer>();

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
                load_op: astrelis::gpu::types::LoadOp::Clear(Color::new(0.05, 0.05, 0.08, 1.0)),
                store_op: astrelis::gpu::types::StoreOp::Store,
            }],
            depth_stencil_attachment: None,
        });
    }

    renderer.begin(&camera);

    let t = time.elapsed_secs() as f32;

    // Grid of filled rectangles (all same z, same texture = 1 batch).
    let cols = 20;
    let rows = 10;
    let cell_w = 50.0_f32;
    let cell_h = 50.0_f32;
    let padding = 5.0_f32;

    for row in 0..rows {
        for col in 0..cols {
            let x = 40.0 + col as f32 * (cell_w + padding);
            let y = 40.0 + row as f32 * (cell_h + padding);

            // Color varies by position.
            let r = col as f32 / cols as f32;
            let g = row as f32 / rows as f32;
            let b = ((t + r * 3.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);

            renderer.draw_rect_filled(
                Vec2::new(x, y),
                Vec2::new(cell_w, cell_h),
                Color::new(r, g, b, 1.0),
            );
        }
    }

    // Scatter some circles at z=1 (different z = forces new batch boundary
    // only if interleaved with z=0 items, but since these are all z=1
    // they should batch together too).
    renderer.set_z_index(1);
    for i in 0..30 {
        let angle = i as f32 * 0.21 + t * 0.5;
        let cx = 640.0 + angle.cos() * (200.0 + i as f32 * 5.0);
        let cy = 360.0 + angle.sin() * (100.0 + i as f32 * 3.0);
        let radius = 8.0 + (t * 2.0 + i as f32).sin().abs() * 12.0;
        renderer.draw_circle_filled(
            Vec2::new(cx, cy),
            radius,
            Color::new(1.0, 0.8, 0.2, 0.7),
        );
    }

    renderer.end(&gpu, &mut encoder, frame.view(), &camera);

    // Log stats.
    let stats = renderer.stats();
    log_timer.0 += time.delta_secs();
    if log_timer.0 >= 1.0 {
        log_timer.0 -= 1.0;
        let reduction = if stats.draw_calls > 0 {
            stats.instance_count as f32 / stats.draw_calls as f32
        } else {
            0.0
        };
        println!(
            "Batch stats: {} instances in {} draw calls ({:.0}x reduction, {} tex switches)",
            stats.instance_count, stats.draw_calls, reduction, stats.texture_switches
        );
    }

    gpu.submit(std::iter::once(encoder));
    frame.present();
}

fn main() {
    astrelis::core::logging::init_default();
    App::new()
        .add_default_plugins()
        .add_plugin(BatchingPlugin)
        .run();
}
