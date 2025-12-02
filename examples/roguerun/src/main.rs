use astrelis_framework::{
    App, AppHandler, EngineCtx, Window, WindowOpts,
    config::{BenchmarkMode, Config},
    event::{Event, HandleStatus, KeyCode},
    graphics::{
        Color, GraphicsContextOpts, MatHandle, Material, MaterialComponent, RenderTarget,
        mesh::{Mesh, MeshComponent, MeshHandle, MeshSource, Vertex},
        renderer::SceneRenderer,
        shader::material_shader,
    },
    input::InputSystem,
    math::{Quat, Vec2, Vec3},
    run_app,
    text::{DEFAULT_FONT_NAME, Font, TextRenderer},
    world::{GlobalTransform, Scene, Transform},
};

fn main() {
    let mut config = Config::default();
    config.benchmark = BenchmarkMode::Off;
    run_app::<GameApp>(config);
}

struct Player {
    position: Vec3,
    speed: f32,
}

struct GameApp {
    window: Window,
    renderer: SceneRenderer,
    inputs: InputSystem,
    scene: Scene,

    player: Player,
    player_mesh: MeshHandle,
    player_material: MatHandle,

    ground_mesh: MeshHandle,
    ground_material: MatHandle,

    text_renderer: TextRenderer,
    frame_count: u64,
    last_fps: f32,
}

impl App for GameApp {
    fn init(mut ctx: EngineCtx) -> Box<dyn AppHandler> {
        let opts = WindowOpts {
            size: Some((1024.0, 768.0)),
            title: "Roguerun - Text Demo".to_string(),
            fullscreen: None,
        };
        let window = ctx.create_window(opts, GraphicsContextOpts::default());
        let renderer = SceneRenderer::new(&window);

        // Initialize text renderer
        let graphics = window.graphics();
        let mut text_renderer = TextRenderer::new(
            graphics.device(),
            graphics.queue(),
            graphics.surface_config().format,
        );

        // Register default font (Sarasa UI TC - comprehensive Unicode support)
        let mut default_font = Font::default();
        text_renderer.register_font(&mut default_font);

        // Create player mesh (cube)
        let player_mesh = ctx.engine_mut().meshes.create_mesh(Mesh::new(
            "Player".to_string(),
            MeshSource::Memory(
                vec![
                    Vertex {
                        pos: Vec3::new(-0.5, -0.5, 0.5),
                        texcoord: Vec2::ZERO,
                    },
                    Vertex {
                        pos: Vec3::new(0.5, -0.5, 0.5),
                        texcoord: Vec2::X,
                    },
                    Vertex {
                        pos: Vec3::new(0.5, 0.5, 0.5),
                        texcoord: Vec2::ONE,
                    },
                    Vertex {
                        pos: Vec3::new(-0.5, 0.5, 0.5),
                        texcoord: Vec2::Y,
                    },
                    Vertex {
                        pos: Vec3::new(-0.5, -0.5, -0.5),
                        texcoord: Vec2::ZERO,
                    },
                    Vertex {
                        pos: Vec3::new(-0.5, 0.5, -0.5),
                        texcoord: Vec2::Y,
                    },
                    Vertex {
                        pos: Vec3::new(0.5, 0.5, -0.5),
                        texcoord: Vec2::ONE,
                    },
                    Vertex {
                        pos: Vec3::new(0.5, -0.5, -0.5),
                        texcoord: Vec2::X,
                    },
                ],
                vec![
                    0, 1, 2, 0, 2, 3, // Front
                    4, 5, 6, 4, 6, 7, // Back
                    4, 0, 3, 4, 3, 5, // Left
                    1, 7, 6, 1, 6, 2, // Right
                    3, 2, 6, 3, 6, 5, // Top
                    4, 7, 1, 4, 1, 0, // Bottom
                ],
            ),
        ));

        // Create ground mesh (large plane)
        let ground_mesh = ctx.engine_mut().meshes.create_mesh(Mesh::new(
            "Ground".to_string(),
            MeshSource::Memory(
                vec![
                    Vertex {
                        pos: Vec3::new(-10.0, -1.0, -10.0),
                        texcoord: Vec2::ZERO,
                    },
                    Vertex {
                        pos: Vec3::new(10.0, -1.0, -10.0),
                        texcoord: Vec2::new(10.0, 0.0),
                    },
                    Vertex {
                        pos: Vec3::new(10.0, -1.0, 10.0),
                        texcoord: Vec2::new(10.0, 10.0),
                    },
                    Vertex {
                        pos: Vec3::new(-10.0, -1.0, 10.0),
                        texcoord: Vec2::new(0.0, 10.0),
                    },
                ],
                vec![0, 1, 2, 0, 2, 3],
            ),
        ));

        let shader = ctx.engine_mut().shaders.create_shader(material_shader());

        let player_material = ctx.engine_mut().mats.create_mat(Material {
            diffuse_color: Color::GREEN,
            shader,
        });

        let ground_material = ctx.engine_mut().mats.create_mat(Material {
            diffuse_color: Color::new(0.3, 0.3, 0.3, 1.0),
            shader,
        });

        let mut scene = Scene::new("Game Scene".to_string());

        // Spawn player entity
        let player_transform = Transform {
            position: Vec3::ZERO,
            scale: Vec3::ONE,
            rotation: Quat::IDENTITY,
        };
        scene.registry.spawn((
            player_transform,
            GlobalTransform::from_transform(&player_transform),
            MaterialComponent(player_material),
            MeshComponent(player_mesh),
        ));

        // Spawn ground entity
        let ground_transform = Transform {
            position: Vec3::ZERO,
            scale: Vec3::ONE,
            rotation: Quat::IDENTITY,
        };
        scene.registry.spawn((
            ground_transform,
            GlobalTransform::from_transform(&ground_transform),
            MaterialComponent(ground_material),
            MeshComponent(ground_mesh),
        ));

        Box::new(Self {
            window,
            renderer,
            inputs: InputSystem::new(),
            scene,
            player: Player {
                position: Vec3::ZERO,
                speed: 5.0,
            },
            player_mesh,
            player_material,
            ground_mesh,
            ground_material,
            text_renderer,
            frame_count: 0,
            last_fps: 0.0,
        })
    }
}

