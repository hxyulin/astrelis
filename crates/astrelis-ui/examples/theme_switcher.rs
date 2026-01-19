//! Theme Switcher - Light/Dark Theme Demo
//!
//! This example demonstrates the theme system with runtime theme switching:
//! - Dark theme (default)
//! - Light theme
//! - Custom theme colors
//! - Consistent color roles across widgets
//! - Theme-aware component styling
//!
//! **Keyboard Controls:**
//! - **T**: Toggle between light/dark themes
//! - **1**: Switch to dark theme
//! - **2**: Switch to light theme
//! - **3**: Switch to custom theme

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow,
    WindowContextDescriptor, wgpu,
};
use astrelis_ui::{UiSystem, Theme, ColorPalette};
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::{EventBatch, Event, HandleStatus, Key},
    window::{PhysicalSize, WindowBackend, WindowDescriptor},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ThemeMode {
    Dark,
    Light,
    Custom,
}

struct ThemeSwitcherApp {
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
    current_theme: ThemeMode,
    theme: Theme,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Theme Switcher - Light/Dark Mode Demo".to_string(),
                size: Some(PhysicalSize::new(1200.0, 800.0)),
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

        let theme = Theme::dark();
        build_themed_ui(&mut ui, size.width as f32, size.height as f32, &theme, ThemeMode::Dark);

        println!("\n═══════════════════════════════════════════════════════");
        println!("  🎨 THEME SWITCHER - Light/Dark Mode Demo");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  CONTROLS:");
        println!("    [T]  Toggle between light/dark themes");
        println!("    [1]  Dark theme (default)");
        println!("    [2]  Light theme");
        println!("    [3]  Custom theme (purple/teal)");
        println!("\n  Notice how all colors update consistently!");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Theme switcher initialized with dark theme");

        Box::new(ThemeSwitcherApp {
            window,
            window_id,
            ui,
            current_theme: ThemeMode::Dark,
            theme,
        })
    });
}

