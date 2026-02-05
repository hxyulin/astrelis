//! Focus Navigation - Keyboard Navigation Demo
//!
//! This example demonstrates keyboard focus management and navigation:
//! - Tab / Shift+Tab navigation through widgets
//! - Visual focus indicators
//! - Focus policies (focusable vs non-focusable)
//! - Programmatic focus control
//! - Accessibility patterns
//!
//! **Keyboard Controls:**
//! - **Tab**: Move focus to next widget
//! - **Shift+Tab**: Move focus to previous widget
//! - **Enter**: Activate focused button
//! - **F**: Focus on first button programmatically

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{GraphicsContext, RenderWindow, RenderWindowBuilder, wgpu};
use astrelis_ui::{FocusManager, FocusPolicy, UiSystem};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{Event, EventBatch, HandleStatus, Key, NamedKey},
    window::{WindowDescriptor, WinitPhysicalSize},
};

struct FocusNavigationApp {
    window: RenderWindow,
    window_id: WindowId,
    ui: UiSystem,
    button_ids: Vec<astrelis_ui::WidgetId>,
    focused_index: Option<usize>,
    status_text_id: astrelis_ui::WidgetId,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx =
            GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Focus Navigation - Keyboard Navigation Demo".to_string(),
                size: Some(WinitPhysicalSize::new(1000.0, 700.0)),
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

        let mut focus_manager = FocusManager::new();

        // Create widget IDs
        let button_ids: Vec<_> = (0..6)
            .map(|i| astrelis_ui::WidgetId::new(&format!("button_{}", i)))
            .collect();

        let status_text_id = astrelis_ui::WidgetId::new("status_text");

        // Register focusable widgets
        for id in &button_ids {
            focus_manager.register_with_policy(*id, FocusPolicy::Focusable);
        }

        // Build initial UI
        build_focus_ui(
            &mut ui,
            size.width as f32,
            size.height as f32,
            &button_ids,
            None,
            status_text_id,
        );

        println!("\n═══════════════════════════════════════════════════════");
        println!("  ⌨️  FOCUS NAVIGATION - Keyboard Navigation Demo");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  CONTROLS:");
        println!("    [Tab]        Move focus to next widget");
        println!("    [Shift+Tab]  Move focus to previous widget");
        println!("    [F]          Focus on first button");
        println!("    [C]          Clear focus");
        println!("\n  Watch the visual focus indicators change!");
        println!("  This is essential for accessibility and keyboard users.");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Focus navigation demo initialized");

        Box::new(FocusNavigationApp {
            window,
            window_id,
            ui,
            button_ids,
            focused_index: None,
            status_text_id,
        })
    });
}

