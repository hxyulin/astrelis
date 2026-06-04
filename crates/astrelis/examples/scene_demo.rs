//! Scene tree demo: a spinning hub with orbiting arms and a nested
//! grandchild, rendered through user-written glue — the scene crate
//! itself knows nothing about rendering.
//!
//! Controls: Space toggles visibility of one arm (its children follow).
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis --example scene_demo
//! ```

use astrelis::prelude::*;

use astrelis::gpu::{Gpu, GpuError, Surface};

/// User-defined drawable component — note: defined HERE, not in the engine.
struct Shape {
    half_size: f32,
    color: Color,
}

/// Marks nodes that spin around their own origin.
struct Spin {
    speed: f32,
}

/// The arm whose visibility Space toggles.
struct ToggleTarget(NodeId);

struct ScenePopulatePlugin;

impl Plugin for ScenePopulatePlugin {
    fn build(&self, app: &mut App) {
        app.add_startup(|resources, _ctx| {
            let mut scene = resources.get_mut::<Scene>();

            // Hub at screen center; spins, dragging its subtree around.
            let hub = scene
                .spawn()
                .name("hub")
                .position(Vec3::new(640.0, 360.0, 0.0))
                .with(Shape { half_size: 30.0, color: Color::new(0.9, 0.3, 0.2, 1.0) })
                .with(Spin { speed: 0.8 })
                .id();

            // Four arms orbiting the hub via parent rotation.
            let mut toggle_arm = None;
            for i in 0..4 {
                let angle = std::f32::consts::FRAC_PI_2 * i as f32;
                let offset = Vec3::new(angle.cos() * 150.0, angle.sin() * 150.0, 0.0);
                let arm = scene
                    .spawn_child(hub)
                    .name(format!("arm{i}"))
                    .position(offset)
                    .with(Shape { half_size: 15.0, color: Color::new(0.2, 0.6, 0.9, 1.0) })
                    .with(Spin { speed: 3.0 })
                    .id();
                // One arm gets a grandchild to show two-level nesting.
                if i == 0 {
                    scene
                        .spawn_child(arm)
                        .name("tip")
                        .position(Vec3::new(50.0, 0.0, 0.0))
                        .with(Shape { half_size: 8.0, color: Color::new(0.3, 0.9, 0.4, 1.0) })
                        .id();
                    toggle_arm = Some(arm);
                }
            }
            let toggle = ToggleTarget(toggle_arm.expect("arm0 created"));
            drop(scene);
            resources.insert(toggle);
        });

        app.add_system(Phase::Update, update_scene);
        app.add_system(Phase::Render, render_scene);
    }
}

fn update_scene(resources: &Resources) {
    let mut scene = resources.get_mut::<Scene>();
    let time = resources.get::<Time>();
    let input = resources.get::<InputState>();

    // Mutating transforms while iterating a column would alias the
    // scene borrow, so collect the targets first (cheap: ids + f32s).
    let spinners: Vec<(NodeId, f32)> =
        scene.iter::<Spin>().map(|(id, s)| (id, s.speed)).collect();
    for (id, speed) in spinners {
        let mut t = *scene.local_transform(id).expect("spinner is live");
        t.set_rotation_2d(time.elapsed_secs() as f32 * speed);
        scene.set_transform(id, t);
    }

    if input.is_key_just_pressed(KeyCode::Space) {
        let target = resources.get::<ToggleTarget>().0;
        let visible = scene.visible(target).expect("toggle target is live");
        scene.set_visible(target, !visible);
    }
}

fn render_scene(resources: &Resources) {
    let gpu = resources.get::<Gpu>();
    let mut surface = resources.get_mut::<Surface>();
    let mut renderer = resources.get_mut::<Renderer2D>();
    let camera = resources.get::<Camera2D>();
    let scene = resources.get::<Scene>();

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
                load_op: astrelis::gpu::types::LoadOp::Clear(Color::new(0.08, 0.08, 0.12, 1.0)),
                store_op: astrelis::gpu::types::StoreOp::Store,
            }],
            depth_stencil_attachment: None,
        });
    }

    renderer.begin(&camera);

    // The glue: world transform from the scene, draw call to the renderer.
    for (id, shape) in scene.iter::<Shape>() {
        if scene.is_world_visible(id) != Some(true) {
            continue;
        }
        let world = scene.world_transform(id).expect("shape node is live");
        let pos = world.transform_point3(Vec3::ZERO);
        renderer.draw_rect_filled(
            Vec2::new(pos.x - shape.half_size, pos.y - shape.half_size),
            Vec2::splat(shape.half_size * 2.0),
            shape.color,
        );
    }

    renderer.end(&gpu, &mut encoder, frame.view(), &camera);

    gpu.submit(std::iter::once(encoder));
    frame.present();
}

/// 2D renderer setup, mirroring game_demo.
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

fn main() {
    astrelis::core::logging::init_default();
    App::new()
        .add_default_plugins()
        .add_plugin(ScenePlugin)
        .add_plugin(Render2DSetupPlugin)
        .add_plugin(ScenePopulatePlugin)
        .run();
}
