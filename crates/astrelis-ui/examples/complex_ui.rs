//! Complex UI Demo - Multi-Panel Application Showcase
//!
//! This example demonstrates a sophisticated multi-panel UI with:
//! - Multi-column layout with navigation sidebar
//! - Dashboard with data cards and statistics
//! - Forms with various input types
//! - Nested containers and layouts
//! - Mixed interactive widgets (buttons, text inputs, etc.)
//!
//! This showcases the full power of the Astrelis UI system with proper
//! composition, nesting, and layout management.

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow,
    WindowContextDescriptor, wgpu,
};
use astrelis_ui::UiSystem;
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::{EventBatch, Event, HandleStatus},
    window::{WinitPhysicalSize, WindowBackend, WindowDescriptor},
};

struct ComplexUiApp {
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
    stats: AppStats,
}

#[derive(Clone, Copy)]
struct AppStats {
    active_users: u32,
    total_revenue: f32,
    tasks_completed: u32,
    success_rate: f32,
}

impl Default for AppStats {
    fn default() -> Self {
        Self {
            active_users: 1247,
            total_revenue: 45678.90,
            tasks_completed: 328,
            success_rate: 94.5,
        }
    }
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync_or_panic();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Complex UI Demo - Multi-Panel Showcase".to_string(),
                size: Some(WinitPhysicalSize::new(1600.0, 900.0)),
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
        let size = window.physical_size();

        let mut ui = UiSystem::new(graphics_ctx.clone());
        ui.set_viewport(window.viewport());

        let stats = AppStats::default();

        // Build initial UI
        build_complex_ui(&mut ui, size.width as f32, size.height as f32, stats);

        println!("\n═══════════════════════════════════════════════════════");
        println!("  📊 COMPLEX UI DEMO - Multi-Panel Application");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  FEATURES:");
        println!("    • Multi-column layout with sidebar navigation");
        println!("    • Real-time dashboard with statistics cards");
        println!("    • Interactive forms with text inputs");
        println!("    • Activity feed with recent events");
        println!("    • Nested containers and flexbox layouts");
        println!("\n  This showcases full UI composition capabilities!");
        println!("  Resize the window to see responsive layout updates.");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Complex UI demo initialized");

        Box::new(ComplexUiApp {
            window,
            window_id,
            ui,
            stats,
        })
    });
}

