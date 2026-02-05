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
use astrelis_render::{Color, GraphicsContext, RenderWindow, RenderWindowBuilder, wgpu};
use astrelis_ui::UiSystem;
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{Event, EventBatch, HandleStatus},
    window::{WindowDescriptor, WinitPhysicalSize},
};

struct ComplexUiApp {
    window: RenderWindow,
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
        let graphics_ctx =
            GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Complex UI Demo - Multi-Panel Showcase".to_string(),
                size: Some(WinitPhysicalSize::new(1600.0, 900.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderWindowBuilder::new()
            .color_format(wgpu::TextureFormat::Bgra8UnormSrgb)
            .with_depth_default()
            .build(window, graphics_ctx.clone())
            .expect("Failed to create render window");

        let window_id = window.id();
        let size = window.physical_size();

        let mut ui = UiSystem::from_window(graphics_ctx.clone(), &window);
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
    let theme = ui.theme().clone();
    let colors = &theme.colors;

    ui.build(|root| {
        // Main container - full screen
        root.container()
            .width(width)
            .height(height)
            .background_color(colors.background)
            .child(|root| {
                // Horizontal layout - sidebar + main content
                root.row()
                    .child(|root| {
                        // Left sidebar - navigation
                        root.container()
                            .width(250.0)
                            .background_color(colors.background)
                            .border_color(colors.border)
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
                                                    .color(colors.info)
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
                                                    .background_color(colors.primary)
                                                    .padding(12.0)
                                                    .font_size(14.0)
                                                    .min_width(200.0)
                                                    .build()
                                            })
                                            .child(|root| {
                                                root.button("Analytics")
                                                    .background_color(colors.surface)
                                                    .padding(12.0)
                                                    .font_size(14.0)
                                                    .min_width(200.0)
                                                    .build()
                                            })
                                            .child(|root| {
                                                root.button("Settings")
                                                    .background_color(colors.surface)
                                                    .padding(12.0)
                                                    .font_size(14.0)
                                                    .min_width(200.0)
                                                    .build()
                                            })
                                            .child(|root| {
                                                root.button("Users")
                                                    .background_color(colors.surface)
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
                                                    .color(colors.text_disabled)
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
                            .background_color(colors.background)
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
                                                            .color(colors.text_primary)
                                                            .bold()
                                                            .build()
                                                    })
                                                    .child(|root| {
                                                        root.text("Real-time application metrics and controls")
                                                            .size(14.0)
                                                            .color(colors.text_secondary)
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
                                                build_stat_card(root, &theme, "Active Users",
                                                    &format!("{}", stats.active_users),
                                                    Color::from_rgb_u8(100, 200, 150))
                                            })
                                            .child(|root| {
                                                build_stat_card(root, &theme, "Revenue",
                                                    &format!("${:.2}", stats.total_revenue),
                                                    Color::from_rgb_u8(100, 180, 255))
                                            })
                                            .child(|root| {
                                                build_stat_card(root, &theme, "Tasks",
                                                    &format!("{}", stats.tasks_completed),
                                                    Color::from_rgb_u8(255, 180, 100))
                                            })
                                            .child(|root| {
                                                build_stat_card(root, &theme, "Success Rate",
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
                                                    .background_color(colors.surface)
                                                    .border_radius(12.0)
                                                    .padding(20.0)
                                                    .min_width(400.0)
                                                    .child(|root| {
                                                        root.column()
                                                            .gap(15.0)
                                                            .child(|root| {
                                                                root.text("Create New Task")
                                                                    .size(20.0)
                                                                    .color(colors.text_primary)
                                                                    .bold()
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text("Task Name")
                                                                    .size(12.0)
                                                                    .color(colors.text_secondary)
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
                                                                    .color(colors.text_secondary)
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
                                                                            .background_color(colors.success)
                                                                            .padding(12.0)
                                                                            .font_size(14.0)
                                                                            .build()
                                                                    })
                                                                    .child(|root| {
                                                                        root.button("Cancel")
                                                                            .background_color(colors.surface)
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
                                                    .background_color(colors.surface)
                                                    .border_radius(12.0)
                                                    .padding(20.0)
                                                    .child(|root| {
                                                        root.column()
                                                            .gap(12.0)
                                                            .child(|root| {
                                                                root.text("Recent Activity")
                                                                    .size(20.0)
                                                                    .color(colors.text_primary)
                                                                    .bold()
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                build_activity_item(root, &theme, "User logged in", "2 minutes ago")
                                                            })
                                                            .child(|root| {
                                                                build_activity_item(root, &theme, "Task completed", "15 minutes ago")
                                                            })
                                                            .child(|root| {
                                                                build_activity_item(root, &theme, "New user registered", "1 hour ago")
                                                            })
                                                            .child(|root| {
                                                                build_activity_item(root, &theme, "System backup completed", "3 hours ago")
                                                            })
                                                            .child(|root| {
                                                                build_activity_item(root, &theme, "Database optimized", "6 hours ago")
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

fn build_stat_card(
    root: &mut astrelis_ui::UiBuilder,
    theme: &astrelis_ui::Theme,
    title: &str,
    value: &str,
    color: Color,
) -> astrelis_ui::NodeId {
    let colors = &theme.colors;
    root.container()
        .background_color(colors.surface)
        .border_radius(10.0)
        .padding(20.0)
        .min_width(150.0)
        .child(|root| {
            root.column()
                .gap(10.0)
                .child(|root| {
                    root.text(title)
                        .size(12.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| root.text(value).size(26.0).color(color).bold().build())
                .build()
        })
        .build()
}

fn build_activity_item(
    root: &mut astrelis_ui::UiBuilder,
    theme: &astrelis_ui::Theme,
    title: &str,
    time: &str,
) -> astrelis_ui::NodeId {
    let colors = &theme.colors;
    root.container()
        .background_color(colors.surface)
        .border_radius(6.0)
        .padding(12.0)
        .child(|root| {
            root.column()
                .gap(4.0)
                .child(|root| {
                    root.text(title)
                        .size(14.0)
                        .color(colors.text_primary)
                        .build()
                })
                .child(|root| {
                    root.text(time)
                        .size(11.0)
                        .color(colors.text_secondary)
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
                build_complex_ui(
                    &mut self.ui,
                    size.width as f32,
                    size.height as f32,
                    self.stats,
                );
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle UI events
        self.ui.handle_events(events);

        // Begin frame and render with depth buffer for proper z-ordering
        let bg = self.ui.theme().colors.background;
        let Some(frame) = self.window.begin_frame() else {
            return; // Surface not available
        };

        {
            let mut pass = frame
                .render_pass()
                .clear_color(bg)
                .with_window_depth()
                .clear_depth(0.0)
                .label("UI")
                .build();

            self.ui.render(pass.wgpu_pass());
        }
        // Frame auto-submits on drop
    }
}
