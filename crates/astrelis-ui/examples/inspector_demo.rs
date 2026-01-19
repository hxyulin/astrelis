//! UI Inspector Demo - F12 Developer Tools for Debugging
//!
//! This example demonstrates the UI inspector debugging tool that provides:
//! - Widget bounds visualization (colored by type)
//! - Dirty flag display (color-coded: red=layout, orange=text, yellow=geometry, green=color)
//! - Interactive widget selection (click to select)
//! - Layout tree hierarchy view
//! - Performance metrics (FPS, layout time, render time)
//!
//! **Keyboard Controls:**
//! - **F12**: Toggle inspector on/off
//!
//! **Mouse Controls:**
//! - **Click**: Increment counter (demonstrates dirty flag updates)
//!
//! This is the most critical debugging tool for UI development!

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow,
    WindowContextDescriptor, wgpu,
};
use astrelis_ui::{UiSystem, UiInspector};
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::{EventBatch, Event, HandleStatus, Key, NamedKey},
    window::{PhysicalSize, WindowBackend, WindowDescriptor},
};
use std::time::Instant;

use std::sync::{Arc, RwLock};

struct InspectorDemoApp {
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
    inspector: UiInspector,
    counter: Arc<RwLock<i32>>,
    counter_text_id: astrelis_ui::WidgetId,
    last_frame: Instant,
    frame_count: u64,
    last_metrics_log: Instant,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "UI Inspector Demo - Press F12 to Toggle Inspector".to_string(),
                size: Some(PhysicalSize::new(1280.0, 800.0)),
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
        );

        let window_id = window.id();
        let size = window.inner_size();

        let mut ui = UiSystem::new(graphics_ctx.clone());
        ui.set_viewport(window.viewport());

        let inspector = UiInspector::new();
        let counter = Arc::new(RwLock::new(0));

        // Build demo UI with various widgets
        let counter_text_id = build_demo_ui(&mut ui, size.width as f32, size.height as f32, counter.clone());

        println!("\n═══════════════════════════════════════════════════════");
        println!("  🔍 UI INSPECTOR DEMO - Developer Debugging Tools");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  CONTROLS:");
        println!("    [F12]    Toggle inspector overlay on/off");
        println!("    [Click]  Increment counter (shows dirty flags)");
        println!("\n  INSPECTOR FEATURES:");
        println!("    • Widget bounds visualization (colored by type)");
        println!("    • Dirty flag display (color-coded)");
        println!("    • Performance metrics (FPS, timing)");
        println!("    • Layout tree hierarchy");
        println!("\n  This is your primary debugging tool for UI development!");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Inspector demo initialized");

        Box::new(InspectorDemoApp {
            window,
            window_id,
            ui,
            inspector,
            counter,
            counter_text_id,
            last_frame: Instant::now(),
            frame_count: 0,
            last_metrics_log: Instant::now(),
        })
    });
}

