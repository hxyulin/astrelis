use astrelis_framework::{
    App, AppHandler, EngineCtx, Window, WindowOpts,
    config::{BenchmarkMode, Config},
    egui,
    event::{Event, HandleStatus},
    graphics::{GraphicsContextOpts, RenderTarget, egui::EguiContext, renderer::SimpleRenderer},
    input::InputSystem,
    math::{Vec2, Vec4},
    run_app,
};

fn main() {
    let mut config = Config::default();
    config.benchmark = BenchmarkMode::Off;
    run_app::<EguiDemoApp>(config);
}

struct EguiDemoApp {
    window: Window,
    renderer: SimpleRenderer,
    egui: EguiContext,
    inputs: InputSystem,

    counter: i32,
    text_input: String,
    slider_value: f32,
    show_debug_panel: bool,
    show_settings_panel: bool,
}

impl App for EguiDemoApp {
    fn init(ctx: EngineCtx) -> Box<dyn AppHandler> {
        let opts = WindowOpts {
            size: Some((1280.0, 720.0)),
            title: "Egui Demo - Floating Panels".to_string(),
            fullscreen: None,
        };
        let window = ctx.create_window(opts, GraphicsContextOpts::default());
        let renderer = SimpleRenderer::new(&window);
        let egui = EguiContext::new(&window);

        Box::new(Self {
            window,
            renderer,
            egui,
            inputs: InputSystem::new(),
            counter: 0,
            text_input: String::new(),
            slider_value: 0.5,
            show_debug_panel: true,
            show_settings_panel: true,
        })
    }
}

impl AppHandler for EguiDemoApp {
    fn shutdown(&mut self, _ctx: EngineCtx) {
        tracing::info!("Egui demo shutting down");
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

    fn update(&mut self, _ctx: EngineCtx) {
        let mut render_ctx = self.window.begin_render();

        // Render some colored quads in the background
        for i in 0..5 {
            for j in 0..5 {
                let x = (i as f32 - 2.0) * 0.3;
                let y = (j as f32 - 2.0) * 0.3;
                let r = (i as f32) / 5.0;
                let g = (j as f32) / 5.0;
                let b = 0.5;
                self.renderer.submit_quad(
                    Vec2::new(x, y),
                    0.0,
                    Vec2::new(0.25, 0.25),
                    Vec4::new(r, g, b, 1.0),
                );
            }
        }

        self.renderer.render(&mut render_ctx, RenderTarget::Window);

        // Draw Egui UI on top
        self.egui.ui(&render_ctx, |ctx| {
            // Top menu bar
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            std::process::exit(0);
                        }
                    });
                    ui.menu_button("View", |ui| {
                        ui.checkbox(&mut self.show_debug_panel, "Debug Panel");
                        ui.checkbox(&mut self.show_settings_panel, "Settings Panel");
                    });
                    ui.menu_button("Help", |ui| {
                        ui.label("Egui Demo Application");
                        ui.label("Built with Astrelis Engine");
                    });
                });
            });

            // Floating debug panel
            if self.show_debug_panel {
                egui::Window::new("Debug Info")
                    .default_pos([20.0, 60.0])
                    .default_size([300.0, 200.0])
                    .resizable(true)
                    .show(ctx, |ui| {
                        ui.heading("Debug Information");
                        ui.separator();
                        ui.label(format!("Counter: {}", self.counter));
                        ui.label(format!("Slider Value: {:.2}", self.slider_value));
                        ui.separator();
                        if ui.button("Increment Counter").clicked() {
                            self.counter += 1;
                        }
                        if ui.button("Reset Counter").clicked() {
                            self.counter = 0;
                        }
                        ui.separator();
                        ui.label("FPS: ~60");
                    });
            }

            // Floating settings panel
            if self.show_settings_panel {
                egui::Window::new("Settings")
                    .default_pos([340.0, 60.0])
                    .default_size([350.0, 300.0])
                    .resizable(true)
                    .show(ctx, |ui| {
                        ui.heading("Application Settings");
                        ui.separator();

                        ui.label("Text Input:");
                        ui.text_edit_singleline(&mut self.text_input);

                        ui.add_space(10.0);

                        ui.label("Slider Control:");
                        ui.add(egui::Slider::new(&mut self.slider_value, 0.0..=1.0));

                        ui.add_space(10.0);

                        ui.collapsing("Advanced Options", |ui| {
                            ui.label("Future settings go here");
                            ui.checkbox(&mut true, "Option 1");
                            ui.checkbox(&mut false, "Option 2");
                        });

                        ui.separator();
                        if ui.button("Apply Settings").clicked() {
                            tracing::info!("Settings applied: {}", self.text_input);
                        }
                    });
            }

            // Floating tool panel
            egui::Window::new("Tools")
                .default_pos([710.0, 60.0])
                .default_size([250.0, 400.0])
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading("Tool Panel");
                    ui.separator();

                    if ui.button("Tool 1").clicked() {
                        tracing::info!("Tool 1 activated");
                    }
                    if ui.button("Tool 2").clicked() {
                        tracing::info!("Tool 2 activated");
                    }
                    if ui.button("Tool 3").clicked() {
                        tracing::info!("Tool 3 activated");
                    }

                    ui.separator();
                    ui.label("Color Picker:");
                    let mut color = [self.slider_value, 0.5, 0.8];
                    ui.color_edit_button_rgb(&mut color);

                    ui.separator();
                    ui.label("Progress:");
                    ui.add(egui::ProgressBar::new(self.slider_value).show_percentage());
                });

            // Status bar at bottom
            egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Status: Ready");
                    ui.separator();
                    ui.label(format!("Objects: {}", 25));
                    ui.separator();
                    ui.label("Astrelis Engine");
                });
            });
        });

        self.egui.render(&mut render_ctx);
        self.inputs.new_frame();
    }
}

impl Drop for EguiDemoApp {
    fn drop(&mut self) {
        tracing::info!("Egui demo dropped");
    }
}
