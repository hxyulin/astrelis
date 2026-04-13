//! App framework demo.
//!
//! Opens a window with cycling clear color, demonstrates:
//! - Plugin system with default plugins
//! - Custom plugin registration
//! - System phase ordering
//! - Time resource (delta, elapsed)
//! - Input polling
//! - GPU rendering via the framework
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-app --example app_demo
//! ```

use astrelis_app::{App, Phase, Plugin, Resources, Time};
use astrelis_core::color::Color;
use astrelis_gpu::command::{ColorAttachment, RenderPassDescriptor};
use astrelis_gpu::types::{LoadOp, StoreOp};
use astrelis_gpu::{Gpu, GpuError, Surface};
use astrelis_input::InputState;
use astrelis_window::event::WindowEvent;
use astrelis_window::keyboard::KeyCode;

/// Custom game state resource.
struct GameState {
    frame_count: u64,
    log_timer: f32,
}

/// Our game plugin.
struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameState {
            frame_count: 0,
            log_timer: 0.0,
        });
        app.add_event::<WindowEvent>();

        app.add_system(Phase::Update, update_system);
        app.add_system(Phase::Render, render_system);
    }
}

fn update_system(resources: &Resources) {
    let time = resources.get::<Time>();
    let input = resources.get::<InputState>();
    let mut state = resources.get_mut::<GameState>();

    state.log_timer += time.delta_secs();

    // Log stats every 2 seconds.
    if state.log_timer >= 2.0 {
        state.log_timer -= 2.0;
        tracing::info!(
            "Frame {} | dt={:.1}ms | elapsed={:.1}s",
            state.frame_count,
            time.delta_secs() * 1000.0,
            time.elapsed_secs(),
        );
    }

    if input.is_key_just_pressed(KeyCode::Escape) {
        tracing::info!("Escape pressed — exiting would happen via window close");
    }

    if input.is_key_just_pressed(KeyCode::Space) {
        tracing::info!("Space pressed at {:.2}s", time.elapsed_secs());
    }
}

fn render_system(resources: &Resources) {
    let gpu = resources.get::<Gpu>();
    let mut surface = resources.get_mut::<Surface>();
    let mut state = resources.get_mut::<GameState>();

    let frame = match surface.acquire() {
        Ok(f) => f,
        Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost | GpuError::Timeout) => return,
        Err(e) => panic!("failed to acquire surface texture: {e}"),
    };

    // Cycle colors.
    let t = state.frame_count as f32 / 120.0;
    let r = (t.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
    let g = ((t + 2.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
    let b = ((t + 4.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);

    let mut encoder = gpu.device().create_command_encoder(Some("frame"));
    {
        let _pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[ColorAttachment {
                view: frame.view(),
                resolve_target: None,
                load_op: LoadOp::Clear(Color::new(r, g, b, 1.0)),
                store_op: StoreOp::Store,
            }],
            depth_stencil_attachment: None,
        });
    }
    gpu.submit(std::iter::once(encoder));
    frame.present();
    state.frame_count += 1;
}

fn main() {
    astrelis_core::logging::init_default();
    App::new()
        .add_default_plugins()
        .add_plugin(GamePlugin)
        .run();
}