fn build_complex_ui(ui: &mut UiSystem, width: f32, height: f32, stats: AppStats) {
    ui.build(|root| {
        // Main container - full screen
        root.container()
            .width(width)
            .height(height)
            .background_color(Color::from_rgb_u8(18, 18, 25))
            .child(|root| {
                // Horizontal layout - sidebar + main content
                root.row()
                    .child(|root| {
                        // Left sidebar - navigation
                        root.container()
                            .width(250.0)
                            .background_color(Color::from_rgb_u8(25, 25, 35))
                            .border_color(Color::from_rgb_u8(50, 50, 70))
                            .border_width(1.0)
                            .padding(20.0)
                            .child(|root| {
                                root.column()
                                    .gap(20.0)
                                    .child(|root| {
                                        // Logo/Title area
                                        root.container()
                                            .padding(10.0)
                                            .child(|root| {
                                                root.text("Astrelis")
                                                    .size(28.0)
                                                    .color(Color::from_rgb_u8(100, 180, 255))
                                                    .bold()
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .child(|root| {
                                        // Navigation menu
                                        root.column()
                                            .gap(8.0)
                                            .child(|root| {
                                                root.button("Dashboard")
                                                    .background_color(Color::from_rgb_u8(60, 120, 200))
                                                    .hover_color(Color::from_rgb_u8(70, 130, 210))
                                                    .padding(12.0)
                                                    .font_size(14.0)
                                                    .min_width(200.0)
                                                    .build()
                                            })
                                            .child(|root| {
                                                root.button("Analytics")
                                                    .background_color(Color::from_rgb_u8(45, 45, 65))
                                                    .hover_color(Color::from_rgb_u8(55, 55, 75))
                                                    .padding(12.0)
                                                    .font_size(14.0)
                                                    .min_width(200.0)
                                                    .build()
                                            })
                                            .child(|root| {
                                                root.button("Settings")
                                                    .background_color(Color::from_rgb_u8(45, 45, 65))
                                                    .hover_color(Color::from_rgb_u8(55, 55, 75))
                                                    .padding(12.0)
                                                    .font_size(14.0)
                                                    .min_width(200.0)
                                                    .build()
                                            })
                                            .child(|root| {
                                                root.button("Users")
                                                    .background_color(Color::from_rgb_u8(45, 45, 65))
                                                    .hover_color(Color::from_rgb_u8(55, 55, 75))
                                                    .padding(12.0)
                                                    .font_size(14.0)
                                                    .min_width(200.0)
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .child(|root| {
                                        // Footer in sidebar
                                        root.container()
                                            .margin(20.0)
                                            .child(|root| {
                                                root.text("Version 0.1.0")
                                                    .size(11.0)
                                                    .color(Color::from_rgb_u8(100, 100, 120))
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .child(|root| {
                        // Main content area
                        root.container()
                            .background_color(Color::from_rgb_u8(22, 22, 32))
                            .padding(30.0)
                            .child(|root| {
                                root.column()
                                    .gap(25.0)
                                    .child(|root| {
                                        // Header
                                        root.container()
                                            .child(|root| {
                                                root.column()
                                                    .gap(8.0)
                                                    .child(|root| {
                                                        root.text("Dashboard Overview")
                                                            .size(32.0)
                                                            .color(Color::WHITE)
                                                            .bold()
                                                            .build()
                                                    })
                                                    .child(|root| {
                                                        root.text("Real-time application metrics and controls")
                                                            .size(14.0)
                                                            .color(Color::from_rgb_u8(150, 150, 170))
                                                            .build()
                                                    })
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .child(|root| {
                                        // Statistics cards row
                                        root.row()
                                            .gap(20.0)
                                            .child(|root| {
                                                build_stat_card(root, "Active Users",
                                                    &format!("{}", stats.active_users),
                                                    Color::from_rgb_u8(100, 200, 150))
                                            })
                                            .child(|root| {
                                                build_stat_card(root, "Revenue",
                                                    &format!("${:.2}", stats.total_revenue),
                                                    Color::from_rgb_u8(100, 180, 255))
                                            })
                                            .child(|root| {
                                                build_stat_card(root, "Tasks",
                                                    &format!("{}", stats.tasks_completed),
                                                    Color::from_rgb_u8(255, 180, 100))
                                            })
                                            .child(|root| {
                                                build_stat_card(root, "Success Rate",
                                                    &format!("{:.1}%", stats.success_rate),
                                                    Color::from_rgb_u8(200, 100, 255))
                                            })
                                            .build()
                                    })
                                    .child(|root| {
                                        // Two-column content area
                                        root.row()
                                            .gap(20.0)
                                            .child(|root| {
                                                // Left column - Form
                                                root.container()
                                                    .background_color(Color::from_rgb_u8(30, 30, 45))
                                                    .border_radius(12.0)
                                                    .padding(20.0)
                                                    .min_width(400.0)
                                                    .child(|root| {
                                                        root.column()
                                                            .gap(15.0)
                                                            .child(|root| {
                                                                root.text("Create New Task")
                                                                    .size(20.0)
                                                                    .color(Color::WHITE)
                                                                    .bold()
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text("Task Name")
                                                                    .size(12.0)
                                                                    .color(Color::from_rgb_u8(180, 180, 200))
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text_input("Enter task name...")
                                                                    .padding(10.0)
                                                                    .min_width(350.0)
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text("Description")
                                                                    .size(12.0)
                                                                    .color(Color::from_rgb_u8(180, 180, 200))
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text_input("Enter description...")
                                                                    .padding(10.0)
                                                                    .min_width(350.0)
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                // Form buttons
                                                                root.row()
                                                                    .gap(10.0)
                                                                    .child(|root| {
                                                                        root.button("Create Task")
                                                                            .background_color(Color::from_rgb_u8(60, 180, 60))
                                                                            .hover_color(Color::from_rgb_u8(70, 200, 70))
                                                                            .padding(12.0)
                                                                            .font_size(14.0)
                                                                            .build()
                                                                    })
                                                                    .child(|root| {
                                                                        root.button("Cancel")
                                                                            .background_color(Color::from_rgb_u8(100, 100, 120))
                                                                            .hover_color(Color::from_rgb_u8(120, 120, 140))
                                                                            .padding(12.0)
                                                                            .font_size(14.0)
                                                                            .build()
                                                                    })
                                                                    .build()
                                                            })
                                                            .build()
                                                    })
                                                    .build()
                                            })
                                            .child(|root| {
                                                // Right column - Recent activity list
                                                root.container()
                                                    .background_color(Color::from_rgb_u8(30, 30, 45))
                                                    .border_radius(12.0)
                                                    .padding(20.0)
                                                    .child(|root| {
                                                        root.column()
                                                            .gap(12.0)
                                                            .child(|root| {
                                                                root.text("Recent Activity")
                                                                    .size(20.0)
                                                                    .color(Color::WHITE)
                                                                    .bold()
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                build_activity_item(root, "User logged in", "2 minutes ago")
                                                            })
                                                            .child(|root| {
                                                                build_activity_item(root, "Task completed", "15 minutes ago")
                                                            })
                                                            .child(|root| {
                                                                build_activity_item(root, "New user registered", "1 hour ago")
                                                            })
                                                            .child(|root| {
                                                                build_activity_item(root, "System backup completed", "3 hours ago")
                                                            })
                                                            .child(|root| {
                                                                build_activity_item(root, "Database optimized", "6 hours ago")
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
}

fn build_stat_card(root: &mut astrelis_ui::UiBuilder, title: &str, value: &str, color: Color) -> astrelis_ui::NodeId {
    root.container()
        .background_color(Color::from_rgb_u8(35, 35, 50))
        .border_radius(10.0)
        .padding(20.0)
        .min_width(150.0)
        .child(|root| {
            root.column()
                .gap(10.0)
                .child(|root| {
                    root.text(title)
                        .size(12.0)
                        .color(Color::from_rgb_u8(150, 150, 170))
                        .build()
                })
                .child(|root| {
                    root.text(value)
                        .size(26.0)
                        .color(color)
                        .bold()
                        .build()
                })
                .build()
        })
        .build()
}

fn build_activity_item(root: &mut astrelis_ui::UiBuilder, title: &str, time: &str) -> astrelis_ui::NodeId {
    root.container()
        .background_color(Color::from_rgb_u8(40, 40, 60))
        .border_radius(6.0)
        .padding(12.0)
        .child(|root| {
            root.column()
                .gap(4.0)
                .child(|root| {
                    root.text(title)
                        .size(14.0)
                        .color(Color::from_rgb_u8(220, 220, 240))
                        .build()
                })
                .child(|root| {
                    root.text(time)
                        .size(11.0)
                        .color(Color::from_rgb_u8(120, 120, 140))
                        .build()
                })
                .build()
        })
        .build()
}

impl App for ComplexUiApp {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        new_frame();
        self.ui.update(0.016);
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
                build_complex_ui(&mut self.ui, size.width as f32, size.height as f32, self.stats);
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
            Color::from_rgb_u8(18, 18, 25),
            |pass| {
                self.ui.render(pass.descriptor());
            },
        );

        frame.finish();
    }
}
