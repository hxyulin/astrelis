//! Widget Gallery - Comprehensive Widget Showcase
//!
//! This example demonstrates ALL available widgets in the Astrelis UI system:
//! - Text (various sizes, colors, styles)
//! - Buttons (different colors, sizes, hover states)
//! - Text Inputs (editable fields with placeholders)
//! - Containers (flexbox layouts with various styles)
//! - Rows and Columns (layout primitives)
//! - Images (texture display)
//! - Tooltips (hover information)
//!
//! This is your one-stop reference for all widget types and their styling options.

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
use std::sync::{Arc, RwLock};

struct WidgetGalleryApp {
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
    counter: Arc<RwLock<i32>>,
    counter_text_id: astrelis_ui::WidgetId,
    input_value_id: astrelis_ui::WidgetId,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Widget Gallery - All UI Components".to_string(),
                size: Some(WinitPhysicalSize::new(1400.0, 900.0)),
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

        let counter = Arc::new(RwLock::new(0));

        // Build initial UI
        let (counter_text_id, input_value_id) = build_gallery_ui(&mut ui, size.width as f32, size.height as f32, counter.clone());

        println!("\n═══════════════════════════════════════════════════════");
        println!("  🎨 WIDGET GALLERY - All UI Components");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  SHOWCASED WIDGETS:");
        println!("    • Text (headings, paragraphs, colored text)");
        println!("    • Buttons (primary, secondary, danger styles)");
        println!("    • Text Inputs (editable fields)");
        println!("    • Containers (layout boxes with styling)");
        println!("    • Rows & Columns (flexbox layouts)");
        println!("    • Images (texture display)");
        println!("    • Tooltips (hover information)");
        println!("\n  CONTROLS:");
        println!("    [+/-] Buttons  Increment/decrement counter");
        println!("\n  Interact with buttons and inputs to see them in action!");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Widget gallery initialized");

        Box::new(WidgetGalleryApp {
            window,
            window_id,
            ui,
            counter,
            counter_text_id,
            input_value_id,
        })
    });
}

fn build_gallery_ui(
    ui: &mut UiSystem,
    width: f32,
    height: f32,
    counter: Arc<RwLock<i32>>,
) -> (astrelis_ui::WidgetId, astrelis_ui::WidgetId) {
    let counter_text_id = astrelis_ui::WidgetId::new("counter_text");
    let input_value_id = astrelis_ui::WidgetId::new("input_value");

    ui.build(|root| {
        root.container()
            .width(width)
            .height(height)
            .padding(30.0)
            .background_color(Color::from_rgb_u8(18, 18, 25))
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
                                        root.text("Widget Gallery")
                                            .size(36.0)
                                            .color(Color::WHITE)
                                            .bold()
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("A comprehensive showcase of all available UI widgets")
                                            .size(14.0)
                                            .color(Color::from_rgb_u8(150, 150, 170))
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
                                // Left column
                                root.column()
                                    .gap(20.0)
                                    .child(|root| {
                                        build_text_section(root)
                                    })
                                    .child(|root| {
                                        build_button_section(root, counter, counter_text_id)
                                    })
                                    .child(|root| {
                                        build_input_section(root)
                                    })
                                    .build()
                            })
                            .child(|root| {
                                // Right column
                                root.column()
                                    .gap(20.0)
                                    .child(|root| {
                                        build_container_section(root)
                                    })
                                    .child(|root| {
                                        build_layout_section(root)
                                    })
                                    .child(|root| {
                                        build_tooltip_section(root)
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .build()
            })
            .build();
    });

    (counter_text_id, input_value_id)
}

