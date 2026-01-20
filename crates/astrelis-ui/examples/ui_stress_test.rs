//! UI Stress Test - Performance Benchmark
//!
//! Stress tests the UI system with thousands of widgets:
//! - 1000+ widgets with rapid updates
//! - Dirty flag optimization validation
//! - Layout performance measurement
//! - Render batching efficiency
//! - Frame time metrics
//!
//! Press SPACE to toggle updates.
//! Press + /- to adjust widget count.

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{Color, GraphicsContext, RenderTarget, RenderableWindow, WindowContextDescriptor, wgpu};
use astrelis_ui::UiSystem;
use astrelis_winit::{WindowId, app::{App, AppCtx, run_app}, event::{EventBatch, Event, HandleStatus, Key, NamedKey}, window::{WinitPhysicalSize, WindowBackend, WindowDescriptor}};
use astrelis_winit::time::FrameTime;
use std::time::Instant;

struct UiStressTest {
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
    widget_count: usize,
    updating: bool,
    frame_count: u64,
    last_fps_time: Instant,
    fps: f32,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync();
        let window = ctx.create_window(WindowDescriptor {
            title: "UI Stress Test - Performance Benchmark".to_string(),
            size: Some(WinitPhysicalSize::new(1400.0, 900.0)),
            ..Default::default()
        }).expect("Failed to create window");

        let window = RenderableWindow::new_with_descriptor(window, graphics_ctx.clone(), WindowContextDescriptor {
            format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
            ..Default::default()
        });

        let window_id = window.id();
        let mut ui = UiSystem::new(graphics_ctx.clone());
        ui.set_viewport(window.viewport());

        let widget_count = 100;
        build_stress_ui(&mut ui, widget_count, 0);

        println!("\n═════════════════════════════════════════════════════");
        println!("  ⚡ UI STRESS TEST - Performance Benchmark");
        println!("═════════════════════════════════════════════════════");
        println!("  CONTROLS:");
        println!("    [Space]  Toggle updates on/off");
        println!("    [+/-]    Increase/decrease widget count");
        println!("  Starting with {} widgets", widget_count);
        println!("═════════════════════════════════════════════════════\n");

        Box::new(UiStressTest {
            window, window_id, ui, widget_count, updating: false,
            frame_count: 0, last_fps_time: Instant::now(), fps: 0.0
        })
    });
}

fn build_stress_ui(ui: &mut UiSystem, count: usize, frame: u64) {
    let width = 1400.0;
    let height = 900.0;

    // Predefined color palette for widget backgrounds
    let colors = [
        Color::from_rgb_u8(255, 100, 100),
        Color::from_rgb_u8(100, 255, 100),
        Color::from_rgb_u8(100, 100, 255),
        Color::from_rgb_u8(255, 255, 100),
        Color::from_rgb_u8(255, 100, 255),
        Color::from_rgb_u8(100, 255, 255),
        Color::from_rgb_u8(255, 150, 100),
        Color::from_rgb_u8(150, 255, 100),
        Color::from_rgb_u8(100, 150, 255),
        Color::from_rgb_u8(200, 100, 200),
    ];

    ui.build(|root| {
        root.container().width(width).height(height).padding(20.0).background_color(Color::from_rgb_u8(20, 20, 30)).child(|root| {
            let mut col = root.column().gap(5.0);

            // Header
            col = col.child(|root| {
                root.text(format!("Widgets: {} | Frame: {} | Press SPACE to toggle updates", count, frame))
                    .size(16.0).color(Color::WHITE).build()
            });

            // Grid of widgets
            col = col.child(|root| {
                let cols = 10;
                let rows = (count + cols - 1) / cols;
                let mut grid_col = root.column().gap(2.0);

                for row in 0..rows {
                    grid_col = grid_col.child(|root| {
                        let mut grid_row = root.row().gap(2.0);

                        for col_idx in 0..cols {
                            let idx = row * cols + col_idx;
                            if idx < count {
                                let color = colors[idx % colors.len()];
                                // Calculate a value that changes each frame
                                let value = ((frame as f32 + idx as f32 * 0.5).sin() * 50.0 + 50.0) as i32;
                                grid_row = grid_row.child(|root| {
                                    root.container().width(120.0).height(60.0).background_color(color)
                                        .justify_content(taffy::JustifyContent::Center)
                                        .align_items(taffy::AlignItems::Center)
                                        .child(|root| {
                                            root.text(format!("#{}\n{}", idx, value)).size(12.0).color(Color::WHITE).build()
                                        }).build()
                                });
                            } else {
                                grid_row = grid_row.child(|root| {
                                    root.container().width(120.0).height(60.0).build()
                                });
                            }
                        }

                        grid_row.build()
                    });
                }

                grid_col.build()
            });

            col.build()
        }).build();
    });
}

impl App for UiStressTest {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        new_frame();
        self.ui.update(0.016);
        self.frame_count += 1;

        let now = Instant::now();
        if now.duration_since(self.last_fps_time).as_secs_f32() >= 1.0 {
            self.fps = self.frame_count as f32;
            self.frame_count = 0;
            self.last_fps_time = now;
            println!("FPS: {:.1} | Widgets: {} | Updating: {}", self.fps, self.widget_count, self.updating);
        }
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id { return; }

        events.dispatch(|event| {
            if let Event::WindowResized(size) = event {
                self.window.resized(*size);
                self.ui.set_viewport(self.window.viewport());
                build_stress_ui(&mut self.ui, self.widget_count, self.frame_count);
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        events.dispatch(|event| {
            if let Event::KeyInput(key) = event {
                if key.state == astrelis_winit::event::ElementState::Pressed {
                    match key.logical_key {
                        Key::Named(NamedKey::Space) => {
                            self.updating = !self.updating;
                            println!("Updating: {}", self.updating);
                            return HandleStatus::consumed();
                        }
                        Key::Character(ref c) if c == "+" || c == "=" => {
                            self.widget_count = (self.widget_count + 50).min(5000);
                            build_stress_ui(&mut self.ui, self.widget_count, self.frame_count);
                            return HandleStatus::consumed();
                        }
                        Key::Character(ref c) if c == "-" => {
                            self.widget_count = self.widget_count.saturating_sub(50).max(10);
                            build_stress_ui(&mut self.ui, self.widget_count, self.frame_count);
                            return HandleStatus::consumed();
                        }
                        _ => {}
                    }
                }
            }
            HandleStatus::ignored()
        });

        self.ui.handle_events(events);

        if self.updating {
            build_stress_ui(&mut self.ui, self.widget_count, self.frame_count);
        }

        let mut frame = self.window.begin_drawing();
        frame.clear_and_render(RenderTarget::Surface, Color::from_rgb_u8(20, 20, 30), |pass| {
            self.ui.render(pass.descriptor());
        });
        frame.finish();
    }
}
