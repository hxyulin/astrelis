//! Overflow, Viewport Units & Scrollbar Demo
//!
//! This example demonstrates:
//! - Overflow clipping (Overflow::Hidden)
//! - Viewport units (vw, vh, vmin, vmax)
//! - HScrollbar / VScrollbar widgets rendering
//!
//! Controls:
//! - Resize the window to see viewport units in action
//! - The panels will clip their overflow content

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{
    Color, GraphicsContext, RenderableWindow, WindowContextDescriptor, wgpu,
};
use astrelis_ui::style::Style;
use astrelis_ui::widgets::{Container, Text};
use astrelis_ui::{
    ColorPalette, HScrollbar, NodeId, Overflow, ScrollbarTheme, UiBuilder, UiSystem, VScrollbar,
};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{Event, EventBatch, HandleStatus},
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
};

struct OverflowDemoApp {
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Overflow & Scrollbar Demo".to_string(),
                size: Some(WinitPhysicalSize::new(900.0, 650.0)),
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

        // Build initial UI
        build_overflow_ui(&mut ui, &window);

        tracing::info!("Overflow demo initialized");
        tracing::info!("Resize the window to see viewport units in action");

        Box::new(OverflowDemoApp {
            window,
            window_id,
            ui,
        })
    });
}

fn build_overflow_ui(ui: &mut UiSystem, window: &RenderableWindow) {
    let size = window.physical_size();
    let width = size.width as f32;
    let height = size.height as f32;

    let theme = ui.theme().clone();
    let colors = theme.colors.clone();

    let bg = colors.background;
    let text_primary = colors.text_primary;
    let text_secondary = colors.text_secondary;

    ui.build(|root| {
        // Main container - full viewport
        root.container()
            .width(width)
            .height(height)
            .padding(20.0)
            .background_color(bg)
            .child(|root| {
                root.column()
                    .gap(20.0)
                    .child(|root| {
                        // Title
                        root.text("Overflow & Scrollbar Demo")
                            .size(24.0)
                            .color(text_primary)
                            .bold()
                            .build()
                    })
                    .child(|root| {
                        // Subtitle showing viewport info
                        root.text(format!("Viewport: {}x{} px", width as i32, height as i32))
                            .size(14.0)
                            .color(text_secondary)
                            .build()
                    })
                    .child(|root| {
                        // Row of demo panels
                        root.row()
                            .gap(20.0)
                            .child(|root| {
                                // Panel 1: Basic overflow hidden
                                build_overflow_hidden_panel(root, &colors)
                            })
                            .child(|root| {
                                // Panel 2: Viewport unit sizes
                                build_viewport_units_panel(root, &colors)
                            })
                            .build()
                    })
                    .child(|root| {
                        // Info text
                        root.text(
                            "The panels above demonstrate overflow clipping and viewport units.",
                        )
                        .size(12.0)
                        .color(text_secondary)
                        .build()
                    })
                    .build()
            })
            .build();
    });

    // Add scrollbar panel directly on the tree (no builder support yet)
    add_scrollbar_panel(ui, &colors);
}

/// Add a scrollbar demo panel by manipulating the tree directly.
///
/// The builder API does not yet support HScrollbar/VScrollbar, so we create
/// them via the tree API and attach them to the existing layout.
fn add_scrollbar_panel(ui: &mut UiSystem, colors: &ColorPalette) {
    let tree = ui.tree_mut();

    // Find the row node (3rd child of the column, which is the child of root).
    // Tree structure: root -> container -> column -> [title, subtitle, row, info]
    let root = tree.root().expect("tree has root");
    let container_id = tree.get_widget(root).unwrap().children()[0]; // the column wrapper
    let column_id = tree.get_widget(container_id).unwrap().children()[0]; // the column
    let row_id = tree.get_widget(column_id).unwrap().children()[2]; // the row

    // --- Panel container ---
    let panel_style = Style::new()
        .display(taffy::Display::Flex)
        .flex_direction(taffy::FlexDirection::Column)
        .width(280.0)
        .height(200.0)
        .background_color(colors.surface)
        .border_color(colors.border)
        .border_width(2.0)
        .border_radius(8.0)
        .padding(10.0)
        .gap(6.0);
    let panel = Container::with_style(panel_style);
    let panel_id = tree.add_widget(Box::new(panel));

    // --- Title ---
    let title = Text::new("Scrollbar Widgets")
        .size(16.0)
        .color(colors.info);
    let title_id = tree.add_widget(Box::new(title));

    // --- Description ---
    let desc = Text::new("HScrollbar and VScrollbar rendered below:")
        .size(11.0)
        .color(colors.text_secondary);
    let desc_id = tree.add_widget(Box::new(desc));

    // --- HScrollbar label + widget ---
    let h_label = Text::new("Horizontal (content=800, viewport=240)")
        .size(10.0)
        .color(colors.text_secondary);
    let h_label_id = tree.add_widget(Box::new(h_label));

    let mut h_scrollbar = HScrollbar::new(800.0, 240.0);
    h_scrollbar.scroll_offset = 120.0; // show thumb not at start
    h_scrollbar.style = Style::new().width(240.0).height(8.0);
    h_scrollbar.theme = bright_scrollbar_theme();
    let h_scroll_id = tree.add_widget(Box::new(h_scrollbar));

    // --- VScrollbar label + row ---
    let v_label = Text::new("Vertical (content=600, viewport=100)")
        .size(10.0)
        .color(colors.text_secondary);
    let v_label_id = tree.add_widget(Box::new(v_label));

    let mut v_scrollbar = VScrollbar::new(600.0, 100.0);
    v_scrollbar.scroll_offset = 80.0; // show thumb not at start
    v_scrollbar.style = Style::new().width(8.0).height(100.0);
    v_scrollbar.theme = bright_scrollbar_theme();
    let v_scroll_id = tree.add_widget(Box::new(v_scrollbar));

    // Wire up panel children: title, desc, h_label, h_scrollbar, v_label, v_scrollbar
    let children = vec![
        title_id,
        desc_id,
        h_label_id,
        h_scroll_id,
        v_label_id,
        v_scroll_id,
    ];

    // Set widget-level children
    if let Some(widget) = tree.get_widget_mut(panel_id)
        && let Some(wchildren) = widget.children_mut()
    {
        *wchildren = children.clone();
    }
    tree.set_children(panel_id, &children);

    // Add panel as child of the row
    if let Some(widget) = tree.get_widget_mut(row_id)
        && let Some(wchildren) = widget.children_mut()
    {
        wchildren.push(panel_id);
    }
    tree.add_child(row_id, panel_id);
}

