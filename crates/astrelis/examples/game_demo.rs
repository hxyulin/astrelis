//! End-to-end game demo using only `astrelis::prelude::*`.
//!
//! Demonstrates the full engine stack: app framework, 2D renderer,
//! input handling, and time tracking — all through the facade crate.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis --example game_demo
//! ```

use astrelis::prelude::*;

use astrelis::gpu::{Gpu, GpuError, Surface};

/// Game state: position of a square controlled by keyboard input.
struct GameState {
    x: f32,
    y: f32,
    speed: f32,
}

struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameState {
            x: 400.0,
            y: 300.0,
            speed: 200.0,
        });

        // Set up the 2D renderer after GPU is initialized.
        app.add_system(Phase::Update, update_game);
        app.add_system(Phase::Render, render_game);
    }
}

/// Set up the 2D renderer. Must be called as a startup function
/// because it needs the GPU to be initialized first.
struct Render2DSetupPlugin;

impl Plugin for Render2DSetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup(|resources, _ctx| {
            let gpu = resources.get::<Gpu>();
            let surface = resources.get::<Surface>();
            let format = surface.preferred_format();
            let renderer = Renderer2D::new(&gpu, format);
            let camera = Camera2D::new(1280, 720);

            drop(gpu);
            drop(surface);

            resources.insert(renderer);
            resources.insert(camera);
        });
    }
}

fn update_game(resources: &Resources) {
    let time = resources.get::<Time>();
    let input = resources.get::<InputState>();
    let mut state = resources.get_mut::<GameState>();

    let dt = time.delta_secs();
    let speed = state.speed;

    if input.is_key_pressed(KeyCode::ArrowLeft) || input.is_key_pressed(KeyCode::KeyA) {
        state.x -= speed * dt;
    }
    if input.is_key_pressed(KeyCode::ArrowRight) || input.is_key_pressed(KeyCode::KeyD) {
        state.x += speed * dt;
    }
    if input.is_key_pressed(KeyCode::ArrowUp) || input.is_key_pressed(KeyCode::KeyW) {
        state.y -= speed * dt;
    }
    if input.is_key_pressed(KeyCode::ArrowDown) || input.is_key_pressed(KeyCode::KeyS) {
        state.y += speed * dt;
    }
}

fn render_game(resources: &Resources) {
    let gpu = resources.get::<Gpu>();
    let mut surface = resources.get_mut::<Surface>();
    let mut renderer = resources.get_mut::<Renderer2D>();
    let camera = resources.get::<Camera2D>();
    let state = resources.get::<GameState>();
    let time = resources.get::<Time>();

    let frame = match surface.acquire() {
        Ok(f) => f,
        Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost | GpuError::Timeout) => return,
        Err(e) => panic!("failed to acquire surface: {e}"),
    };

    let mut encoder = gpu.device().create_command_encoder(Some("frame"));

    // Clear pass.
    {
        let _pass = encoder.begin_render_pass(&astrelis::gpu::command::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[astrelis::gpu::command::ColorAttachment {
                view: frame.view(),
                resolve_target: None,
                load_op: astrelis::gpu::types::LoadOp::Clear(Color::new(0.1, 0.1, 0.15, 1.0)),
                store_op: astrelis::gpu::types::StoreOp::Store,
            }],
            depth_stencil_attachment: None,
        });
    }

    // 2D drawing.
    renderer.begin(&camera);

    // Background decorations.
    renderer.set_z_index(0);
    renderer.draw_rect_filled(Vec2::new(50.0, 50.0), Vec2::new(200.0, 150.0), Color::new(0.2, 0.3, 0.4, 1.0));
    renderer.draw_circle_filled(Vec2::new(900.0, 200.0), 60.0, Color::new(0.4, 0.2, 0.5, 1.0));

    // Animated circle.
    let pulse = (time.elapsed_secs() as f32 * 2.0).sin() * 0.5 + 0.5;
    renderer.draw_circle_filled(
        Vec2::new(640.0, 500.0),
        30.0 + pulse * 20.0,
        Color::new(0.2, 0.8, 0.3, 0.7),
    );

    // Player square (on top).
    renderer.set_z_index(1);
    renderer.draw_rect_filled(
        Vec2::new(state.x - 25.0, state.y - 25.0),
        Vec2::new(50.0, 50.0),
        Color::new(0.9, 0.3, 0.2, 1.0),
    );

    // Lines.
    renderer.set_z_index(0);
    renderer.draw_line(Vec2::new(100.0, 400.0), Vec2::new(500.0, 450.0), 3.0, Color::new(0.8, 0.8, 0.2, 1.0));

    renderer.end(&gpu, &mut encoder, frame.view(), &camera);

    gpu.submit(std::iter::once(encoder));
    frame.present();
}

fn main() {
    astrelis::core::logging::init_default();
    App::new()
        .add_default_plugins()
        .add_plugin(Render2DSetupPlugin)
        .add_plugin(GamePlugin)
        .run();
}
