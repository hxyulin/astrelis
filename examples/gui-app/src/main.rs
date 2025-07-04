use astrelis_framework::{
    config::{BenchmarkMode, Config}, egui, event::{Event, HandleStatus, KeyCode}, graphics::{
        egui::EguiContext, renderer::SimpleRenderer, Framebuffer, FramebufferOpts, GraphicsContextOpts, RenderableSurface, TextureFormat, TextureUsages
    }, input::InputSystem, math::{Vec2, Vec4}, profiling::profile_scope, run_app, App, AppHandler, EngineCtx, Extent3D, Window, WindowOpts
};

fn main() {
    let mut config = Config::default();
    config.benchmark = BenchmarkMode::WithWebsever;
    run_app::<GuiApp>(config);
}

struct GuiApp {
    window: Window,
    renderer: SimpleRenderer,
    egui: EguiContext,
    inputs: InputSystem,
    fb: Framebuffer,
}

impl App for GuiApp {
    fn init(ctx: EngineCtx) -> Box<dyn AppHandler> {
        let opts = WindowOpts::default();
        let window = ctx.create_window(opts, GraphicsContextOpts::default());
        let renderer = SimpleRenderer::new(&window);
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

        Box::new(Self {
            window,
            renderer,
            egui,
            inputs: InputSystem::new(),
            fb,
        })
    }
}

impl AppHandler for GuiApp {
    fn shutdown(&mut self, _ctx: EngineCtx) {
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

    fn update(&mut self, _ctx: EngineCtx) {
        let mut render_ctx = self.window.begin_render();

        let texture = self.egui.update_texture(&render_ctx, &self.fb);

        self.egui.ui(&render_ctx, |ctx| {
            profile_scope!("egui_update");
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("Test");
                if ui.button("Test Button").clicked() {
                    println!("button clicked!");
                }
                let size = egui::Vec2::new(400.0, 400.0);
                ui.image((texture, size));
            });
        });

        if self.inputs.is_key_pressed(&KeyCode::Space) {
            println!("Space pressed");
        }

        self.renderer
            .submit_quad(Vec2::new(0.0, 0.0), 0.0, Vec2::new(0.5, 0.5), Vec4::ONE);

        self.renderer.render(&mut render_ctx, RenderableSurface::Framebuffer(&self.fb));

        // egui needs to be updated using the 'ui' function before it can draw,
        // but this is not explicitly required by the type checker to allow for more
        // flexibility, if the UI is updated after the render, and it is rendered the next frame
        self.egui.render(&mut render_ctx);

        self.inputs.new_frame();
    }
}

impl Drop for GuiApp {
    fn drop(&mut self) {
        // You could also deinitialize here, but 'shutdown' is guarantted to be called
        log::info!("you can also shutdown here");
    }
}
