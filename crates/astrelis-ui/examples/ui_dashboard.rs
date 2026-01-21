//! Dashboard Performance Demo - Incremental Text Updates
//!
//! **Purpose**: Tests incremental text update performance using `.update_text()` API.
//! This is different from ui_stress_test which tests full UI rebuild performance.
//!
//! This example simulates a telemetry dashboard with many values updating every frame.
//! It uses the INCREMENTAL UPDATE API (update_text) to modify existing widgets without
//! rebuilding the entire UI tree, demonstrating the dirty flag optimization system.
//!
//! Features:
//! - 100 telemetry values updating 60 times per second
//! - Uses .update_text() for incremental updates (TEXT_SHAPING dirty flag only)
//! - Performance metrics: layout time, dirty node counts
//! - F12 inspector toggle for debugging
//!
//! Compare with:
//! - ui_stress_test.rs: Tests full UI rebuild performance
//!
//! Controls:
//! - Press F12 to toggle UI inspector (shows widget bounds, dirty flags, layout tree)

use astrelis_core::logging;
use astrelis_core::profiling::{init_profiling, ProfilingBackend, new_frame};
use astrelis_render::{
    GraphicsContext, RenderTarget, RenderableWindow, WindowContextDescriptor,
    wgpu,
};
use astrelis_ui::{
    Color, FlexDirection, FlexWrap, JustifyContent, UiSystem, WidgetId, AlignItems,
};
use astrelis_winit::{
    WindowId,
    app::run_app,
    event::{Event, HandleStatus, Key, NamedKey},
    window::{WindowDescriptor, WindowBackend, Window, WinitPhysicalSize},
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::sync::Arc;

struct App {
    context: Arc<GraphicsContext>,
    windows: HashMap<WindowId, RenderableWindow>,
    ui: UiSystem,
    last_update: Instant,
    frame_count: u64,
    last_metrics_print: Instant,
    inspector_enabled: bool,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync_or_panic();
        let mut windows = HashMap::new();

        let scale = Window::platform_dpi() as f32;
        let window = ctx
            .create_window(WindowDescriptor {
                title: "Astrelis UI - Dashboard Performance".to_string(),
                size: Some(WinitPhysicalSize::new(1280.0 * scale, 800.0 * scale)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let renderable_window = RenderableWindow::new_with_descriptor(
            window,
            graphics_ctx.clone(),
            WindowContextDescriptor {
                format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                ..Default::default()
            },
        ).expect("Failed to create renderable window");

        let window_id = renderable_window.id();
        let viewport = renderable_window.viewport();
        windows.insert(window_id, renderable_window);

        let mut ui = UiSystem::new(graphics_ctx.clone());
        ui.set_viewport(viewport);
        
        // Build the initial UI
        build_dashboard(&mut ui);

        println!("\n═══════════════════════════════════════════════════════");
        println!("  📊 UI DASHBOARD - Incremental Update Performance");
        println!("═══════════════════════════════════════════════════════");
        println!("  PURPOSE:");
        println!("    Tests .update_text() API performance (dirty flags)");
        println!("    100 values update 60x/sec WITHOUT rebuilding UI tree");
        println!("\n  COMPARE WITH:");
        println!("    ui_stress_test - Tests full UI rebuild performance");
        println!("\n  CONTROLS:");
        println!("    [F12]  Toggle UI Inspector");
        println!("═══════════════════════════════════════════════════════\n");

        Box::new(App {
            context: graphics_ctx,
            windows,
            ui,
            last_update: Instant::now(),
            frame_count: 0,
            last_metrics_print: Instant::now(),
            inspector_enabled: false,
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
    fn update(&mut self, _ctx: &mut astrelis_winit::app::AppCtx, _time: &astrelis_winit::FrameTime) {
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
            if let Event::WindowResized(size) = event {
                window.resized(*size);
                self.ui.set_viewport(window.viewport());
                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        // Handle F12 key for inspector toggle
        events.dispatch(|event| {
            if let Event::KeyInput(key) = event {
                if key.state == astrelis_winit::event::ElementState::Pressed {
                    match &key.logical_key {
                        Key::Named(NamedKey::F12) => {
                            self.inspector_enabled = !self.inspector_enabled;
                            println!("Inspector: {}", if self.inspector_enabled { "ON" } else { "OFF" });
                            return HandleStatus::consumed();
                        }
                        _ => {}
                    }
                }
            }
            HandleStatus::ignored()
        });

        let mut frame = window.begin_drawing();

        // Render UI with automatic scoping (no manual {} block needed)
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::BLACK,
            |pass| {
                self.ui.render(pass.descriptor());
            },
        );

        frame.finish();
    }
}
