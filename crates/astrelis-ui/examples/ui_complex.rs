//! Complex UI example simulating a large widget tree.
//!
//! This example creates a deep and wide widget tree (1000+ nodes) to stress
//! the layout engine and rendering performance.
//!
//! Features:
//! - Recursive widget generation
//! - Deep nesting
//! - Large number of nodes
//! - Layout performance metrics

use astrelis_core::logging;
use astrelis_core::profiling::{init_profiling, ProfilingBackend, new_frame};
use astrelis_render::{
    GraphicsContext, RenderPassBuilder, RenderTarget, RenderableWindow, WindowContextDescriptor,
    wgpu,
};
use astrelis_ui::{
    Color, FlexDirection, UiSystem, WidgetId, AlignItems, UiBuilder, NodeId,
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
                title: "Astrelis UI - Complex Layout".to_string(),
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
        let viewport = renderable_window.viewport();
        windows.insert(window_id, renderable_window);

        let mut ui = UiSystem::new(graphics_ctx);
        ui.set_viewport(viewport);
        
        // Build the complex UI
        build_complex_ui(&mut ui);

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

fn build_complex_ui(ui: &mut UiSystem) {
    ui.build(|root| {
        root.container()
            .width(1280.0)
            .height(800.0)
            .background_color(Color::from_rgb_u8(30, 30, 35))
            .padding(10.0)
            .flex_direction(FlexDirection::Row)
            .child(|row| {
                // Left Sidebar
                row.container()
                    .width(250.0)
                    .background_color(Color::from_rgb_u8(40, 40, 45))
                    .padding(10.0)
                    .child(|sidebar| {
                        build_tree_view(sidebar, 0, 5) // Depth 5 tree view
                    })
                    .build();

                // Main Content
                row.container()
                    .flex_direction(FlexDirection::Column)
                    .padding(10.0)
                    .gap(10.0)
                    .child(|main| {
                        // Header
                        main.container()
                            .height(50.0)
                            .background_color(Color::from_rgb_u8(50, 50, 60))
                            .child(|h| h.text("Complex Layout Demo").size(20.0).build())
                            .build();

                        // Content Grid
                        main.container()
                            .flex_direction(FlexDirection::Row)
                            .flex_wrap(astrelis_ui::FlexWrap::Wrap)
                            .gap(5.0)
                            .children(|grid| {
                                let mut ids = Vec::new();
                                // Create 500 items
                                for i in 0..500 {
                                    let id = grid.container()
                                        .width(50.0)
                                        .height(50.0)
                                        .background_color(Color::from_rgb_u8(
                                            ((i * 10) % 255) as u8,
                                            ((i * 20) % 255) as u8,
                                            ((i * 30) % 255) as u8,
                                        ))
                                        .child(|c| {
                                            c.text(format!("{}", i)).size(10.0).build()
                                        })
                                        .build();
                                    ids.push(id);
                                }
                                ids
                            })
                            .build()
                    })
                    .build()
            })
            .build();
    });
}

fn build_tree_view(builder: &mut UiBuilder, depth: usize, max_depth: usize) -> NodeId {
    if depth >= max_depth {
        return builder.text("Leaf").size(12.0).color(Color::from_rgb_u8(128, 128, 128)).build();
    }

    builder.column()
        .gap(2.0)
        .padding(5.0)
        .child(|col| {
            col.text(format!("Node Level {}", depth)).size(14.0).build();
            
            col.container()
                .padding(5.0)
                .margin(2.0)
                .background_color(Color::from_rgba_u8(255, 255, 255, 10))
                .child(|c| build_tree_view(c, depth + 1, max_depth))
                .build();
            
            col.container()
                .padding(5.0)
                .margin(2.0)
                .background_color(Color::from_rgba_u8(255, 255, 255, 10))
                .child(|c| build_tree_view(c, depth + 1, max_depth))
                .build()
        })
        .build()
}

impl astrelis_winit::app::App for App {
    fn update(&mut self, _ctx: &mut astrelis_winit::app::AppCtx) {
        new_frame();
        let now = Instant::now();
        self.last_update = now;
        self.frame_count += 1;

        // Print metrics every second
        if now.duration_since(self.last_metrics_print) > Duration::from_secs(1) {
            let metrics = self.ui.core().tree().last_metrics();
            if let Some(m) = metrics {
                println!("FPS: {:.1} | Nodes: {} | Layout: {:.2}ms", 
                    self.frame_count as f32,
                    m.total_nodes,
                    m.layout_time.as_secs_f64() * 1000.0,
                );
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
