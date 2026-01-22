//! Overflow & Viewport Units Demo
//!
//! This example demonstrates:
//! - Overflow clipping (Overflow::Hidden)
//! - Nested clip regions
//! - Viewport units (vw, vh, vmin, vmax)
//!
//! The example shows several panels with overflow content that gets clipped
//! at their bounds, demonstrating the scissor rect system.
//!
//! Controls:
//! - Resize the window to see viewport units in action
//! - The panels will clip their overflow content

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow,
    WindowContextDescriptor, wgpu,
};
use astrelis_ui::{UiSystem, Overflow, UiBuilder, NodeId};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{Event, EventBatch, HandleStatus},
    window::{WinitPhysicalSize, WindowBackend, WindowDescriptor},
};

struct OverflowDemoApp {
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync_or_panic();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Overflow & Viewport Units Demo".to_string(),
                size: Some(WinitPhysicalSize::new(800.0, 600.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderableWindow::new_with_descriptor(
            window,
            graphics_ctx.clone(),
            WindowContextDescriptor {
                format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                ..Default::default()
            },
        ).expect("Failed to create renderable window");

        let window_id = window.id();

        // Create UI system
        let mut ui = UiSystem::new(graphics_ctx.clone());
        ui.set_viewport(window.viewport());

        // Build initial UI
        build_overflow_ui(&mut ui, &window);

        tracing::info!("Overflow demo initialized");
        tracing::info!("Resize the window to see viewport units in action");

        Box::new(OverflowDemoApp {
            window,
            window_id,
            ui,
        })
    });
}

fn build_overflow_ui(ui: &mut UiSystem, window: &RenderableWindow) {
    let size = window.physical_size();
    let width = size.width as f32;
    let height = size.height as f32;

    ui.build(|root| {
        // Main container - full viewport
        root.container()
            .width(width)
            .height(height)
            .padding(20.0)
            .background_color(Color::from_rgb_u8(30, 30, 40))
            .child(|root| {
                root.column()
                    .gap(20.0)
                    .child(|root| {
                        // Title
                        root.text("Overflow & Viewport Units Demo")
                            .size(24.0)
                            .color(Color::WHITE)
                            .bold()
                            .build()
                    })
                    .child(|root| {
                        // Subtitle showing viewport info
                        root.text(format!("Viewport: {}x{} px", width as i32, height as i32))
                            .size(14.0)
                            .color(Color::from_rgb_u8(150, 150, 160))
                            .build()
                    })
                    .child(|root| {
                        // Row of demo panels
                        root.row()
                            .gap(20.0)
                            .child(|root| {
                                // Panel 1: Basic overflow hidden
                                build_overflow_hidden_panel(root)
                            })
                            .child(|root| {
                                // Panel 2: Viewport unit sizes
                                build_viewport_units_panel(root)
                            })
                            .build()
                    })
                    .child(|root| {
                        // Info text
                        root.text("The panels above demonstrate overflow clipping and viewport units.")
                            .size(12.0)
                            .color(Color::from_rgb_u8(120, 120, 130))
                            .build()
                    })
                    .build()
            })
            .build();
    });
}

fn build_overflow_hidden_panel(root: &mut UiBuilder) -> NodeId {
    root.container()
        .width(250.0)
        .height(200.0)
        .background_color(Color::from_rgb_u8(50, 50, 65))
        .border_color(Color::from_rgb_u8(80, 80, 100))
        .border_width(2.0)
        .border_radius(8.0)
        .overflow(Overflow::Hidden) // This clips overflow content
        .child(|root| {
            let mut col = root.column()
                .padding(10.0)
                .gap(5.0)
                .child(|root| {
                    root.text("Overflow: Hidden")
                        .size(16.0)
                        .color(Color::from_rgb_u8(100, 200, 255))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("This panel clips its content.")
                        .size(12.0)
                        .color(Color::from_rgb_u8(180, 180, 190))
                        .build()
                });

            // Add many items that will overflow
            for i in 1..=20 {
                let color = if i <= 5 {
                    Color::from_rgb_u8(200, 200, 210)
                } else {
                    Color::from_rgb_u8(150, 150, 160)
                };
                col = col.child(move |root| {
                    root.text(format!("Item {} - This is a list item that may be clipped", i))
                        .size(12.0)
                        .color(color)
                        .build()
                });
            }

            col.build()
        })
        .build()
}

fn build_viewport_units_panel(root: &mut UiBuilder) -> NodeId {
    root.container()
        .width(300.0)  // Fixed width for now (viewport units need resolution)
        .height(200.0)
        .background_color(Color::from_rgb_u8(50, 65, 50))
        .border_color(Color::from_rgb_u8(80, 100, 80))
        .border_width(2.0)
        .border_radius(8.0)
        .child(|root| {
            root.column()
                .padding(10.0)
                .gap(8.0)
                .child(|root| {
                    root.text("Viewport Units")
                        .size(16.0)
                        .color(Color::from_rgb_u8(100, 255, 100))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Available units:")
                        .size(12.0)
                        .color(Color::from_rgb_u8(180, 190, 180))
                        .build()
                })
                .child(|root| {
                    root.text("- vw: 1% of viewport width")
                        .size(11.0)
                        .color(Color::from_rgb_u8(160, 170, 160))
                        .build()
                })
                .child(|root| {
                    root.text("- vh: 1% of viewport height")
                        .size(11.0)
                        .color(Color::from_rgb_u8(160, 170, 160))
                        .build()
                })
                .child(|root| {
                    root.text("- vmin: 1% of smaller dimension")
                        .size(11.0)
                        .color(Color::from_rgb_u8(160, 170, 160))
                        .build()
                })
                .child(|root| {
                    root.text("- vmax: 1% of larger dimension")
                        .size(11.0)
                        .color(Color::from_rgb_u8(160, 170, 160))
                        .build()
                })
                .child(|root| {
                    root.text("Resize window to see changes!")
                        .size(12.0)
                        .color(Color::from_rgb_u8(255, 200, 100))
                        .margin(10.0)
                        .build()
                })
                .build()
        })
        .build()
}

impl App for OverflowDemoApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        new_frame();
        self.ui.update(time.delta_seconds());
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window resize events
        events.dispatch(|event| {
            if let Event::WindowResized(size) = event {
                self.window.resized(*size);
                self.ui.set_viewport(self.window.viewport());

                tracing::info!(
                    "Window resized to {}x{} - rebuilding UI",
                    size.width,
                    size.height,
                );

                // Rebuild UI with new viewport size
                build_overflow_ui(&mut self.ui, &self.window);

                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle UI events
        self.ui.handle_events(events);

        // Begin frame and render
        let mut frame = self.window.begin_drawing();

        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb_u8(25, 25, 35),
            |pass| {
                self.ui.render(pass.descriptor());
            },
        );

        frame.finish();
    }
}
