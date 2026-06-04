//! Unlit 3D demo: a spinning rainbow cube with two orbiting spheres,
//! a ground grid and origin axes, driven by the scene tree. The glue
//! component (`MeshInstance`) is defined HERE, not in the engine.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis --example render_3d_demo
//! ```

use astrelis::prelude::*;

use astrelis::core::geometry::{Physical, Size};
use astrelis::gpu::{Gpu, GpuError, Surface};
use astrelis::render_3d::primitives;

/// User-defined drawable component — engine knows nothing about it.
struct MeshInstance {
    mesh: MeshHandle,
    tint: Color,
}

/// Marks nodes that spin around their +Y axis.
struct Spin {
    speed: f32,
}

/// Handles to the demo's uploaded meshes.
struct DemoMeshes {
    cube: MeshHandle,
    sphere: MeshHandle,
}

/// Creates the renderer and camera, and uploads the demo meshes.
struct Render3DSetupPlugin;

impl Plugin for Render3DSetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup(|resources, _ctx| {
            let gpu = resources.get::<Gpu>();
            let surface = resources.get::<Surface>();
            let format = surface.preferred_format();
            let mut renderer = Renderer3D::new(&gpu, format);
            let camera = Camera3D::new(1280.0 / 720.0);

            // Rainbow cube: paint vertex colors from positions.
            let mut cube_data = primitives::cube(2.0);
            for v in &mut cube_data.vertices {
                let p = Vec3::from(v.position).normalize() * 0.5 + Vec3::splat(0.5);
                v.color = [p.x, p.y, p.z, 1.0];
            }
            let cube = renderer.create_mesh(&gpu, &cube_data);
            let sphere = renderer.create_mesh(&gpu, &primitives::uv_sphere(0.5, 24, 12));

            drop(gpu);
            drop(surface);

            resources.insert(renderer);
            resources.insert(camera);
            resources.insert(DemoMeshes { cube, sphere });
        });
    }
}

/// Builds the scene: spinning cube hub with two orbiting spheres.
struct ScenePopulatePlugin;

impl Plugin for ScenePopulatePlugin {
    fn build(&self, app: &mut App) {
        app.add_startup(|resources, _ctx| {
            let meshes = resources.get::<DemoMeshes>();
            let cube = meshes.cube;
            let sphere = meshes.sphere;
            drop(meshes);

            let mut scene = resources.get_mut::<Scene>();

            // Hub cube above the grid; its spin drags the spheres around.
            let hub = scene
                .spawn()
                .name("hub")
                .position(Vec3::new(0.0, 1.0, 0.0))
                .with(MeshInstance { mesh: cube, tint: Color::new(1.0, 1.0, 1.0, 1.0) })
                .with(Spin { speed: 0.6 })
                .id();

            for i in 0..2 {
                let angle = std::f32::consts::PI * i as f32;
                scene
                    .spawn_child(hub)
                    .name(format!("orbiter{i}"))
                    .position(Vec3::new(angle.cos() * 3.0, 0.0, angle.sin() * 3.0))
                    .with(MeshInstance {
                        mesh: sphere,
                        tint: if i == 0 {
                            Color::new(0.3, 0.7, 1.0, 1.0)
                        } else {
                            Color::new(1.0, 0.6, 0.2, 1.0)
                        },
                    })
                    .id();
            }
        });

        app.add_system(Phase::Update, update_scene);
        app.add_system(Phase::Render, render_scene);
    }
}

fn update_scene(resources: &Resources) {
    let time = resources.get::<Time>();
    let t = time.elapsed_secs() as f32;

    {
        let mut scene = resources.get_mut::<Scene>();
        // Collect-first: mutating while iterating would alias the borrow.
        let spinners: Vec<(NodeId, f32)> =
            scene.iter::<Spin>().map(|(id, s)| (id, s.speed)).collect();
        for (id, speed) in spinners {
            scene.set_rotation(id, Quat::from_rotation_y(t * speed));
        }
    }

    // Slow orbit camera around the scene.
    let mut camera = resources.get_mut::<Camera3D>();
    let a = t * 0.25;
    camera.position = Vec3::new(a.sin() * 9.0, 4.0, a.cos() * 9.0);
    camera.look_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
}

fn render_scene(resources: &Resources) {
    let gpu = resources.get::<Gpu>();
    let mut surface = resources.get_mut::<Surface>();
    let mut renderer = resources.get_mut::<Renderer3D>();
    let mut camera = resources.get_mut::<Camera3D>();
    let scene = resources.get::<Scene>();

    let frame = match surface.acquire() {
        Ok(f) => f,
        Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost | GpuError::Timeout) => return,
        Err(e) => panic!("failed to acquire surface: {e}"),
    };

    // Physical size of the swapchain — drives depth-buffer sizing and
    // the camera aspect.
    let config = surface.config().expect("surface is configured");
    let size = Size::<Physical>::new(config.width as f32, config.height as f32);
    camera.aspect = size.width / size.height;

    let mut encoder = gpu.device().create_command_encoder(Some("frame"));

    // Clear pass (color only; the 3D pass owns and clears its depth).
    {
        let _pass = encoder.begin_render_pass(&astrelis::gpu::command::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[astrelis::gpu::command::ColorAttachment {
                view: frame.view(),
                resolve_target: None,
                load_op: astrelis::gpu::types::LoadOp::Clear(Color::new(0.05, 0.06, 0.09, 1.0)),
                store_op: astrelis::gpu::types::StoreOp::Store,
            }],
            depth_stencil_attachment: None,
        });
    }

    renderer.begin(&camera);

    // The glue: world transforms from the scene, draws to the renderer.
    for (id, inst) in scene.iter::<MeshInstance>() {
        if scene.is_world_visible(id) != Some(true) {
            continue;
        }
        let world = scene.world_transform(id).expect("mesh node is live");
        renderer.draw_mesh(inst.mesh, world, inst.tint);
    }

    renderer.draw_grid(10.0, 1.0, Color::new(0.25, 0.27, 0.32, 1.0));
    renderer.draw_axes(Mat4::IDENTITY, 1.5);

    renderer.end(&gpu, &mut encoder, frame.view(), size);

    gpu.submit(std::iter::once(encoder));
    frame.present();
}

fn main() {
    astrelis::core::logging::init_default();
    App::new()
        .add_default_plugins()
        .add_plugin(ScenePlugin)
        .add_plugin(Render3DSetupPlugin)
        .add_plugin(ScenePopulatePlugin)
        .run();
}
