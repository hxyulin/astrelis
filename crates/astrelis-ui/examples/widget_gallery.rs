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
    Color, GraphicsContext, RenderableWindow, WindowContextDescriptor, wgpu,
};
use astrelis_ui::UiSystem;
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{Event, EventBatch, HandleStatus},
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
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
        let graphics_ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

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
        )
        .expect("Failed to create renderable window");

        let window_id = window.id();
        let size = window.physical_size();

        let mut ui = UiSystem::new(graphics_ctx.clone());
        ui.set_viewport(window.viewport());

        let counter = Arc::new(RwLock::new(0));

        // Build initial UI
        let (counter_text_id, input_value_id) = build_gallery_ui(
            &mut ui,
            size.width as f32,
            size.height as f32,
            counter.clone(),
        );

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
                            .child(|root| {
                                root.column()
                                    .gap(8.0)
                                    .child(|root| {
                                        root.text("Widget Gallery")
                                            .size(36.0)
                                            .color(colors.text_primary)
                                            .bold()
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text(
                                            "A comprehensive showcase of all available UI widgets",
                                        )
                                        .size(14.0)
                                        .color(colors.text_secondary)
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
                                    .child(|root| build_text_section(root, &theme))
                                    .child(|root| {
                                        build_button_section(root, &theme, counter, counter_text_id)
                                    })
                                    .child(|root| build_input_section(root, &theme))
                                    .build()
                            })
                            .child(|root| {
                                // Right column
                                root.column()
                                    .gap(20.0)
                                    .child(|root| build_container_section(root, &theme))
                                    .child(|root| build_layout_section(root, &theme))
                                    .child(|root| build_spacing_section(root, &theme))
                                    .child(|root| build_tooltip_section(root, &theme))
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

fn build_text_section(
    root: &mut astrelis_ui::UiBuilder,
    theme: &astrelis_ui::Theme,
) -> astrelis_ui::NodeId {
    let colors = &theme.colors;
    root.container()
        .background_color(colors.surface)
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Text Widgets")
                        .size(24.0)
                        .color(colors.info)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Large Heading")
                        .size(28.0)
                        .color(colors.text_primary)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Medium Heading")
                        .size(20.0)
                        .color(colors.text_primary)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Regular body text with default styling")
                        .size(14.0)
                        .color(colors.text_primary)
                        .build()
                })
                .child(|root| {
                    root.text("Small caption text for less important info")
                        .size(12.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    root.row()
                        .gap(10.0)
                        .child(|root| {
                            root.text("Colored:")
                                .size(14.0)
                                .color(colors.text_primary)
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
                                .color(colors.info)
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
    theme: &astrelis_ui::Theme,
    counter: Arc<RwLock<i32>>,
    counter_text_id: astrelis_ui::WidgetId,
) -> astrelis_ui::NodeId {
    let colors = &theme.colors;
    let counter_value = *counter.read().unwrap();
    root.container()
        .background_color(colors.surface)
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Button Widgets")
                        .size(24.0)
                        .color(colors.info)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Primary Buttons")
                        .size(14.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    root.row()
                        .gap(10.0)
                        .child(|root| {
                            root.button("Primary")
                                .background_color(colors.primary)
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .child(|root| {
                            root.button("Secondary")
                                .background_color(colors.surface)
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.text("Success / Danger Buttons")
                        .size(14.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    root.row()
                        .gap(10.0)
                        .child(|root| {
                            root.button("Success")
                                .background_color(colors.success)
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .child(|root| {
                            root.button("Danger")
                                .background_color(colors.error)
                                .padding(12.0)
                                .font_size(14.0)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.text("Interactive Counter")
                        .size(14.0)
                        .color(colors.text_secondary)
                        .margin(10.0)
                        .build()
                })
                .child(|root| {
                    let counter_dec = counter.clone();
                    let counter_inc = counter.clone();
                    root.container()
                        .background_color(colors.surface)
                        .border_radius(8.0)
                        .padding(12.0)
                        .justify_content(taffy::JustifyContent::Center)
                        .align_items(taffy::AlignItems::Center)
                        .child(|root| {
                            root.row()
                                .gap(10.0)
                                .child(|root| {
                                    root.button("-")
                                        .background_color(colors.error)
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
                                        .color(colors.info)
                                        .bold()
                                        .build()
                                })
                                .child(|root| {
                                    root.button("+")
                                        .background_color(colors.success)
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

fn build_input_section(
    root: &mut astrelis_ui::UiBuilder,
    theme: &astrelis_ui::Theme,
) -> astrelis_ui::NodeId {
    let colors = &theme.colors;
    root.container()
        .background_color(colors.surface)
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Text Input Widgets")
                        .size(24.0)
                        .color(colors.info)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Username")
                        .size(12.0)
                        .color(colors.text_secondary)
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
                        .color(colors.text_secondary)
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
                        .color(colors.text_secondary)
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

fn build_container_section(
    root: &mut astrelis_ui::UiBuilder,
    theme: &astrelis_ui::Theme,
) -> astrelis_ui::NodeId {
    let colors = &theme.colors;
    root.container()
        .background_color(colors.surface)
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Container Widgets")
                        .size(24.0)
                        .color(colors.info)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Styled Containers")
                        .size(14.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    root.container()
                        .background_color(colors.border)
                        .border_radius(8.0)
                        .padding(15.0)
                        .child(|root| {
                            root.text("Box with rounded corners and padding")
                                .size(13.0)
                                .color(colors.text_primary)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    // Success-tinted container
                    root.container()
                        .background_color(colors.surface)
                        .border_color(colors.success)
                        .border_width(2.0)
                        .border_radius(8.0)
                        .padding(15.0)
                        .child(|root| {
                            root.text("Box with border and custom color")
                                .size(13.0)
                                .color(colors.success)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    // Error-tinted container
                    root.container()
                        .background_color(colors.surface)
                        .border_color(colors.error)
                        .border_width(3.0)
                        .border_radius(12.0)
                        .padding(15.0)
                        .child(|root| {
                            root.text("Box with thick border and large radius")
                                .size(13.0)
                                .color(colors.error)
                                .build()
                        })
                        .build()
                })
                .build()
        })
        .build()
}

fn build_layout_section(
    root: &mut astrelis_ui::UiBuilder,
    theme: &astrelis_ui::Theme,
) -> astrelis_ui::NodeId {
    let colors = &theme.colors;
    root.container()
        .background_color(colors.surface)
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Layout Widgets")
                        .size(24.0)
                        .color(colors.info)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Row Layout (Horizontal)")
                        .size(14.0)
                        .color(colors.text_secondary)
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
                        .color(colors.text_secondary)
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

fn build_spacing_section(
    root: &mut astrelis_ui::UiBuilder,
    theme: &astrelis_ui::Theme,
) -> astrelis_ui::NodeId {
    let colors = &theme.colors;
    root.container()
        .background_color(colors.surface)
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Spacing Examples")
                        .size(24.0)
                        .color(colors.info)
                        .bold()
                        .build()
                })
                // Asymmetric padding example
                .child(|root| {
                    root.text("Asymmetric Padding (more horizontal)")
                        .size(12.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    root.container()
                        .background_color(colors.border)
                        .border_radius(8.0)
                        .padding_x(40.0) // More horizontal padding
                        .padding_y(10.0) // Less vertical padding
                        .child(|root| {
                            root.text("Horizontal emphasis")
                                .size(13.0)
                                .color(colors.text_primary)
                                .build()
                        })
                        .build()
                })
                // Card with different top/bottom padding
                .child(|root| {
                    root.text("Card Header/Content Padding")
                        .size(12.0)
                        .color(colors.text_secondary)
                        .margin_top(10.0)
                        .build()
                })
                .child(|root| {
                    root.column()
                        .background_color(colors.border)
                        .border_radius(8.0)
                        .padding_top(20.0)
                        .padding_bottom(10.0)
                        .padding_x(15.0)
                        .gap(8.0)
                        .child(|root| {
                            root.text("Card Title")
                                .size(16.0)
                                .color(colors.text_primary)
                                .bold()
                                .build()
                        })
                        .child(|root| {
                            root.text("Different top/bottom padding")
                                .size(12.0)
                                .color(colors.text_secondary)
                                .build()
                        })
                        .build()
                })
                // Per-side margin example
                .child(|root| {
                    root.text("Per-Side Margins")
                        .size(12.0)
                        .color(colors.text_secondary)
                        .margin_top(10.0)
                        .build()
                })
                .child(|root| {
                    root.container()
                        .background_color(colors.border)
                        .border_radius(8.0)
                        .padding(10.0)
                        .child(|root| {
                            root.container()
                                .background_color(colors.primary)
                                .border_radius(6.0)
                                .margin_left(30.0) // Indented from left
                                .margin_right(10.0)
                                .padding(10.0)
                                .child(|root| {
                                    root.text("Indented content")
                                        .size(12.0)
                                        .color(Color::WHITE)
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

fn build_tooltip_section(
    root: &mut astrelis_ui::UiBuilder,
    theme: &astrelis_ui::Theme,
) -> astrelis_ui::NodeId {
    let colors = &theme.colors;
    root.container()
        .background_color(colors.surface)
        .border_radius(12.0)
        .padding(20.0)
        .child(|root| {
            root.column()
                .gap(15.0)
                .child(|root| {
                    root.text("Tooltips & Overlays")
                        .size(20.0)
                        .color(colors.info)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Hover Information")
                        .size(14.0)
                        .color(colors.text_secondary)
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
                                .text_color(colors.text_primary)
                                .background_color(colors.surface)
                                .padding(8.0)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.text("Note: Tooltip API is implemented. Hover-triggered display is in development.")
                        .size(11.0)
                        .color(colors.text_disabled)
                        .margin(10.0)
                        .build()
                })
                .child(|root| {
                    root.text("Future: Tooltips will show on hover with customizable delay and positioning.")
                        .size(11.0)
                        .color(colors.text_secondary)
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
        self.ui
            .update_text(self.counter_text_id, format!("Count: {}", counter_value));

        // Begin frame and render with depth buffer for proper z-ordering
        let bg = self.ui.theme().colors.background;

        // Get depth view before starting frame (avoids borrow conflicts)
        let depth_view = self.ui.depth_view();

        let mut frame = self.window.begin_drawing();

        // Create render pass with depth attachment
        // We need to use raw wgpu API because frame.clear_and_render doesn't support depth
        {
            // SAFETY: We're creating a scope that ensures pass is dropped before we call
            // frame methods. The raw pointer usage is to work around borrow checker limitations.
            let surface_view = frame.surface().view() as *const wgpu::TextureView;
            let encoder = frame.encoder();

            // SAFETY: surface_view pointer is valid for the duration of this scope
            let surface_view = unsafe { &*surface_view };

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(bg.to_wgpu()),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0), // Clear to 0.0 for reverse-Z
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.ui.render(&mut pass);
        }

        frame.increment_passes();
        frame.finish();
    }
}
