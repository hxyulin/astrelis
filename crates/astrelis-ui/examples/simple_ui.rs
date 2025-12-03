use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{
    Color, GraphicsContext, RenderPassBuilder, RenderableWindow, WindowContextDescriptor, wgpu,
};
use astrelis_ui::{UiSystem, widgets::*};
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{PhysicalSize, WindowBackend, WindowDescriptor},
};

struct SimpleUiApp {
    graphics_ctx: &'static GraphicsContext,
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
}

fn main() {
    logging::init();

    // Initialize profiling - connect to puffin_viewer at http://127.0.0.1:8585
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_sync();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Simple UI Example".to_string(),
                size: Some(PhysicalSize::new(800.0, 600.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderableWindow::new_with_descriptor(
            window,
            graphics_ctx,
            WindowContextDescriptor {
                format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                ..Default::default()
            },
        );

        let window_id = window.id();
        let ui = UiSystem::new(graphics_ctx);

        Box::new(SimpleUiApp {
            graphics_ctx,
            window,
            window_id,
            ui,
        })
    });
}

impl App for SimpleUiApp {
    fn update(&mut self, _ctx: &mut AppCtx) {
        // Mark new profiling frame
        new_frame();

        // Update UI animations, etc.
        self.ui.update(0.016); // ~60 FPS
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window resize events
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                self.ui.set_viewport(self.window.viewport());

                // Rebuild UI with new viewport size
                let viewport_width = size.width as f32;
                let viewport_height = size.height as f32;
                self.ui.build(|root| {
                    root.container()
                        .width(viewport_width)
                        .height(viewport_height)
                        .padding(20.0)
                        .background_color(Color::from_rgb_u8(30, 30, 40))
                        .child(|root| {
                            root.column()
                                .gap(20.0)
                                .child(|root| {
                                    root.text("Astrelis UI System")
                                        .size(32.0)
                                        .color(Color::WHITE)
                                        .bold()
                                        .padding(10.0)
                                        .build()
                                })
                                .child(|root| {
                                    root.text("Built with Taffy Layout Engine")
                                        .size(18.0)
                                        .color(Color::from_rgb_u8(150, 150, 200))
                                        .padding(10.0)
                                        .build()
                                })
                                .child(|root| {
                                    root.row()
                                        .gap(10.0)
                                        .padding(20.0)
                                        .child(|root| {
                                            root.button("Click Me")
                                                .background_color(Color::from_rgb_u8(60, 120, 200))
                                                .hover_color(Color::from_rgb_u8(80, 140, 220))
                                                .padding(15.0)
                                                .build()
                                        })
                                        .child(|root| {
                                            root.button("Another Button")
                                                .background_color(Color::from_rgb_u8(200, 60, 120))
                                                .hover_color(Color::from_rgb_u8(220, 80, 140))
                                                .padding(15.0)
                                                .build()
                                        })
                                        .child(|root| {
                                            root.button("Disabled")
                                                .background_color(Color::from_rgb_u8(100, 100, 100))
                                                .hover_color(Color::from_rgb_u8(120, 120, 120))
                                                .padding(15.0)
                                                .build()
                                        })
                                        .build()
                                })
                                .child(|root| {
                                    root.container()
                                        .background_color(Color::from_rgb_u8(50, 50, 70))
                                        .border_color(Color::from_rgb_u8(100, 100, 150))
                                        .border_width(2.0)
                                        .border_radius(8.0)
                                        .padding(20.0)
                                        .margin(20.0)
                                        .child(|root| {
                                            root.column()
                                                .gap(10.0)
                                                .child(|root| {
                                                    root.text("Features:")
                                                        .size(20.0)
                                                        .color(Color::WHITE)
                                                        .bold()
                                                        .build()
                                                })
                                                .child(|root| {
                                                    root.text("- Flexbox layouts with Taffy")
                                                        .size(16.0)
                                                        .color(Color::from_rgb_u8(200, 200, 200))
                                                        .build()
                                                })
                                                .child(|root| {
                                                    root.text("- GPU-accelerated rendering")
                                                        .size(16.0)
                                                        .color(Color::from_rgb_u8(200, 200, 200))
                                                        .build()
                                                })
                                                .child(|root| {
                                                    root.text(
                                                        "- Interactive buttons with hover states",
                                                    )
                                                    .size(16.0)
                                                    .color(Color::from_rgb_u8(200, 200, 200))
                                                    .build()
                                                })
                                                .child(|root| {
                                                    root.text("- Event handling system")
                                                        .size(16.0)
                                                        .color(Color::from_rgb_u8(200, 200, 200))
                                                        .build()
                                                })
                                                .build()
                                        })
                                        .build()
                                })
                                .child(|root| {
                                    root.text("Press Ctrl+C to exit")
                                        .size(14.0)
                                        .color(Color::from_rgb_u8(150, 150, 150))
                                        .margin(20.0)
                                        .build()
                                })
                                .build()
                        })
                        .build();
                });

                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        // Handle UI events (mouse, keyboard, etc.)
        self.ui.handle_events(events);

        // Begin frame and render
        let mut frame = self.window.begin_drawing();

        {
            let mut render_pass = RenderPassBuilder::new()
                .label("UI Render Pass")
                .color_attachment(
                    None,
                    None,
                    wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Color::from_rgb_u8(20, 20, 30).to_wgpu()),
                        store: wgpu::StoreOp::Store,
                    },
                )
                .build(&mut frame);

            // Render UI
            self.ui.render(render_pass.descriptor());
        }

        frame.finish();
    }
}
