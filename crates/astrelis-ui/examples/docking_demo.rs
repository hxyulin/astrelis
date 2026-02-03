//! Docking Demo - Demonstrating Resizable Splits and Tabbed Panels
//!
//! This example shows the docking system features:
//! - Resizable horizontal and vertical splits
//! - Tabbed panels with closable tabs
//! - Nested split layouts
//! - Draggable separators
//!
//! Controls:
//! - Drag separators to resize panels
//! - Click tabs to switch between panels
//! - Drag tabs to reorder them
//! - Click X to close tabs
//! - T: Toggle dark/light theme

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow, WindowContextDescriptor, wgpu,
};
use astrelis_ui::Constraint;
use astrelis_ui::{Theme, UiSystem};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{Event, EventBatch, HandleStatus, Key, SystemTheme},
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
};

struct DockingApp {
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
    is_dark: bool,
}

fn main() {
    logging::init();

    // Initialize profiling - connect to puffin_viewer at http://127.0.0.1:8585
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Docking Demo".to_string(),
                size: Some(WinitPhysicalSize::new(1024.0, 768.0)),
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
        )
        .expect("Failed to create renderable window");

        let window_id = window.id();

        // Create UI system
        let mut ui = UiSystem::new(graphics_ctx.clone());
        ui.set_viewport(window.viewport());

        // Build docking layout
        build_docking_ui(&mut ui);

        tracing::info!("Docking demo initialized");
        tracing::info!("Drag separators to resize panels");
        tracing::info!("Click tabs to switch, drag to reorder");
        tracing::info!("Click X to close tabs");
        tracing::info!("Press T to toggle dark/light theme");

        Box::new(DockingApp {
            window,
            window_id,
            ui,
            is_dark: true,
        })
    });
}

