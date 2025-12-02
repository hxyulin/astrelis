use astrelis_framework::{
    App, AppHandler, EngineCtx, Extent3D, Window, WindowOpts,
    config::{BenchmarkMode, Config},
    egui,
    event::{Event, HandleStatus},
    graphics::{
        Color, FramebufferOpts, GraphicsContextOpts, MatHandle, Material, MaterialComponent,
        RenderTarget, RenderTargetId, TextureUsages,
        egui::EguiContext,
        mesh::{Mesh, MeshComponent, MeshHandle, MeshSource, Vertex},
        renderer::SceneRenderer,
        shader::material_shader,
    },
    input::InputSystem,
    math::{Quat, Vec2, Vec3},
    profiling::profile_scope,
    run_app,
    world::{GlobalTransform, Scene, Transform},
};

fn main() {
    let mut config = Config::default();
    config.benchmark = BenchmarkMode::Off;
    run_app::<SceneEditorApp>(config);
}

struct SceneEditorApp {
    window: Window,
    renderer: SceneRenderer,
    egui: EguiContext,
    inputs: InputSystem,
    viewport_fb: RenderTargetId,

    scene: Scene,
    material: MatHandle,
    cube_mesh: MeshHandle,

    viewport_size: [f32; 2],
    object_count: usize,
}

impl App for SceneEditorApp {
    fn init(mut ctx: EngineCtx) -> Box<dyn AppHandler> {
        let opts = WindowOpts {
            size: Some((1280.0, 720.0)),
            title: "Scene Editor - Embedded Viewport".to_string(),
            fullscreen: None,
        };
        let mut window = ctx.create_window(opts, GraphicsContextOpts::default());
        let renderer = SceneRenderer::new(&window);
        let egui = EguiContext::new(&window);

        let viewport_size = [800.0, 600.0];
        let viewport_fb = window.create_framebuffer(FramebufferOpts {
            extent: Extent3D {
                width: viewport_size[0] as u32,
                height: viewport_size[1] as u32,
                depth: 1,
            },
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            depth: true,
            format: None,
            sample_count: 1,
        });

        let cube_mesh = ctx.engine_mut().meshes.create_mesh(Mesh::new(
            "Cube".to_string(),
            MeshSource::Memory(
                vec![
                    // Front face
                    Vertex {
                        pos: Vec3::new(-0.5, -0.5, 0.5),
                        texcoord: Vec2::new(0.0, 0.0),
                    },
                    Vertex {
                        pos: Vec3::new(0.5, -0.5, 0.5),
                        texcoord: Vec2::new(1.0, 0.0),
                    },
                    Vertex {
                        pos: Vec3::new(0.5, 0.5, 0.5),
                        texcoord: Vec2::new(1.0, 1.0),
                    },
                    Vertex {
                        pos: Vec3::new(-0.5, 0.5, 0.5),
                        texcoord: Vec2::new(0.0, 1.0),
                    },
                    // Back face
                    Vertex {
                        pos: Vec3::new(-0.5, -0.5, -0.5),
                        texcoord: Vec2::new(1.0, 0.0),
                    },
                    Vertex {
                        pos: Vec3::new(-0.5, 0.5, -0.5),
                        texcoord: Vec2::new(1.0, 1.0),
                    },
                    Vertex {
                        pos: Vec3::new(0.5, 0.5, -0.5),
                        texcoord: Vec2::new(0.0, 1.0),
                    },
                    Vertex {
                        pos: Vec3::new(0.5, -0.5, -0.5),
                        texcoord: Vec2::new(0.0, 0.0),
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

        let shader = ctx.engine_mut().shaders.create_shader(material_shader());
        let material = ctx.engine_mut().mats.create_mat(Material {
            diffuse_color: Color::BLUE,
            shader,
        });

        Box::new(Self {
            window,
            renderer,
            egui,
            inputs: InputSystem::new(),
            viewport_fb,
            scene: Scene::new("Main Scene".to_string()),
            cube_mesh,
            material,
            viewport_size,
            object_count: 0,
        })
    }
}

impl AppHandler for SceneEditorApp {
    fn shutdown(&mut self, _ctx: EngineCtx) {
        tracing::info!("Scene editor shutting down...");
    }

    fn on_event(&mut self, ctx: EngineCtx, event: &Event) -> HandleStatus {
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

        let viewport_texture = self.egui.update_texture(&render_ctx, self.viewport_fb);

        self.egui.ui(&render_ctx, |egui_ctx| {
            profile_scope!("egui_ui");

            // Left panel - Scene hierarchy
            egui::SidePanel::left("hierarchy_panel")
                .default_width(200.0)
                .show(egui_ctx, |ui| {
                    ui.heading("Scene Hierarchy");
                    ui.separator();
                    ui.label(format!("Objects: {}", self.object_count));
                    ui.separator();
                    if ui.button("Add Cube").clicked() {
                        let x = (rand::random::<f32>() - 0.5) * 4.0;
                        let y = (rand::random::<f32>() - 0.5) * 4.0;
                        let transform = Transform {
                            position: Vec3::new(x, y, 0.0),
                            scale: Vec3::ONE,
                            rotation: Quat::IDENTITY,
                        };
                        self.scene.registry.spawn((
                            transform,
                            GlobalTransform::from_transform(&transform),
                            MaterialComponent(self.material),
                            MeshComponent(self.cube_mesh),
                        ));
                        self.object_count += 1;
                    }
                    if ui.button("Clear Scene").clicked() {
                        self.scene.registry.clear();
                        self.object_count = 0;
                    }
                });

            // Right panel - Properties
            egui::SidePanel::right("properties_panel")
                .default_width(250.0)
                .show(egui_ctx, |ui| {
                    ui.heading("Properties");
                    ui.separator();
                    ui.label("Viewport Settings");
                    ui.add(
                        egui::Slider::new(&mut self.viewport_size[0], 400.0..=1920.0).text("Width"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.viewport_size[1], 300.0..=1080.0)
                            .text("Height"),
                    );
                });

            // Central panel - 3D Viewport
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                ui.heading("3D Viewport");
                ui.separator();

                let size = egui::Vec2::new(self.viewport_size[0], self.viewport_size[1]);
                ui.image((viewport_texture, size));

                ui.separator();
                ui.label("Embedded framebuffer rendered from engine core");
            });
        });

        self.renderer.encode_scene(&self.scene.registry);
        self.renderer.render(
            ctx.engine_mut(),
            &mut render_ctx,
            RenderTarget::Target(self.viewport_fb),
        );

        self.egui.render(&mut render_ctx);
        self.inputs.new_frame();
    }
}

impl Drop for SceneEditorApp {
    fn drop(&mut self) {
        tracing::info!("Scene editor dropped");
    }
}