impl AppHandler for GameApp {
    fn shutdown(&mut self, _ctx: EngineCtx) {
        tracing::info!("Game shutting down");
    }

    fn on_event(&mut self, ctx: EngineCtx, event: &Event) -> HandleStatus {
        self.inputs.on_event(&event);

        match event {
            Event::CloseRequested => ctx.request_shutdown(),
            Event::WindowResized(new_size) => self.window.resized(*new_size),
            _ => {}
        }
        HandleStatus::ignored()
    }

    fn update(&mut self, mut ctx: EngineCtx) {
        self.frame_count += 1;

        // Fixed timestep (assume 60fps)
        let dt = 1.0 / 60.0;
        self.last_fps = 60.0;

        // Simple player movement
        let mut movement = Vec3::ZERO;

        if self.inputs.is_key_pressed(&KeyCode::KeyW) {
            movement.y += self.player.speed * dt;
        }
        if self.inputs.is_key_pressed(&KeyCode::KeyS) {
            movement.y -= self.player.speed * dt;
        }
        if self.inputs.is_key_pressed(&KeyCode::KeyA) {
            movement.x -= self.player.speed * dt;
        }
        if self.inputs.is_key_pressed(&KeyCode::KeyD) {
            movement.x += self.player.speed * dt;
        }

        self.player.position += movement;

        // Update player entity transform in scene
        // Note: In real game, would query for player entity and update
        for (_ent, transform, global_transform, mesh, _mat) in
            self.scene
                .registry
                .query_mut::<(Transform, GlobalTransform, MeshComponent, MaterialComponent)>()
        {
            // Simple check: if it's using player mesh, update position
            if mesh.0 == self.player_mesh {
                transform.position = self.player.position;
                *global_transform = GlobalTransform::from_transform(&transform);
            }
        }

        // Get window info before mutable borrow
        let window_size = self.window.size();
        let width = window_size.0 as f32;
        let height = window_size.1 as f32;

        // Update text renderer screen size for coordinate conversion
        self.text_renderer.set_screen_size(width, height);

        let mut render_ctx = self.window.begin_render();

        // Render 3D scene
        self.renderer.encode_scene(&self.scene.registry);
        self.renderer
            .render(ctx.engine_mut(), &mut render_ctx, RenderTarget::Window);

        // Render text overlay using screen pixel coordinates

        // Draw FPS counter (top-left)
        self.text_renderer.draw_text(
            &format!("FPS: {:.0}", self.last_fps),
            Vec2::new(10.0, 20.0),
            24.0,
            Color::GREEN,
            DEFAULT_FONT_NAME,
        );

        // Draw player position
        self.text_renderer.draw_text(
            &format!(
                "Position: ({:.1}, {:.1}, {:.1})",
                self.player.position.x, self.player.position.y, self.player.position.z
            ),
            Vec2::new(10.0, 50.0),
            20.0,
            Color::WHITE,
            DEFAULT_FONT_NAME,
        );

        // Draw controls (bottom-left)
        self.text_renderer.draw_text(
            "Controls: WASD to move",
            Vec2::new(10.0, height - 30.0),
            18.0,
            Color::new(0.8, 0.8, 0.8, 1.0),
            DEFAULT_FONT_NAME,
        );

        // Render text (get view from render context)
        let view = render_ctx.get_surface_view();
        self.text_renderer.render(
            render_ctx.window.graphics().device(),
            render_ctx.window.graphics().queue(),
            view,
        );

        self.inputs.new_frame();
    }
}

impl Drop for GameApp {
    fn drop(&mut self) {
        tracing::info!("Game app dropped");
    }
}