fn build_docking_ui(ui: &mut UiSystem) {
    let theme = ui.theme().clone();
    let colors = &theme.colors;

    ui.build(|root| {
        // Main horizontal split: sidebar (left) | content area (right)
        root.hsplit()
            .width(Constraint::Vw(100.0))
            .height(Constraint::Vh(100.0))
            .split_ratio(0.25)
            .first_min_size(150.0)
            .second_min_size(300.0)
            .first(|left| {
                // Left sidebar with tabs
                left.dock_tabs()
                    .tab("Explorer", |t| {
                        t.container()
                            .background_color(colors.surface)
                            .padding(10.0)
                            .child(|c| {
                                c.column()
                                    .gap(8.0)
                                    .child(|c| {
                                        c.text("File Explorer")
                                            .color(colors.text_primary)
                                            .size(14.0)
                                            .bold()
                                            .build()
                                    })
                                    .child(|c| {
                                        c.text("  src/")
                                            .color(colors.text_primary)
                                            .size(12.0)
                                            .build()
                                    })
                                    .child(|c| {
                                        c.text("    main.rs")
                                            .color(colors.text_secondary)
                                            .size(12.0)
                                            .build()
                                    })
                                    .child(|c| {
                                        c.text("    lib.rs")
                                            .color(colors.text_secondary)
                                            .size(12.0)
                                            .build()
                                    })
                                    .child(|c| {
                                        c.text("  Cargo.toml")
                                            .color(colors.text_primary)
                                            .size(12.0)
                                            .build()
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .tab("Search", |t| {
                        t.container()
                            .background_color(colors.surface)
                            .padding(10.0)
                            .child(|c| {
                                c.column()
                                    .gap(8.0)
                                    .child(|c| {
                                        c.text("Search")
                                            .color(colors.text_primary)
                                            .size(14.0)
                                            .bold()
                                            .build()
                                    })
                                    .child(|c| {
                                        c.text("Type to search files...")
                                            .color(colors.text_secondary)
                                            .size(12.0)
                                            .build()
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .tab("Git", |t| {
                        t.container()
                            .background_color(colors.surface)
                            .padding(10.0)
                            .child(|c| {
                                c.column()
                                    .gap(8.0)
                                    .child(|c| {
                                        c.text("Source Control")
                                            .color(colors.text_primary)
                                            .size(14.0)
                                            .bold()
                                            .build()
                                    })
                                    // KEEP: Git status colors are content-specific
                                    .child(|c| {
                                        c.text("  main (2 changes)")
                                            .color(Color::from_rgb_u8(255, 200, 100))
                                            .size(12.0)
                                            .build()
                                    })
                                    .child(|c| {
                                        c.text("  M src/main.rs")
                                            .color(Color::from_rgb_u8(100, 200, 255))
                                            .size(12.0)
                                            .build()
                                    })
                                    .child(|c| {
                                        c.text("  A src/new.rs")
                                            .color(Color::from_rgb_u8(100, 255, 100))
                                            .size(12.0)
                                            .build()
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .tab("Debug", |t| {
                        t.container()
                            .background_color(colors.surface)
                            .padding(10.0)
                            .child(|c| {
                                c.column()
                                    .gap(8.0)
                                    .child(|c| {
                                        c.text("Debug")
                                            .color(colors.text_primary)
                                            .size(14.0)
                                            .bold()
                                            .build()
                                    })
                                    .child(|c| {
                                        c.text("No active debug session")
                                            .color(colors.text_secondary)
                                            .size(12.0)
                                            .build()
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .closable(true)
                    .build()
            })
            .second(|right| {
                // Right area: vertical split for editor (top) | terminal (bottom)
                right.vsplit()
                    .split_ratio(0.7)
                    .first_min_size(100.0)
                    .second_min_size(80.0)
                    .first(|top| {
                        // Editor area with tabs
                        top.dock_tabs()
                            .tab("main.rs", |t| {
                                t.container()
                                    .background_color(colors.background)
                                    .padding(15.0)
                                    .child(|c| {
                                        // KEEP: Syntax highlighting colors are content-specific
                                        c.column()
                                            .gap(4.0)
                                            .child(|c| {
                                                c.text("fn main() {")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("    println!(\"Hello, world!\");")
                                                    .color(Color::from_rgb_u8(150, 200, 150))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("}")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .tab("lib.rs", |t| {
                                t.container()
                                    .background_color(colors.background)
                                    .padding(15.0)
                                    .child(|c| {
                                        // KEEP: Syntax highlighting colors are content-specific
                                        c.column()
                                            .gap(4.0)
                                            .child(|c| {
                                                c.text("//! Library crate")
                                                    .color(Color::from_rgb_u8(120, 120, 120))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("pub fn add(a: i32, b: i32) -> i32 {")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("    a + b")
                                                    .color(Color::from_rgb_u8(180, 180, 255))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("}")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .tab("Cargo.toml", |t| {
                                t.container()
                                    .background_color(colors.background)
                                    .padding(15.0)
                                    .child(|c| {
                                        // KEEP: Syntax highlighting colors are content-specific
                                        c.column()
                                            .gap(4.0)
                                            .child(|c| {
                                                c.text("[package]")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("name = \"my-app\"")
                                                    .color(Color::from_rgb_u8(150, 200, 150))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("version = \"0.1.0\"")
                                                    .color(Color::from_rgb_u8(150, 200, 150))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .tab("README.md", |t| {
                                t.container()
                                    .background_color(colors.background)
                                    .padding(15.0)
                                    .child(|c| {
                                        // KEEP: Syntax highlighting colors are content-specific
                                        c.column()
                                            .gap(4.0)
                                            .child(|c| {
                                                c.text("# My App")
                                                    .color(Color::from_rgb_u8(100, 180, 255))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("A sample Rust application.")
                                                    .color(Color::from_rgb_u8(180, 180, 180))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .tab("config.yaml", |t| {
                                t.container()
                                    .background_color(colors.background)
                                    .padding(15.0)
                                    .child(|c| {
                                        // KEEP: Syntax highlighting colors are content-specific
                                        c.column()
                                            .gap(4.0)
                                            .child(|c| {
                                                c.text("debug: true")
                                                    .color(Color::from_rgb_u8(150, 200, 150))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("port: 8080")
                                                    .color(Color::from_rgb_u8(150, 200, 150))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .tab("tests.rs", |t| {
                                t.container()
                                    .background_color(colors.background)
                                    .padding(15.0)
                                    .child(|c| {
                                        // KEEP: Syntax highlighting colors are content-specific
                                        c.column()
                                            .gap(4.0)
                                            .child(|c| {
                                                c.text("#[cfg(test)]")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("mod tests {")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("    #[test]")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("    fn it_works() { }")
                                                    .color(Color::from_rgb_u8(180, 180, 255))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("}")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .tab("build.rs", |t| {
                                t.container()
                                    .background_color(colors.background)
                                    .padding(15.0)
                                    .child(|c| {
                                        // KEEP: Syntax highlighting colors are content-specific
                                        c.column()
                                            .gap(4.0)
                                            .child(|c| {
                                                c.text("fn main() {")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("    println!(\"cargo:rerun-if-changed=build.rs\");")
                                                    .color(Color::from_rgb_u8(150, 200, 150))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("}")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .tab("utils.rs", |t| {
                                t.container()
                                    .background_color(colors.background)
                                    .padding(15.0)
                                    .child(|c| {
                                        // KEEP: Syntax highlighting colors are content-specific
                                        c.column()
                                            .gap(4.0)
                                            .child(|c| {
                                                c.text("pub fn clamp(val: f32, min: f32, max: f32) -> f32 {")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("    val.max(min).min(max)")
                                                    .color(Color::from_rgb_u8(180, 180, 255))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("}")
                                                    .color(Color::from_rgb_u8(200, 150, 100))
                                                    .size(13.0)
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .closable(true)
                            .build()
                    })
                    .second(|bottom| {
                        // Bottom panel with tabs for output/terminal
                        bottom.dock_tabs()
                            .tab("Terminal", |t| {
                                t.container()
                                    .background_color(colors.background)
                                    .padding(10.0)
                                    .child(|c| {
                                        // KEEP: Terminal colors are content-specific
                                        c.column()
                                            .gap(2.0)
                                            .child(|c| {
                                                c.text("$ cargo build")
                                                    .color(Color::from_rgb_u8(100, 200, 100))
                                                    .size(12.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("   Compiling my-app v0.1.0")
                                                    .color(Color::from_rgb_u8(180, 180, 180))
                                                    .size(12.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("    Finished dev [unoptimized + debuginfo]")
                                                    .color(Color::from_rgb_u8(100, 200, 100))
                                                    .size(12.0)
                                                    .build()
                                            })
                                            .child(|c| {
                                                c.text("$")
                                                    .color(Color::from_rgb_u8(100, 200, 100))
                                                    .size(12.0)
                                                    .build()
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .tab("Problems", |t| {
                                t.container()
                                    .background_color(colors.background)
                                    .padding(10.0)
                                    .child(|c| {
                                        // KEEP: Terminal success color is content-specific
                                        c.text("No problems detected")
                                            .color(Color::from_rgb_u8(100, 200, 100))
                                            .size(12.0)
                                            .build()
                                    })
                                    .build()
                            })
                            .tab("Output", |t| {
                                t.container()
                                    .background_color(colors.background)
                                    .padding(10.0)
                                    .child(|c| {
                                        c.text("Build output will appear here...")
                                            .color(colors.text_secondary)
                                            .size(12.0)
                                            .build()
                                    })
                                    .build()
                            })
                            .closable(true)
                            .build()
                    })
                    .build()
            })
            .build();
    });
}

impl App for DockingApp {
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
                // No rebuild needed: Vw/Vh units auto-resolve on viewport change
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle theme switching
        let mut theme_changed = false;
        events.dispatch(|event| {
            match event {
                Event::ThemeChanged(system_theme) => {
                    self.is_dark = *system_theme == SystemTheme::Dark;
                    theme_changed = true;
                    return HandleStatus::consumed();
                }
                Event::KeyInput(key) => {
                    if key.state == astrelis_winit::event::ElementState::Pressed {
                        if let Key::Character(ref c) = key.logical_key {
                            if c.as_str() == "t" || c.as_str() == "T" {
                                self.is_dark = !self.is_dark;
                                theme_changed = true;
                                return HandleStatus::consumed();
                            }
                        }
                    }
                }
                _ => {}
            }
            HandleStatus::ignored()
        });

        if theme_changed {
            let theme = if self.is_dark {
                Theme::dark()
            } else {
                Theme::light()
            };
            self.ui.set_theme(theme);
            build_docking_ui(&mut self.ui);
        }

        // Handle UI events (drag, clicks, etc.)
        self.ui.handle_events(events);

        // Begin frame and render
        let bg = self.ui.theme().colors.background;
        let mut frame = self.window.begin_drawing();

        frame.clear_and_render(
            RenderTarget::Surface,
            bg,
            |pass| {
                self.ui.render(pass.wgpu_pass());
            },
        );

        frame.finish();
    }
}
