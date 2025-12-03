use astrelis_core::logging;
use astrelis_egui::{Color32, Egui, RichText, Slider};
use astrelis_render::{GraphicsContext, RenderableWindow};
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{PhysicalSize, WindowBackend, WindowDescriptor},
};

struct DemoApp {
    _context: &'static GraphicsContext,
    window: RenderableWindow,
    window_id: WindowId,
    egui: Egui,

    // Demo state
    counter: i32,
    slider_value: f32,
    text_input: String,
    show_window: bool,
    checkbox_value: bool,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_sync();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "EGUI Full App Demo".to_string(),
                size: Some(PhysicalSize::new(1280.0, 720.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderableWindow::new(window, graphics_ctx);
        let window_id = window.id();
        let egui = Egui::new(&window, graphics_ctx);

        Box::new(DemoApp {
            _context: graphics_ctx,
            window,
            window_id,
            egui,
            counter: 0,
            slider_value: 50.0,
            text_input: "Edit me!".to_string(),
            show_window: true,
            checkbox_value: false,
        })
    });
}

impl App for DemoApp {
    fn update(&mut self, _ctx: &mut AppCtx) {
        // Global logic - called once per frame
        // (none needed for this example)
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window-specific resize events
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        self.egui.handle_events(&self.window, events);

        self.egui.ui(&self.window, |ctx| {
            // Top panel
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("New").clicked() {
                            self.counter = 0;
                        }
                        if ui.button("Reset").clicked() {
                            self.slider_value = 50.0;
                            self.text_input = "Edit me!".to_string();
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            std::process::exit(0);
                        }
                    });

                    ui.menu_button("View", |ui| {
                        ui.checkbox(&mut self.show_window, "Show Demo Window");
                    });

                    ui.menu_button("Help", |ui| {
                        if ui.button("About").clicked() {
                            tracing::info!("EGUI Demo App - Built with Astrelis");
                        }
                    });
                });
            });

            // Side panel
            egui::SidePanel::left("left_panel")
                .default_width(250.0)
                .show(ctx, |ui| {
                    ui.heading("Controls");
                    ui.separator();

                    ui.label("Counter:");
                    ui.horizontal(|ui| {
                        if ui.button("-").clicked() {
                            self.counter -= 1;
                        }
                        ui.label(format!("{}", self.counter));
                        if ui.button("+").clicked() {
                            self.counter += 1;
                        }
                    });

                    ui.add_space(10.0);

                    ui.label("Slider:");
                    ui.add(Slider::new(&mut self.slider_value, 0.0..=100.0));
                    ui.label(format!("Value: {:.1}", self.slider_value));

                    ui.add_space(10.0);

                    ui.label("Text Input:");
                    ui.text_edit_singleline(&mut self.text_input);

                    ui.add_space(10.0);

                    ui.checkbox(&mut self.checkbox_value, "Checkbox option");

                    ui.add_space(20.0);

                    if ui.button("Reset All").clicked() {
                        self.counter = 0;
                        self.slider_value = 50.0;
                        self.text_input = "Edit me!".to_string();
                        self.checkbox_value = false;
                    }

                    ui.separator();
                    ui.label(RichText::new("Stats").strong());
                    ui.label(format!("FPS: ~60"));
                });

            // Central panel
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Welcome to EGUI with Astrelis");
                ui.separator();

                ui.label("This is a full EGUI application example demonstrating:");
                ui.label("- Top menu bar with File, View, Help menus");
                ui.label("- Side panel with various controls");
                ui.label("- Central panel with main content");
                ui.label("- Optional floating window");

                ui.add_space(20.0);

                ui.horizontal(|ui| {
                    ui.label("Counter value:");
                    ui.colored_label(
                        if self.counter >= 0 {
                            Color32::GREEN
                        } else {
                            Color32::RED
                        },
                        format!("{}", self.counter),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label("Slider value:");
                    ui.colored_label(Color32::LIGHT_BLUE, format!("{:.1}%", self.slider_value));
                });

                ui.add_space(20.0);

                if ui.button("Increment Counter").clicked() {
                    self.counter += 1;
                }

                ui.add_space(20.0);

                ui.group(|ui| {
                    ui.label(RichText::new("Text Input Echo").strong());
                    ui.label(&self.text_input);
                });

                ui.add_space(20.0);

                ui.collapsing("Color Examples", |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::RED, "Red");
                        ui.colored_label(Color32::GREEN, "Green");
                        ui.colored_label(Color32::BLUE, "Blue");
                        ui.colored_label(Color32::YELLOW, "Yellow");
                        ui.colored_label(Color32::LIGHT_GRAY, "Gray");
                    });
                });

                ui.collapsing("Layout Examples", |ui| {
                    ui.label("Horizontal layout:");
                    ui.horizontal(|ui| {
                        ui.button("Button 1");
                        ui.button("Button 2");
                        ui.button("Button 3");
                    });

                    ui.label("Vertical layout with spacing:");
                    ui.vertical(|ui| {
                        ui.button("Button A");
                        ui.add_space(5.0);
                        ui.button("Button B");
                        ui.add_space(5.0);
                        ui.button("Button C");
                    });
                });
            });

            // Optional floating window
            if self.show_window {
                egui::Window::new("Demo Window")
                    .default_width(300.0)
                    .show(ctx, |ui| {
                        ui.label("This is a floating window!");
                        ui.label("You can drag it around.");
                        ui.separator();

                        ui.label(format!("Counter: {}", self.counter));
                        ui.label(format!("Slider: {:.1}", self.slider_value));

                        ui.add_space(10.0);

                        if ui.button("Close").clicked() {
                            self.show_window = false;
                        }
                    });
            }
        });

        let mut frame = self.window.begin_drawing();

        // Clear to dark background
        {
            use astrelis_render::RenderPassBuilder;
            let _render_pass = RenderPassBuilder::new()
                .label("Clear Pass")
                .color_attachment(
                    None,
                    None,
                    wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                )
                .build(&mut frame);
        }

        self.egui.render(&self.window, &mut frame);
        frame.finish();
    }
}
