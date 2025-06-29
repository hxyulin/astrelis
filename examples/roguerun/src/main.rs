use astrelis_framework::{
    App, AppHandler, EngineCtx, Extent3D, Window, WindowOpts,
    config::{BenchmarkMode, Config},
    event::{Event, HandleStatus},
    graphics::{Framebuffer, GraphicsContextOpts, TextureUsages, renderer::SimpleRenderer},
    input::InputSystem,
    math::{Vec2, Vec4},
    run_app,
    world::{Component, Registry},
};

fn main() {
    let mut config = Config::default();
    config.benchmark = BenchmarkMode::WithWebsever;
    run_app::<RoguerunApp>(config);
}

struct RoguerunApp {
    window: Window,
    renderer: SimpleRenderer,
    inputs: InputSystem,
    fb: Framebuffer,
    scene: Registry,
}

impl App for RoguerunApp {
    fn init(ctx: EngineCtx) -> Box<dyn AppHandler> {
        let opts = WindowOpts::default();
        let window = ctx.create_window(opts, GraphicsContextOpts::default());
        let renderer = SimpleRenderer::new(&window);
        let fb = Framebuffer::new(
            &window,
            Extent3D {
                width: 400,
                height: 400,
                depth: 1,
            },
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );

        Box::new(Self {
            window,
            renderer,
            inputs: InputSystem::new(),
            fb,
            scene: Registry::new(),
        })
    }
}

#[derive(Debug, Default)]
pub struct Transform {
    translation: Vec2,
    rotation: f32,
    scale: Vec2,
}

impl Component for Transform {}

impl AppHandler for RoguerunApp {
    fn on_event(&mut self, ctx: EngineCtx, event: &Event) -> HandleStatus {
        self.inputs.on_event(&event);

        match event {
            Event::CloseRequested => ctx.request_shutdown(),
            Event::WindowResized(new_size) => self.window.resized(*new_size),
            _ => {}
        }
        HandleStatus::ignored()
    }

    fn update(&mut self, _ctx: EngineCtx) {
        let mut render_ctx = self.window.begin_render();

        self.renderer
            .submit_quad(Vec2::new(0.0, 0.0), 0.0, Vec2::new(0.5, 0.5), Vec4::ONE);

        self.renderer.render(&mut render_ctx, Some(&self.fb));

        self.inputs.new_frame();
    }
}

impl Drop for RoguerunApp {
    fn drop(&mut self) {
        // You could also deinitialize here, but 'shutdown' is guarantted to be called
        log::info!("you can also shutdown here");
    }
}