fn build_text_section(root: &mut astrelis_ui::UiBuilder) -> astrelis_ui::NodeId {
    root.container()
        .background_color(Color::from_rgb_u8(30, 30, 45))
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Text Widgets")
                        .size(24.0)
                        .color(Color::from_rgb_u8(100, 180, 255))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Large Heading")
                        .size(28.0)
                        .color(Color::WHITE)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Medium Heading")
                        .size(20.0)
                        .color(Color::from_rgb_u8(220, 220, 240))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Regular body text with default styling")
                        .size(14.0)
                        .color(Color::from_rgb_u8(200, 200, 220))
                        .build()
                })
                .child(|root| {
                    root.text("Small caption text for less important info")
                        .size(12.0)
                        .color(Color::from_rgb_u8(150, 150, 170))
                        .build()
                })
                .child(|root| {
                    root.row()
                        .gap(10.0)
                        .child(|root| {
                            root.text("Colored:")
                                .size(14.0)
                                .color(Color::from_rgb_u8(200, 200, 220))
                                .build()
                        })
                        .child(|root| {
                            root.text("Red")
                                .size(14.0)
                                .color(Color::from_rgb_u8(255, 100, 100))
                                .build()
                        })
                        .child(|root| {
                            root.text("Green")
                                .size(14.0)
                                .color(Color::from_rgb_u8(100, 255, 100))
                                .build()
                        })
                        .child(|root| {
                            root.text("Blue")
                                .size(14.0)
                                .color(Color::from_rgb_u8(100, 180, 255))
                                .build()
                        })
                        .build()
                })
                .build()
        })
        .build()
}

