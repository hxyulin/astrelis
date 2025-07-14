use astrelis_framework::{
    App, AppHandler, EngineCtx, Extent3D, Window, WindowOpts,
    config::{BenchmarkMode, Config},
    egui,
    event::{Event, HandleStatus, KeyCode},
    graphics::{
        Color, Framebuffer, FramebufferOpts, GraphicsContextOpts, MatHandle, Material,
        MaterialComponent, RenderableSurface, TextureUsages,
        egui::EguiContext,
        mesh::{Mesh, MeshComponent, MeshHandle, MeshSource, Vertex},
        renderer::SceneRenderer,
        shader::material_shader,
    },
    input::InputSystem,
    math::{Quat, Vec2, Vec3, Vec4},
    profiling::profile_scope,
    run_app,
    world::{GlobalTransform, Scene, Transform},
};

fn main() {
    let mut config = Config::default();
    config.benchmark = BenchmarkMode::WithWebsever;
    run_app::<GuiApp>(config);
}

struct GuiApp {
    window: Window,
    renderer: SceneRenderer,
    egui: EguiContext,
    inputs: InputSystem,
    fb: Framebuffer,

    scene: Scene,
    material: MatHandle,
    mesh: MeshHandle,
}

impl App for GuiApp {
    fn init(mut ctx: EngineCtx) -> Box<dyn AppHandler> {
        let opts = WindowOpts::default();
        let window = ctx.create_window(opts, GraphicsContextOpts::default());
        let renderer = SceneRenderer::new(&window);
        let egui = EguiContext::new(&window);
        let fb = Framebuffer::new(
            &window,
            FramebufferOpts {
                extent: Extent3D {
                    width: 400,
                    height: 400,
                    depth: 1,
                },
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                depth: true,
                format: None,
                sample_count: 1,
            },
        );

        let mesh = ctx.engine_mut().meshes.create_mesh(Mesh::new(
            "Square".to_string(),
            MeshSource::Memory(
                vec![
                    Vertex {
                        pos: Vec3::new(-0.5, -0.5, 0.0),
                        texcoord: Vec2::new(0.0, 0.0),
                    },
                    Vertex {
                        pos: Vec3::new(0.5, -0.5, 0.0),
                        texcoord: Vec2::new(1.0, 0.0),
                    },
                    Vertex {
                        pos: Vec3::new(0.5, 0.5, 0.0),
                        texcoord: Vec2::new(1.0, 1.0),
                    },
                    Vertex {
                        pos: Vec3::new(-0.5, 0.5, 0.0),
                        texcoord: Vec2::new(0.0, 1.0),
                    },
                ],
                vec![
                    0, 1, 2, // Triangle 1: bottom right
                    0, 2, 3, // Triangle 2: top left
                ],
            ),
        ));

        let shader = ctx.engine_mut().shaders.create_shader(material_shader());

        let material = ctx.engine_mut().mats.create_mat(Material {
            diffuse_color: Color::WHITE,
            shader,
        });

        Box::new(Self {
            window,
            renderer,
            egui,
            inputs: InputSystem::new(),
            fb,

            scene: Scene::new("Default Scene".to_string()),
            mesh,
            material,
        })
    }
}

impl AppHandler for GuiApp {
    fn shutdown(&mut self, _ctx: EngineCtx) {
        // Here this will be called when gracefully shutdown
        log::info!("saving work...");
    }

    fn on_event(&mut self, ctx: EngineCtx, event: &Event) -> HandleStatus {
        // We handle egui events before our own events
        if self.egui.on_event(&self.window, event).consumed {
            return HandleStatus::consumed();
        }

        self.inputs.on_event(&event);

        match event {
            Event::CloseRequested => ctx.request_shutdown(),
            Event::WindowResized(new_size) => self.window.resized(*new_size),
            _ => {}
        }
        HandleStatus::ignored()
    }

    fn update(&mut self, mut ctx: EngineCtx) {
        let mut render_ctx = self.window.begin_render();

        let texture = self.egui.update_texture(&render_ctx, &self.fb);

        self.egui.ui(&render_ctx, |ctx| {
            profile_scope!("egui_update");
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("Test");
                if ui.button("Add Object").clicked() {
                    let transform = Transform {
                        position: Vec3::ZERO,
                        scale: Vec3::ONE,
                        rotation: Quat::IDENTITY,
                    };
                    self.scene.registry.spawn((
                        transform,
                        GlobalTransform::from_transform(&transform),
                        MaterialComponent(self.material),
                        MeshComponent(self.mesh),
                    ));
                }
                let size = egui::Vec2::new(400.0, 400.0);
                ui.image((texture, size));
            });
        });

        if self.inputs.is_key_pressed(&KeyCode::Space) {
            println!("Space pressed");
        }

        self.renderer.encode_scene(&self.scene.registry);

        self.renderer.render(
            ctx.engine_mut(),
            &mut render_ctx,
            RenderableSurface::Framebuffer(&self.fb),
        );

        // egui needs to be updated using the 'ui' function before it can draw,
        // but this is not explicitly required by the type checker to allow for more
        // flexibility, if the UI is updated after the render, and it is rendered the next frame
        self.egui.render(&mut render_ctx);

        self.inputs.new_frame();
    }
}

impl Drop for GuiApp {
    fn drop(&mut self) {
        // You could also deinitialize here, but 'shutdown' is guarantted to be called if the
        // program does not panic
        log::info!("you can also shutdown here");
        // This is called even if the program panics (unwinding)
    }
}
