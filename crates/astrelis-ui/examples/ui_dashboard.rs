//! Dashboard example simulating high-frequency text updates.
//!
//! This example creates a dashboard with many "telemetry" cards that update
//! their values every frame. This stresses the text pipeline and dirty tracking system.
//!
//! Features:
//! - Grid layout
//! - High frequency text updates (simulating 60Hz telemetry)
//! - Performance metrics display

use astrelis_core::logging;
use astrelis_core::profiling::{init_profiling, ProfilingBackend, new_frame};
use astrelis_render::{
    GraphicsContext, RenderPassBuilder, RenderTarget, RenderableWindow, WindowContextDescriptor,
    wgpu,
};
use astrelis_ui::{
    Color, FlexDirection, FlexWrap, JustifyContent, UiSystem, WidgetId, AlignItems,
};
use astrelis_winit::{
    WindowId,
    app::run_app,
    event::PhysicalSize,
    window::{WindowDescriptor, WindowBackend, Window},
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

struct App {
    context: &'static GraphicsContext,
    windows: HashMap<WindowId, RenderableWindow>,
    ui: UiSystem,
    last_update: Instant,
    frame_count: u64,
    last_metrics_print: Instant,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_sync();
        let mut windows = HashMap::new();

        let scale = Window::platform_dpi() as f32;
        let window = ctx
            .create_window(WindowDescriptor {
                title: "Astrelis UI - Dashboard Performance".to_string(),
                size: Some(PhysicalSize::new(1280.0 * scale, 800.0 * scale)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let renderable_window = RenderableWindow::new_with_descriptor(
            window,
            graphics_ctx,
            WindowContextDescriptor {
                format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                ..Default::default()
            },
        );

        let window_id = renderable_window.id();
        windows.insert(window_id, renderable_window);

        let mut ui = UiSystem::new(graphics_ctx);
        
        // Build the initial UI
        build_dashboard(&mut ui);

        Box::new(App {
            context: graphics_ctx,
            windows,
            ui,
            last_update: Instant::now(),
            frame_count: 0,
            last_metrics_print: Instant::now(),
        })
    });
}

fn build_dashboard(ui: &mut UiSystem) {
    ui.build(|root| {
        root.container()
            .width(1280.0)
            .height(800.0)
            .background_color(Color::from_rgb_u8(20, 20, 25))
            .padding(20.0)
            .flex_direction(FlexDirection::Column)
            .child(|col| {
                // Header
                col.row()
                    .height(60.0)
                    .justify_content(JustifyContent::SpaceBetween)
                    .align_items(AlignItems::Center)
                    .child(|header| {
                        header.text("System Telemetry")
                            .size(24.0)
                            .bold()
                            .color(Color::WHITE)
                            .build();
                        
                        header.text("Status: ONLINE")
                            .size(16.0)
                            .color(Color::from_rgb_u8(100, 255, 100))
                            .build()
                    })
                    .build();

                // Grid of cards
                col.container()
                    .flex_direction(FlexDirection::Row)
                    .flex_wrap(FlexWrap::Wrap)
                    .gap(10.0)
                    .children(|grid| {
                        let mut ids = Vec::new();
                        // Create 100 telemetry cards
                        for i in 0..100 {
                            let card_id = grid.container()
                                .width(230.0)
                                .height(100.0)
                                .background_color(Color::from_rgb_u8(40, 40, 50))
                                .border_radius(8.0)
                                .padding(15.0)
                                .child(|card| {
                                    card.column()
                                        .gap(5.0)
                                        .child(|content| {
                                            content.text(format!("Sensor #{}", i))
                                                .size(14.0)
                                                .color(Color::from_rgb_u8(150, 150, 170))
                                                .build();
                                            
                                            content.text("0.000")
                                                .size(28.0)
                                                .bold()
                                                .color(Color::WHITE)
                                                .id(WidgetId::new(&format!("value_{}", i)))
                                                .build()
                                        })
                                        .build()
                                })
                                .build();
                            ids.push(card_id);
                        }
                        ids
                    })
                    .build()
            })
            .build();
    });
}

impl astrelis_winit::app::App for App {
    fn update(&mut self, _ctx: &mut astrelis_winit::app::AppCtx) {
        new_frame();
        let now = Instant::now();
        let _dt = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;
        self.frame_count += 1;

        // Update telemetry values every frame
        for i in 0..100 {
            let value = (now.elapsed().as_secs_f32() * (i as f32 * 0.1)).sin() * 100.0;
            let id = WidgetId::new(&format!("value_{}", i));
            self.ui.update_text(id, format!("{:.3}", value));
        }

        // Print metrics every second
        if now.duration_since(self.last_metrics_print) > Duration::from_secs(1) {
            let metrics = self.ui.core().tree().last_metrics();
            if let Some(m) = metrics {
                println!("FPS: {:.1} | Layout: {:.2}ms | Text Dirty: {} | Paint Dirty: {}", 
                    self.frame_count as f32,
                    m.layout_time.as_secs_f64() * 1000.0,
                    m.nodes_text_dirty,
                    m.nodes_paint_dirty
                );
                self.ui.log_text_cache_stats();
            }
            self.last_metrics_print = now;
            self.frame_count = 0;
        }
    }

    fn render(
        &mut self,
        _ctx: &mut astrelis_winit::app::AppCtx,
        window_id: WindowId,
        events: &mut astrelis_winit::event::EventBatch,
    ) {
        let Some(window) = self.windows.get_mut(&window_id) else {
            return;
        };

        // Handle resize
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                window.resized(*size);
                self.ui.set_viewport(astrelis_render::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: size.width as f32,
                    height: size.height as f32,
                    scale_factor: window.scale_factor(),
                });
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        let mut frame = window.begin_drawing();

        {
            let mut render_pass = RenderPassBuilder::new()
                .label("UI Render Pass")
                .target(RenderTarget::Surface)
                .clear_color(wgpu::Color::BLACK)
                .build(&mut frame);

            self.ui.render(render_pass.descriptor());
        }

        frame.finish();
    }
}