/// A brighter scrollbar theme so the track and thumb are clearly visible.
fn bright_scrollbar_theme() -> ScrollbarTheme {
    ScrollbarTheme {
        track_color: Color::from_rgba_u8(60, 60, 70, 200),
        thumb_color: Color::from_rgb_u8(130, 140, 220),
        thumb_hover_color: Color::from_rgb_u8(160, 170, 240),
        thumb_active_color: Color::from_rgb_u8(180, 190, 255),
        thumb_border_radius: 4.0,
        min_thumb_length: 20.0,
        thickness: 8.0,
    }
}

fn build_overflow_hidden_panel(root: &mut UiBuilder, colors: &ColorPalette) -> NodeId {
    let surface = colors.surface;
    let border = colors.border;
    let info = colors.info;
    let text_secondary = colors.text_secondary;
    let text_primary = colors.text_primary;

    root.container()
        .width(250.0)
        .height(200.0)
        .background_color(surface)
        .border_color(border)
        .border_width(2.0)
        .border_radius(8.0)
        .overflow(Overflow::Hidden) // This clips overflow content
        .child(|root| {
            let mut col = root
                .column()
                .padding(10.0)
                .gap(5.0)
                .child(|root| {
                    root.text("Overflow: Hidden")
                        .size(16.0)
                        .color(info)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("This panel clips its content.")
                        .size(12.0)
                        .color(text_secondary)
                        .build()
                });

            // Add many items that will overflow
            for i in 1..=20 {
                let color = if i <= 5 {
                    text_primary
                } else {
                    text_secondary
                };
                col = col.child(move |root| {
                    root.text(format!(
                        "Item {} - This is a list item that may be clipped",
                        i
                    ))
                    .size(12.0)
                    .color(color)
                    .build()
                });
            }

            col.build()
        })
        .build()
}

fn build_viewport_units_panel(root: &mut UiBuilder, colors: &ColorPalette) -> NodeId {
    root.container()
        .width(300.0) // Fixed width for now (viewport units need resolution)
        .height(200.0)
        .background_color(colors.surface)
        .border_color(colors.border)
        .border_width(2.0)
        .border_radius(8.0)
        .child(|root| {
            root.column()
                .padding(10.0)
                .gap(8.0)
                .child(|root| {
                    root.text("Viewport Units")
                        .size(16.0)
                        .color(colors.info)
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("Available units:")
                        .size(12.0)
                        .color(colors.text_primary)
                        .build()
                })
                .child(|root| {
                    root.text("- vw: 1% of viewport width")
                        .size(11.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    root.text("- vh: 1% of viewport height")
                        .size(11.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    root.text("- vmin: 1% of smaller dimension")
                        .size(11.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    root.text("- vmax: 1% of larger dimension")
                        .size(11.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    root.text("Resize window to see changes!")
                        .size(12.0)
                        .color(colors.warning)
                        .margin(10.0)
                        .build()
                })
                .build()
        })
        .build()
}

impl App for OverflowDemoApp {
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

                tracing::info!(
                    "Window resized to {}x{} - rebuilding UI",
                    size.width,
                    size.height,
                );

                // Rebuild UI with new viewport size
                build_overflow_ui(&mut self.ui, &self.window);

                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle UI events
        self.ui.handle_events(events);

        // Begin frame and render with depth buffer for proper z-ordering
        let bg = self.ui.theme().colors.background;

        // Get depth view before starting frame (avoids borrow conflicts)
        let depth_view = self.ui.depth_view();

        let mut frame = self.window.begin_drawing();

        // Create render pass with depth attachment
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