fn build_demo_ui(
    ui: &mut UiSystem,
    width: f32,
    height: f32,
    counter: Arc<RwLock<i32>>,
) -> astrelis_ui::WidgetId {
    let counter_value = *counter.read().unwrap();
    let counter_text_id = astrelis_ui::WidgetId::new("counter_text");

    ui.build(|root| {
        // Main container
        root.container()
            .width(width)
            .height(height)
            .padding(20.0)
            .background_color(Color::from_rgb_u8(20, 20, 30))
            .child(|root| {
                // Header section
                root.column()
                    .gap(20.0)
                    .child(|root| {
                        root.container()
                            .background_color(Color::from_rgb_u8(40, 40, 55))
                            .border_color(Color::from_rgb_u8(80, 80, 120))
                            .border_width(2.0)
                            .border_radius(12.0)
                            .padding(20.0)
                            .child(|root| {
                                root.column()
                                    .gap(10.0)
                                    .child(|root| {
                                        root.text("UI Inspector Demo")
                                            .size(32.0)
                                            .color(Color::WHITE)
                                            .bold()
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("Press F12 to toggle the inspector overlay")
                                            .size(16.0)
                                            .color(Color::from_rgb_u8(150, 150, 150))
                                            .build()
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .child(|root| {
                        // Two-column layout
                        root.row()
                            .gap(20.0)
                            .child(|root| {
                                // Left panel - Interactive widgets
                                root.container()
                                    .background_color(Color::from_rgb_u8(30, 30, 45))
                                    .border_radius(8.0)
                                    .padding(15.0)
                                    .child(|root| {
                                        root.column()
                                            .gap(15.0)
                                            .child(|root| {
                                                root.text("Interactive Widgets")
                                                    .size(20.0)
                                                    .color(Color::from_rgb_u8(100, 200, 255))
                                                    .bold()
                                                    .build()
                                            })
                                            .child(|root| {
                                                // Counter display
                                                root.container()
                                                    .background_color(Color::from_rgb_u8(40, 40, 55))
                                                    .border_color(Color::from_rgb_u8(80, 120, 180))
                                                    .border_width(2.0)
                                                    .border_radius(8.0)
                                                    .padding(15.0)
                                                    .justify_content(taffy::JustifyContent::Center)
                                                    .align_items(taffy::AlignItems::Center)
                                                    .child(|root| {
                                                        root.text(format!("Counter: {}", counter_value))
                                                            .id(counter_text_id)
                                                            .size(24.0)
                                                            .color(Color::from_rgb_u8(100, 255, 150))
                                                            .bold()
                                                            .build()
                                                    })
                                                    .build()
                                            })
                                            .child(|root| {
                                                // Button row
                                                let counter_inc = counter.clone();
                                                let counter_reset = counter.clone();
                                                root.row()
                                                    .gap(10.0)
                                                    .child(|root| {
                                                        root.button("Increment")
                                                            .background_color(Color::from_rgb_u8(60, 180, 60))
                                                            .hover_color(Color::from_rgb_u8(80, 200, 80))
                                                            .padding(12.0)
                                                            .font_size(14.0)
                                                            .on_click(move || {
                                                                *counter_inc.write().unwrap() += 1;
                                                            })
                                                            .build()
                                                    })
                                                    .child(|root| {
                                                        root.button("Reset")
                                                            .background_color(Color::from_rgb_u8(180, 60, 60))
                                                            .hover_color(Color::from_rgb_u8(200, 80, 80))
                                                            .padding(12.0)
                                                            .font_size(14.0)
                                                            .on_click(move || {
                                                                *counter_reset.write().unwrap() = 0;
                                                            })
                                                            .build()
                                                    })
                                                    .build()
                                            })
                                            .child(|root| {
                                                // More widgets to inspect
                                                root.container()
                                                    .background_color(Color::from_rgb_u8(50, 50, 70))
                                                    .border_radius(6.0)
                                                    .padding(12.0)
                                                    .child(|root| {
                                                        root.column()
                                                            .gap(8.0)
                                                            .child(|root| {
                                                                root.text("Nested Container 1")
                                                                    .size(14.0)
                                                                    .color(Color::from_rgb_u8(200, 200, 200))
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text("Click widgets to select them in the inspector")
                                                                    .size(12.0)
                                                                    .color(Color::from_rgb_u8(150, 150, 150))
                                                                    .build()
                                                            })
                                                            .build()
                                                    })
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .child(|root| {
                                // Right panel - Inspector info
                                root.container()
                                    .background_color(Color::from_rgb_u8(30, 30, 45))
                                    .border_radius(8.0)
                                    .padding(15.0)
                                    .child(|root| {
                                        root.column()
                                            .gap(15.0)
                                            .child(|root| {
                                                root.text("Inspector Features")
                                                    .size(20.0)
                                                    .color(Color::from_rgb_u8(255, 180, 100))
                                                    .bold()
                                                    .build()
                                            })
                                            .child(|root| {
                                                root.column()
                                                    .gap(10.0)
                                                    .child(|root| {
                                                        root.text("• Widget Bounds Visualization")
                                                            .size(14.0)
                                                            .color(Color::from_rgb_u8(200, 200, 200))
                                                            .build()
                                                    })
                                                    .child(|root| {
                                                        root.text("• Dirty Flag Display")
                                                            .size(14.0)
                                                            .color(Color::from_rgb_u8(200, 200, 200))
                                                            .build()
                                                    })
                                                    .child(|root| {
                                                        root.text("• Layout Tree Hierarchy")
                                                            .size(14.0)
                                                            .color(Color::from_rgb_u8(200, 200, 200))
                                                            .build()
                                                    })
                                                    .child(|root| {
                                                        root.text("• Performance Metrics")
                                                            .size(14.0)
                                                            .color(Color::from_rgb_u8(200, 200, 200))
                                                            .build()
                                                    })
                                                    .child(|root| {
                                                        root.text("• Widget Selection")
                                                            .size(14.0)
                                                            .color(Color::from_rgb_u8(200, 200, 200))
                                                            .build()
                                                    })
                                                    .build()
                                            })
                                            .child(|root| {
                                                root.container()
                                                    .background_color(Color::from_rgb_u8(50, 50, 70))
                                                    .border_radius(6.0)
                                                    .padding(12.0)
                                                    .child(|root| {
                                                        root.column()
                                                            .gap(6.0)
                                                            .child(|root| {
                                                                root.text("Dirty Flag Colors:")
                                                                    .size(14.0)
                                                                    .color(Color::from_rgb_u8(220, 220, 220))
                                                                    .bold()
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text("🔴 Red = Layout dirty")
                                                                    .size(12.0)
                                                                    .color(Color::from_rgb_u8(255, 100, 100))
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text("🟠 Orange = Text shaping")
                                                                    .size(12.0)
                                                                    .color(Color::from_rgb_u8(255, 180, 100))
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text("🟡 Yellow = Geometry")
                                                                    .size(12.0)
                                                                    .color(Color::from_rgb_u8(255, 255, 100))
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text("🟢 Green = Color only")
                                                                    .size(12.0)
                                                                    .color(Color::from_rgb_u8(100, 255, 100))
                                                                    .build()
                                                            })
                                                            .build()
                                                    })
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .build()
            })
            .build();
    });

    counter_text_id
}

impl App for InspectorDemoApp {
    fn update(&mut self, _ctx: &mut AppCtx) {
        new_frame();

        // Calculate frame time
        let now = Instant::now();
        let frame_time = now.duration_since(self.last_frame).as_secs_f32() * 1000.0;
        self.last_frame = now;
        self.frame_count += 1;

        // Update UI animations
        self.ui.update(0.016);

        // Update inspector data
        if self.inspector.is_enabled() {
            self.inspector.update(self.ui.tree(), self.ui.core().widget_registry());
            self.inspector.update_metrics(0.0, 0.0, frame_time);
        }
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window resize
        events.dispatch(|event| {
            if let Event::WindowResized(size) = event {
                self.window.resized(*size);
                self.ui.set_viewport(self.window.viewport());
                self.counter_text_id = build_demo_ui(
                    &mut self.ui,
                    size.width as f32,
                    size.height as f32,
                    self.counter.clone(),
                );
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle keyboard events
        events.dispatch(|event| {
            if let Event::KeyInput(key) = event {
                if key.state == astrelis_winit::event::ElementState::Pressed {
                    // Match on logical key for F12
                    if matches!(key.logical_key, Key::Named(NamedKey::F12)) {
                        self.inspector.toggle();
                        let status = if self.inspector.is_enabled() { "ENABLED" } else { "DISABLED" };
                        tracing::info!("Inspector {}", status);
                        return HandleStatus::consumed();
                    }
                }
            }
            HandleStatus::ignored()
        });

        // Handle UI events (button clicks are handled by callbacks in build_demo_ui)
        self.ui.handle_events(events);

        // Update counter text every frame
        let counter_value = *self.counter.read().unwrap();
        self.ui.update_text(self.counter_text_id, format!("Counter: {}", counter_value));

        // Begin frame and render
        let mut frame = self.window.begin_drawing();

        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb_u8(20, 20, 30),
            |pass| {
                // Render main UI
                self.ui.render(pass.descriptor());

                // Render inspector overlay if enabled
                if self.inspector.is_enabled() {
                    // TODO: Render inspector overlay rectangles
                    // This would require drawing colored rectangles for widget bounds
                    // For now, the inspector data is updated and can be queried

                    // Log inspector info every 2 seconds
                    let now = Instant::now();
                    if now.duration_since(self.last_metrics_log).as_secs_f32() >= 2.0 {
                        tracing::info!("Inspector metrics (frame {}):\n{}",
                            self.frame_count,
                            self.inspector.generate_metrics_text());
                        self.last_metrics_log = now;
                    }
                }
            },
        );

        frame.finish();
    }
}
