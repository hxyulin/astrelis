use astrelis_framework::{
    App, AppHandler, EngineCtx, Extent3D, Window, WindowOpts,
    config::{BenchmarkMode, Config},
    event::{Event, HandleStatus},
    graphics::{
        Framebuffer, FramebufferOpts, GraphicsContextOpts, TextureFormat, TextureUsages,
        renderer::SimpleRenderer,
    },
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

        Box::new(Self {
            window,
            renderer,
            inputs: InputSystem::new(),
            fb,
            scene: Registry::new(),
        })
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
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

        self.renderer.render(&mut render_ctx);

        self.inputs.new_frame();
    }
}

impl Drop for RoguerunApp {
    fn drop(&mut self) {
        // You could also deinitialize here, but 'shutdown' is guarantted to be called
        log::info!("you can also shutdown here");
    }
}