fn build_button_section(
    root: &mut astrelis_ui::UiBuilder,
    counter: Arc<RwLock<i32>>,
    counter_text_id: astrelis_ui::WidgetId,
) -> astrelis_ui::NodeId {
    let counter_value = *counter.read().unwrap();
    root.container()
        .background_color(Color::from_rgb_u8(30, 30, 45))
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Button Widgets")
                        .size(24.0)
                        .color(Color::from_rgb_u8(100, 180, 255))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Primary Buttons")
                        .size(14.0)
                        .color(Color::from_rgb_u8(180, 180, 200))
                        .build()
                })
                .child(|root| {
                    root.row()
                        .gap(10.0)
                        .child(|root| {
                            root.button("Primary")
                                .background_color(Color::from_rgb_u8(60, 120, 200))
                                .hover_color(Color::from_rgb_u8(70, 130, 210))
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .child(|root| {
                            root.button("Secondary")
                                .background_color(Color::from_rgb_u8(80, 80, 100))
                                .hover_color(Color::from_rgb_u8(90, 90, 110))
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.text("Success / Danger Buttons")
                        .size(14.0)
                        .color(Color::from_rgb_u8(180, 180, 200))
                        .build()
                })
                .child(|root| {
                    root.row()
                        .gap(10.0)
                        .child(|root| {
                            root.button("Success")
                                .background_color(Color::from_rgb_u8(60, 180, 60))
                                .hover_color(Color::from_rgb_u8(70, 200, 70))
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .child(|root| {
                            root.button("Danger")
                                .background_color(Color::from_rgb_u8(200, 60, 60))
                                .hover_color(Color::from_rgb_u8(220, 70, 70))
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.text("Interactive Counter")
                        .size(14.0)
                        .color(Color::from_rgb_u8(180, 180, 200))
                        .margin(10.0)
                        .build()
                })
                .child(|root| {
                    let counter_dec = counter.clone();
                    let counter_inc = counter.clone();
                    root.container()
                        .background_color(Color::from_rgb_u8(40, 40, 60))
                        .border_radius(8.0)
                        .padding(12.0)
                        .justify_content(taffy::JustifyContent::Center)
                        .align_items(taffy::AlignItems::Center)
                        .child(|root| {
                            root.row()
                                .gap(10.0)
                                .child(|root| {
                                    root.button("-")
                                        .background_color(Color::from_rgb_u8(200, 60, 60))
                                        .hover_color(Color::from_rgb_u8(220, 70, 70))
                                        .padding(8.0)
                                        .min_width(30.0)
                                        .font_size(16.0)
                                        .on_click(move || {
                                            *counter_dec.write().unwrap() -= 1;
                                        })
                                        .build()
                                })
                                .child(|root| {
                                    root.text(format!("Count: {}", counter_value))
                                        .id(counter_text_id)
                                        .size(16.0)
                                        .color(Color::from_rgb_u8(100, 200, 255))
                                        .bold()
                                        .build()
                                })
                                .child(|root| {
                                    root.button("+")
                                        .background_color(Color::from_rgb_u8(60, 180, 60))
                                        .hover_color(Color::from_rgb_u8(70, 200, 70))
                                        .padding(8.0)
                                        .min_width(30.0)
                                        .font_size(16.0)
                                        .on_click(move || {
                                            *counter_inc.write().unwrap() += 1;
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
}

fn build_input_section(root: &mut astrelis_ui::UiBuilder) -> astrelis_ui::NodeId {
    root.container()
        .background_color(Color::from_rgb_u8(30, 30, 45))
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Text Input Widgets")
                        .size(24.0)
                        .color(Color::from_rgb_u8(100, 180, 255))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Username")
                        .size(12.0)
                        .color(Color::from_rgb_u8(180, 180, 200))
                        .build()
                })
                .child(|root| {
                    root.text_input("Enter your username...")
                        .padding(10.0)
                        .min_width(300.0)
                        .build()
                })
                .child(|root| {
                    root.text("Email")
                        .size(12.0)
                        .color(Color::from_rgb_u8(180, 180, 200))
                        .build()
                })
                .child(|root| {
                    root.text_input("user@example.com")
                        .padding(10.0)
                        .min_width(300.0)
                        .build()
                })
                .child(|root| {
                    root.text("Message")
                        .size(12.0)
                        .color(Color::from_rgb_u8(180, 180, 200))
                        .build()
                })
                .child(|root| {
                    root.text_input("Type a message...")
                        .padding(10.0)
                        .min_width(300.0)
                        .build()
                })
                .build()
        })
        .build()
}

fn build_container_section(root: &mut astrelis_ui::UiBuilder) -> astrelis_ui::NodeId {
    root.container()
        .background_color(Color::from_rgb_u8(30, 30, 45))
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Container Widgets")
                        .size(24.0)
                        .color(Color::from_rgb_u8(100, 180, 255))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Styled Containers")
                        .size(14.0)
                        .color(Color::from_rgb_u8(180, 180, 200))
                        .build()
                })
                .child(|root| {
                    root.container()
                        .background_color(Color::from_rgb_u8(50, 50, 70))
                        .border_radius(8.0)
                        .padding(15.0)
                        .child(|root| {
                            root.text("Box with rounded corners and padding")
                                .size(13.0)
                                .color(Color::from_rgb_u8(200, 200, 220))
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.container()
                        .background_color(Color::from_rgb_u8(40, 60, 50))
                        .border_color(Color::from_rgb_u8(100, 200, 150))
                        .border_width(2.0)
                        .border_radius(8.0)
                        .padding(15.0)
                        .child(|root| {
                            root.text("Box with border and custom color")
                                .size(13.0)
                                .color(Color::from_rgb_u8(150, 255, 200))
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.container()
                        .background_color(Color::from_rgb_u8(60, 40, 50))
                        .border_color(Color::from_rgb_u8(255, 100, 150))
                        .border_width(3.0)
                        .border_radius(12.0)
                        .padding(15.0)
                        .child(|root| {
                            root.text("Box with thick border and large radius")
                                .size(13.0)
                                .color(Color::from_rgb_u8(255, 180, 200))
                                .build()
                        })
                        .build()
                })
                .build()
        })
        .build()
}

fn build_layout_section(root: &mut astrelis_ui::UiBuilder) -> astrelis_ui::NodeId {
    root.container()
        .background_color(Color::from_rgb_u8(30, 30, 45))
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Layout Widgets")
                        .size(24.0)
                        .color(Color::from_rgb_u8(100, 180, 255))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Row Layout (Horizontal)")
                        .size(14.0)
                        .color(Color::from_rgb_u8(180, 180, 200))
                        .build()
                })
                .child(|root| {
                    root.row()
                        .gap(10.0)
                        .child(|root| {
                            root.container()
                                .background_color(Color::from_rgb_u8(200, 100, 100))
                                .width(60.0)
                                .height(60.0)
                                .border_radius(6.0)
                                .build()
                        })
                        .child(|root| {
                            root.container()
                                .background_color(Color::from_rgb_u8(100, 200, 100))
                                .width(60.0)
                                .height(60.0)
                                .border_radius(6.0)
                                .build()
                        })
                        .child(|root| {
                            root.container()
                                .background_color(Color::from_rgb_u8(100, 100, 200))
                                .width(60.0)
                                .height(60.0)
                                .border_radius(6.0)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.text("Column Layout (Vertical)")
                        .size(14.0)
                        .color(Color::from_rgb_u8(180, 180, 200))
                        .margin(10.0)
                        .build()
                })
                .child(|root| {
                    root.column()
                        .gap(8.0)
                        .child(|root| {
                            root.container()
                                .background_color(Color::from_rgb_u8(255, 180, 100))
                                .width(150.0)
                                .height(30.0)
                                .border_radius(6.0)
                                .build()
                        })
                        .child(|root| {
                            root.container()
                                .background_color(Color::from_rgb_u8(180, 100, 255))
                                .width(150.0)
                                .height(30.0)
                                .border_radius(6.0)
                                .build()
                        })
                        .child(|root| {
                            root.container()
                                .background_color(Color::from_rgb_u8(100, 255, 255))
                                .width(150.0)
                                .height(30.0)
                                .border_radius(6.0)
                                .build()
                        })
                        .build()
                })
                .build()
        })
        .build()
}

fn build_tooltip_section(root: &mut astrelis_ui::UiBuilder) -> astrelis_ui::NodeId {
    root.container()
        .background_color(Color::from_rgb_u8(30, 30, 45))
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Tooltips & Overlays")
                        .size(20.0)
                        .color(Color::from_rgb_u8(100, 200, 255))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Hover Information")
                        .size(14.0)
                        .color(Color::from_rgb_u8(180, 180, 200))
                        .margin(10.0)
                        .build()
                })
                .child(|root| {
                    // Tooltip demonstration (API only - hover functionality in development)
                    root.row()
                        .gap(10.0)
                        .child(|root| {
                            root.tooltip("This is a tooltip with helpful information")
                                .font_size(12.0)
                                .text_color(Color::WHITE)
                                .background_color(Color::from_rgb_u8(40, 40, 60))
                                .padding(8.0)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.text("Note: Tooltip API is implemented. Hover-triggered display is in development.")
                        .size(11.0)
                        .color(Color::from_rgb_u8(150, 150, 100))
                        .margin(10.0)
                        .build()
                })
                .child(|root| {
                    root.text("Future: Tooltips will show on hover with customizable delay and positioning.")
                        .size(11.0)
                        .color(Color::from_rgb_u8(120, 120, 140))
                        .margin(10.0)
                        .build()
                })
                .build()
        })
        .build()
}

impl App for WidgetGalleryApp {
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
                let (counter_text_id, input_value_id) = build_gallery_ui(
                    &mut self.ui,
                    size.width as f32,
                    size.height as f32,
                    self.counter.clone(),
                );
                self.counter_text_id = counter_text_id;
                self.input_value_id = input_value_id;
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle UI events (button clicks are handled by callbacks)
        self.ui.handle_events(events);

        // Update counter text every frame
        let counter_value = *self.counter.read().unwrap();
        self.ui.update_text(self.counter_text_id, format!("Count: {}", counter_value));

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