fn build_themed_ui(
    ui: &mut UiSystem,
    width: f32,
    height: f32,
    theme: &Theme,
    current_mode: ThemeMode,
) {
    let bg = theme.colors.background;
    let surface = theme.colors.surface;
    let primary = theme.colors.primary;
    let secondary = theme.colors.secondary;
    let text_primary = theme.colors.text_primary;
    let text_secondary = theme.colors.text_secondary;
    let success = theme.colors.success;
    let warning = theme.colors.warning;
    let error = theme.colors.error;

    ui.build(|root| {
        root.container()
            .width(width)
            .height(height)
            .padding(30.0)
            .background_color(bg)
            .child(|root| {
                root.column()
                    .gap(25.0)
                    .child(|root| {
                        // Header
                        root.container()
                            .background_color(surface)
                            .border_radius(12.0)
                            .padding(20.0)
                            .child(|root| {
                                root.column()
                                    .gap(10.0)
                                    .child(|root| {
                                        root.text("Theme Switcher")
                                            .size(36.0)
                                            .color(text_primary)
                                            .bold()
                                            .build()
                                    })
                                    .child(|root| {
                                        let theme_name = match current_mode {
                                            ThemeMode::Dark => "Dark Theme",
                                            ThemeMode::Light => "Light Theme",
                                            ThemeMode::Custom => "Custom Theme",
                                        };
                                        root.text(format!("Current: {}", theme_name))
                                            .size(16.0)
                                            .color(primary)
                                            .bold()
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("Press T to toggle themes, or 1/2/3 for specific themes")
                                            .size(14.0)
                                            .color(text_secondary)
                                            .build()
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .child(|root| {
                        // Two-column layout
                        root.row()
                            .gap(25.0)
                            .child(|root| {
                                // Left column - Color showcase
                                root.column()
                                    .gap(20.0)
                                    .child(|root| {
                                        build_color_section(root, "Primary Colors", theme, primary, secondary)
                                    })
                                    .child(|root| {
                                        build_status_colors_section(root, theme, success, warning, error)
                                    })
                                    .build()
                            })
                            .child(|root| {
                                // Right column - Themed components
                                root.column()
                                    .gap(20.0)
                                    .child(|root| {
                                        build_buttons_section(root, theme)
                                    })
                                    .child(|root| {
                                        build_cards_section(root, theme)
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

fn build_color_section(
    root: &mut astrelis_ui::UiBuilder,
    title: &str,
    theme: &Theme,
    primary: Color,
    secondary: Color,
) -> astrelis_ui::NodeId {
    root.container()
        .background_color(theme.colors.surface)
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text(title)
                        .size(24.0)
                        .color(theme.colors.text_primary)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Primary")
                        .size(14.0)
                        .color(theme.colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    root.container()
                        .background_color(primary)
                        .border_radius(8.0)
                        .width(200.0)
                        .height(60.0)
                        .build()
                })
                .child(|root| {
                    root.text("Secondary")
                        .size(14.0)
                        .color(theme.colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    root.container()
                        .background_color(secondary)
                        .border_radius(8.0)
                        .width(200.0)
                        .height(60.0)
                        .build()
                })
                .build()
        })
        .build()
}

fn build_status_colors_section(
    root: &mut astrelis_ui::UiBuilder,
    theme: &Theme,
    success: Color,
    warning: Color,
    error: Color,
) -> astrelis_ui::NodeId {
    root.container()
        .background_color(theme.colors.surface)
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Status Colors")
                        .size(24.0)
                        .color(theme.colors.text_primary)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.row()
                        .gap(10.0)
                        .child(|root| {
                            root.column()
                                .gap(8.0)
                                .child(|root| {
                                    root.text("Success")
                                        .size(12.0)
                                        .color(theme.colors.text_secondary)
                                        .build()
                                })
                                .child(|root| {
                                    root.container()
                                        .background_color(success)
                                        .border_radius(6.0)
                                        .width(60.0)
                                        .height(60.0)
                                        .build()
                                })
                                .build()
                        })
                        .child(|root| {
                            root.column()
                                .gap(8.0)
                                .child(|root| {
                                    root.text("Warning")
                                        .size(12.0)
                                        .color(theme.colors.text_secondary)
                                        .build()
                                })
                                .child(|root| {
                                    root.container()
                                        .background_color(warning)
                                        .border_radius(6.0)
                                        .width(60.0)
                                        .height(60.0)
                                        .build()
                                })
                                .build()
                        })
                        .child(|root| {
                            root.column()
                                .gap(8.0)
                                .child(|root| {
                                    root.text("Error")
                                        .size(12.0)
                                        .color(theme.colors.text_secondary)
                                        .build()
                                })
                                .child(|root| {
                                    root.container()
                                        .background_color(error)
                                        .border_radius(6.0)
                                        .width(60.0)
                                        .height(60.0)
                                        .build()
                                })
                                .build()
                        })
                        .build()
                })
                .build()
        })
        .build()
}

fn build_buttons_section(root: &mut astrelis_ui::UiBuilder, theme: &Theme) -> astrelis_ui::NodeId {
    root.container()
        .background_color(theme.colors.surface)
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Themed Buttons")
                        .size(24.0)
                        .color(theme.colors.text_primary)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.row()
                        .gap(10.0)
                        .child(|root| {
                            root.button("Primary")
                                .background_color(theme.colors.primary)
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .child(|root| {
                            root.button("Secondary")
                                .background_color(theme.colors.secondary)
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.row()
                        .gap(10.0)
                        .child(|root| {
                            root.button("Success")
                                .background_color(theme.colors.success)
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .child(|root| {
                            root.button("Warning")
                                .background_color(theme.colors.warning)
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .child(|root| {
                            root.button("Error")
                                .background_color(theme.colors.error)
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .build()
                })
                .build()
        })
        .build()
}

fn build_cards_section(root: &mut astrelis_ui::UiBuilder, theme: &Theme) -> astrelis_ui::NodeId {
    root.container()
        .background_color(theme.colors.surface)
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Themed Cards")
                        .size(24.0)
                        .color(theme.colors.text_primary)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.container()
                        .background_color(theme.colors.background)
                        .border_color(theme.colors.border)
                        .border_width(1.0)
                        .border_radius(8.0)
                        .padding(15.0)
                        .child(|root| {
                            root.column()
                                .gap(8.0)
                                .child(|root| {
                                    root.text("Information Card")
                                        .size(16.0)
                                        .color(theme.colors.text_primary)
                                        .bold()
                                        .build()
                                })
                                .child(|root| {
                                    root.text("This card adapts to the current theme")
                                        .size(13.0)
                                        .color(theme.colors.text_secondary)
                                        .build()
                                })
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.container()
                        .background_color(theme.colors.background)
                        .border_color(theme.colors.primary)
                        .border_width(2.0)
                        .border_radius(8.0)
                        .padding(15.0)
                        .child(|root| {
                            root.column()
                                .gap(8.0)
                                .child(|root| {
                                    root.text("Highlighted Card")
                                        .size(16.0)
                                        .color(theme.colors.primary)
                                        .bold()
                                        .build()
                                })
                                .child(|root| {
                                    root.text("Uses primary color for emphasis")
                                        .size(13.0)
                                        .color(theme.colors.text_secondary)
                                        .build()
                                })
                                .build()
                        })
                        .build()
                })
                .build()
        })
        .build()
}

fn create_custom_theme() -> Theme {
    Theme {
        colors: ColorPalette {
            primary: Color::from_rgb_u8(138, 80, 255),      // Purple
            secondary: Color::from_rgb_u8(80, 200, 200),    // Teal
            background: Color::from_rgb_u8(15, 15, 20),
            surface: Color::from_rgb_u8(25, 25, 35),
            error: Color::from_rgb_u8(255, 80, 120),
            warning: Color::from_rgb_u8(255, 200, 80),
            success: Color::from_rgb_u8(80, 220, 140),
            info: Color::from_rgb_u8(100, 180, 255),
            text_primary: Color::from_rgb_u8(240, 240, 255),
            text_secondary: Color::from_rgb_u8(160, 160, 180),
            text_disabled: Color::from_rgb_u8(100, 100, 120),
            border: Color::from_rgb_u8(70, 70, 90),
            divider: Color::from_rgb_u8(50, 50, 70),
            hover_overlay: Color::from_rgba_u8(255, 255, 255, 20),
            active_overlay: Color::from_rgba_u8(255, 255, 255, 40),
        },
        typography: astrelis_ui::Typography::new(),
        spacing: astrelis_ui::Spacing::new(),
        shapes: astrelis_ui::Shapes::new(),
    }
}

impl App for ThemeSwitcherApp {
    fn update(&mut self, _ctx: &mut AppCtx) {
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
                build_themed_ui(
                    &mut self.ui,
                    size.width as f32,
                    size.height as f32,
                    &self.theme,
                    self.current_theme,
                );
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle keyboard events for theme switching
        let mut theme_changed = false;
        events.dispatch(|event| {
            if let Event::KeyInput(key) = event {
                if key.state == astrelis_winit::event::ElementState::Pressed {
                    match key.logical_key {
                        Key::Character(ref c) if c.as_str() == "t" || c.as_str() == "T" => {
                            // Toggle between dark and light
                            self.current_theme = if self.current_theme == ThemeMode::Dark {
                                ThemeMode::Light
                            } else {
                                ThemeMode::Dark
                            };
                            theme_changed = true;
                            return HandleStatus::consumed();
                        }
                        Key::Character(ref c) if c.as_str() == "1" => {
                            self.current_theme = ThemeMode::Dark;
                            theme_changed = true;
                            return HandleStatus::consumed();
                        }
                        Key::Character(ref c) if c.as_str() == "2" => {
                            self.current_theme = ThemeMode::Light;
                            theme_changed = true;
                            return HandleStatus::consumed();
                        }
                        Key::Character(ref c) if c.as_str() == "3" => {
                            self.current_theme = ThemeMode::Custom;
                            theme_changed = true;
                            return HandleStatus::consumed();
                        }
                        _ => {}
                    }
                }
            }
            HandleStatus::ignored()
        });

        // Apply theme change if needed
        if theme_changed {
            self.theme = match self.current_theme {
                ThemeMode::Dark => Theme::dark(),
                ThemeMode::Light => Theme::light(),
                ThemeMode::Custom => create_custom_theme(),
            };

            let theme_name = match self.current_theme {
                ThemeMode::Dark => "Dark",
                ThemeMode::Light => "Light",
                ThemeMode::Custom => "Custom",
            };
            println!("  🎨 Switched to {} theme", theme_name);
            tracing::info!("Theme switched to: {}", theme_name);

            let size = self.window.inner_size();
            build_themed_ui(
                &mut self.ui,
                size.width as f32,
                size.height as f32,
                &self.theme,
                self.current_theme,
            );
        }

        // Handle UI events
        self.ui.handle_events(events);

        // Begin frame and render
        let mut frame = self.window.begin_drawing();

        frame.clear_and_render(
            RenderTarget::Surface,
            self.theme.colors.background,
            |pass| {
                self.ui.render(pass.descriptor());
            },
        );

        frame.finish();
    }
}