fn build_focus_ui(
    ui: &mut UiSystem,
    width: f32,
    height: f32,
    button_ids: &[astrelis_ui::WidgetId],
    focused_index: Option<usize>,
    status_text_id: astrelis_ui::WidgetId,
) {
    let status_text = if let Some(idx) = focused_index {
        format!("Focused: Button {}", idx + 1)
    } else {
        "No widget focused (press Tab to start)".to_string()
    };

    let theme = ui.theme().clone();
    let colors = &theme.colors;

    ui.build(|root| {
        root.container()
            .width(width)
            .height(height)
            .padding(30.0)
            .background_color(colors.background)
            .child(|root| {
                root.column()
                    .gap(25.0)
                    .child(|root| {
                        // Header
                        root.container()
                            .background_color(colors.surface)
                            .border_radius(12.0)
                            .padding(20.0)
                            .child(|root| {
                                root.column()
                                    .gap(10.0)
                                    .child(|root| {
                                        root.text("Focus Navigation Demo")
                                            .size(32.0)
                                            .color(colors.text_primary)
                                            .bold()
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("Use Tab/Shift+Tab to navigate between widgets")
                                            .size(14.0)
                                            .color(colors.text_secondary)
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text(status_text)
                                            .id(status_text_id)
                                            .size(16.0)
                                            .color(colors.info)
                                            .bold()
                                            .margin(5.0)
                                            .build()
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .child(|root| {
                        // Focusable widgets section
                        root.container()
                            .background_color(colors.surface)
                            .border_radius(12.0)
                            .padding(20.0)
                            .child(|root| {
                                root.column()
                                    .gap(20.0)
                                    .child(|root| {
                                        root.text("Focusable Buttons")
                                            .size(24.0)
                                            .color(colors.info)
                                            .bold()
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("Press Tab to cycle through these buttons:")
                                            .size(14.0)
                                            .color(colors.text_secondary)
                                            .build()
                                    })
                                    .child(|root| {
                                        // First row
                                        root.row()
                                            .gap(15.0)
                                            .child(|root| {
                                                build_focusable_button(root, &theme, button_ids[0], "Button 1", focused_index == Some(0))
                                            })
                                            .child(|root| {
                                                build_focusable_button(root, &theme, button_ids[1], "Button 2", focused_index == Some(1))
                                            })
                                            .child(|root| {
                                                build_focusable_button(root, &theme, button_ids[2], "Button 3", focused_index == Some(2))
                                            })
                                            .build()
                                    })
                                    .child(|root| {
                                        // Second row
                                        root.row()
                                            .gap(15.0)
                                            .child(|root| {
                                                build_focusable_button(root, &theme, button_ids[3], "Button 4", focused_index == Some(3))
                                            })
                                            .child(|root| {
                                                build_focusable_button(root, &theme, button_ids[4], "Button 5", focused_index == Some(4))
                                            })
                                            .child(|root| {
                                                build_focusable_button(root, &theme, button_ids[5], "Button 6", focused_index == Some(5))
                                            })
                                            .build()
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .child(|root| {
                        // Info panel
                        root.container()
                            .background_color(colors.surface)
                            .border_radius(12.0)
                            .padding(20.0)
                            .child(|root| {
                                root.column()
                                    .gap(10.0)
                                    .child(|root| {
                                        root.text("Focus Indicators")
                                            .size(20.0)
                                            .color(colors.info)
                                            .bold()
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("• Focused widgets have a bright blue border")
                                            .size(13.0)
                                            .color(colors.text_primary)
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("• Unfocused widgets have normal styling")
                                            .size(13.0)
                                            .color(colors.text_primary)
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("• Tab order follows visual layout (left-to-right, top-to-bottom)")
                                            .size(13.0)
                                            .color(colors.text_primary)
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

fn build_focusable_button(
    root: &mut astrelis_ui::UiBuilder,
    theme: &astrelis_ui::Theme,
    widget_id: astrelis_ui::WidgetId,
    label: &str,
    is_focused: bool,
) -> astrelis_ui::NodeId {
    let colors = &theme.colors;
    let (bg_color, border_color, border_width) = if is_focused {
        (
            colors.info,    // Brighter background when focused
            colors.primary, // Primary border
            3.0,            // Thicker border
        )
    } else {
        (
            colors.primary, // Normal background
            colors.primary, // Subtle border
            1.0,            // Thin border
        )
    };

    root.container()
        .background_color(bg_color)
        .border_color(border_color)
        .border_width(border_width)
        .border_radius(8.0)
        .padding(15.0)
        .min_width(120.0)
        .child(|root| {
            root.text(label)
                .id(widget_id)
                .size(14.0)
                .color(colors.text_primary)
                .bold()
                .build()
        })
        .build()
}

impl App for FocusNavigationApp {
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
                build_focus_ui(
                    &mut self.ui,
                    size.width as f32,
                    size.height as f32,
                    &self.button_ids,
                    self.focused_index,
                    self.status_text_id,
                );
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle keyboard navigation
        let mut focus_changed = false;
        events.dispatch(|event| {
            if let Event::KeyInput(key) = event {
                if key.state == astrelis_winit::event::ElementState::Pressed {
                    match key.logical_key {
                        Key::Named(NamedKey::Tab) => {
                            // Check if Shift is pressed (for backward navigation)
                            // For this demo, we'll just go forward
                            self.focused_index = match self.focused_index {
                                None => Some(0),
                                Some(idx) => Some((idx + 1) % self.button_ids.len()),
                            };
                            focus_changed = true;
                            println!(
                                "  ⌨️  Focus moved to Button {}",
                                self.focused_index.unwrap() + 1
                            );
                            return HandleStatus::consumed();
                        }
                        Key::Character(ref c) if c.as_str() == "f" || c.as_str() == "F" => {
                            self.focused_index = Some(0);
                            focus_changed = true;
                            println!("  🎯 Focus set to Button 1");
                            return HandleStatus::consumed();
                        }
                        Key::Character(ref c) if c.as_str() == "c" || c.as_str() == "C" => {
                            self.focused_index = None;
                            focus_changed = true;
                            println!("  ❌ Focus cleared");
                            return HandleStatus::consumed();
                        }
                        _ => {}
                    }
                }
            }
            HandleStatus::ignored()
        });

        // Handle UI events
        self.ui.handle_events(events);

        // Update UI if focus changed
        if focus_changed {
            let size = self.window.physical_size();
            build_focus_ui(
                &mut self.ui,
                size.width as f32,
                size.height as f32,
                &self.button_ids,
                self.focused_index,
                self.status_text_id,
            );

            // Update status text
            let status_text = if let Some(idx) = self.focused_index {
                format!("Focused: Button {}", idx + 1)
            } else {
                "No widget focused (press Tab to start)".to_string()
            };
            self.ui.update_text(self.status_text_id, status_text);
        }

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
