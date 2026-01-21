//! UI Inspector Demo - F12 Developer Tools for Debugging
//!
//! This example demonstrates the UI inspector debugging tool using the middleware API.
//!
//! **Features:**
//! - Widget bounds visualization (colored by type)
//! - Dirty flag display (color-coded: red=layout, orange=text, yellow=geometry, green=color)
//! - Interactive widget selection (click to select)
//! - Layout freeze functionality (pause layout to inspect dirty flags)
//! - Layout tree hierarchy view
//! - Performance metrics (FPS, layout time, render time)
//!
//! **Keyboard Controls (Middleware-based):**
//! - **F12**: Toggle inspector on/off
//! - **F5**: Toggle layout freeze (pause layout computation)
//! - **F6**: Toggle dirty flag overlay
//! - **F7**: Toggle bounds overlay
//! - **Escape**: Deselect widget
//!
//! **Mouse Controls:**
//! - **Click**: Increment counter (demonstrates dirty flag updates)

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow,
    WindowContextDescriptor, wgpu,
};
use astrelis_ui::{
    InspectorMiddleware, MiddlewareContext, MiddlewareManager, OverlayRenderer,
    UiSystem, WidgetId,
};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{ElementState, EventBatch, Event, HandleStatus, PhysicalKey},
    window::{WinitPhysicalSize, WindowBackend, WindowDescriptor},
};
use std::time::Instant;
use std::sync::{Arc, RwLock};

struct InspectorDemoApp {
    #[allow(dead_code)]
    graphics_ctx: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
    middlewares: MiddlewareManager,
    overlay_renderer: OverlayRenderer,
    counter: Arc<RwLock<i32>>,
    counter_text_id: WidgetId,
    last_frame: Instant,
    frame_count: u64,
    last_metrics_log: Instant,
    delta_time: f32,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync_or_panic();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "UI Inspector Demo - Press F12 to Toggle Inspector".to_string(),
                size: Some(WinitPhysicalSize::new(1280.0, 800.0)),
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
        ).expect("Failed to create renderable window");

        let window_id = window.id();
        let size = window.physical_size();

        let mut ui = UiSystem::new(graphics_ctx.clone());
        ui.set_viewport(window.viewport());

        // Create middleware manager with inspector
        let mut middlewares = MiddlewareManager::new();
        let inspector = InspectorMiddleware::new();

        // Register inspector keybinds with middleware's keybind registry
        {
            let keybinds = middlewares.keybind_registry_mut();
            inspector.register_keybinds(keybinds);
        }

        // Add inspector to middleware manager
        middlewares.add(inspector);

        // Create overlay renderer for middleware overlays
        let mut overlay_renderer = OverlayRenderer::new(graphics_ctx.clone());
        overlay_renderer.set_viewport(window.viewport());

        let counter = Arc::new(RwLock::new(0));

        // Build demo UI with various widgets
        let counter_text_id = build_demo_ui(&mut ui, size.width as f32, size.height as f32, counter.clone());

        println!("\n════════════════════════════════════════════════════════════");
        println!("  UI INSPECTOR DEMO - Middleware-Based Developer Tools");
        println!("════════════════════════════════════════════════════════════");
        println!("\n  CONTROLS (via Middleware Keybinds):");
        println!("    [F12]    Toggle inspector overlay on/off");
        println!("    [F5]     Toggle layout freeze (pause layout)");
        println!("    [F6]     Toggle dirty flag visualization");
        println!("    [F7]     Toggle bounds visualization");
        println!("    [Escape] Deselect widget");
        println!("    [Click]  Increment counter (shows dirty flags)");
        println!("\n  INSPECTOR FEATURES:");
        println!("    - Widget bounds visualization (colored by type)");
        println!("    - Dirty flag display (color-coded)");
        println!("    - Layout freeze for dirty flag inspection");
        println!("    - Performance metrics (FPS, timing)");
        println!("    - Layout tree hierarchy");
        println!("\n  This is your primary debugging tool for UI development!");
        println!("════════════════════════════════════════════════════════════\n");

        tracing::info!("Inspector demo initialized with middleware system");

        Box::new(InspectorDemoApp {
            graphics_ctx,
            window,
            window_id,
            ui,
            middlewares,
            overlay_renderer,
            counter,
            counter_text_id,
            last_frame: Instant::now(),
            frame_count: 0,
            last_metrics_log: Instant::now(),
            delta_time: 0.016,
        })
    });
}

fn build_demo_ui(
    ui: &mut UiSystem,
    width: f32,
    height: f32,
    counter: Arc<RwLock<i32>>,
) -> WidgetId {
    let counter_value = *counter.read().unwrap();
    let counter_text_id = WidgetId::new("counter_text");

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
                                        root.text("UI Inspector Demo (Middleware API)")
                                            .size(32.0)
                                            .color(Color::WHITE)
                                            .bold()
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("Press F12 to toggle inspector | F5 to freeze layout")
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
                                                root.text("Middleware Keybinds")
                                                    .size(20.0)
                                                    .color(Color::from_rgb_u8(255, 180, 100))
                                                    .bold()
                                                    .build()
                                            })
                                            .child(|root| {
                                                root.column()
                                                    .gap(10.0)
                                                    .child(|root| {
                                                        root.text("F12 - Toggle inspector")
                                                            .size(14.0)
                                                            .color(Color::from_rgb_u8(200, 200, 200))
                                                            .build()
                                                    })
                                                    .child(|root| {
                                                        root.text("F5 - Toggle layout freeze")
                                                            .size(14.0)
                                                            .color(Color::from_rgb_u8(200, 200, 200))
                                                            .build()
                                                    })
                                                    .child(|root| {
                                                        root.text("F6 - Toggle dirty flags")
                                                            .size(14.0)
                                                            .color(Color::from_rgb_u8(200, 200, 200))
                                                            .build()
                                                    })
                                                    .child(|root| {
                                                        root.text("F7 - Toggle bounds")
                                                            .size(14.0)
                                                            .color(Color::from_rgb_u8(200, 200, 200))
                                                            .build()
                                                    })
                                                    .child(|root| {
                                                        root.text("Escape - Deselect widget")
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
                                                                root.text("Red = Layout dirty")
                                                                    .size(12.0)
                                                                    .color(Color::from_rgb_u8(255, 100, 100))
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text("Orange = Text shaping")
                                                                    .size(12.0)
                                                                    .color(Color::from_rgb_u8(255, 180, 100))
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text("Yellow = Geometry")
                                                                    .size(12.0)
                                                                    .color(Color::from_rgb_u8(255, 255, 100))
                                                                    .build()
                                                            })
                                                            .child(|root| {
                                                                root.text("Green = Color only")
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
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        new_frame();

        // Calculate frame time
        let now = Instant::now();
        self.delta_time = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.frame_count += 1;

        // Update UI animations
        self.ui.update(self.delta_time);

        // Create middleware context and update middlewares
        let ctx = MiddlewareContext::new(
            self.ui.tree(),
            self.ui.core().events(),
            self.ui.core().widget_registry(),
            self.window.viewport(),
        )
        .with_delta_time(self.delta_time)
        .with_frame_number(self.frame_count);

        self.middlewares.update(&ctx, self.ui.tree());
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
                self.overlay_renderer.set_viewport(self.window.viewport());
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

        // Handle keyboard events through middleware system
        events.dispatch(|event| {
            if let Event::KeyInput(key) = event {
                if key.state == ElementState::Pressed {
                    // Convert physical key to KeyCode
                    if let PhysicalKey::Code(code) = key.physical_key {
                        // Create middleware context for keybind handling
                        let ctx = MiddlewareContext::new(
                            self.ui.tree(),
                            self.ui.core().events(),
                            self.ui.core().widget_registry(),
                            self.window.viewport(),
                        )
                        .with_delta_time(self.delta_time)
                        .with_frame_number(self.frame_count);

                        // Let middlewares handle the key event
                        let modifiers = astrelis_ui::Modifiers::NONE;
                        if self.middlewares.handle_key_event(code, modifiers, true, &ctx) {
                            return HandleStatus::consumed();
                        }
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

        // Pre-layout hook (can pause layout)
        let skip_layout = {
            let ctx = MiddlewareContext::new(
                self.ui.tree(),
                self.ui.core().events(),
                self.ui.core().widget_registry(),
                self.window.viewport(),
            )
            .with_delta_time(self.delta_time)
            .with_frame_number(self.frame_count);
            self.middlewares.pre_layout(&ctx)
        };

        // Compute layout (unless middleware paused it)
        if !skip_layout {
            self.ui.compute_layout();
        }

        // Post-layout hook
        {
            let ctx = MiddlewareContext::new(
                self.ui.tree(),
                self.ui.core().events(),
                self.ui.core().widget_registry(),
                self.window.viewport(),
            )
            .with_delta_time(self.delta_time)
            .with_frame_number(self.frame_count);
            self.middlewares.post_layout(&ctx);
        }

        // Begin frame and render
        let mut frame = self.window.begin_drawing();

        // Render main UI and overlays in a single render pass
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb_u8(20, 20, 30),
            |pass| {
                // Render main UI without computing layout (we already did that above)
                // When frozen, don't clear dirty flags so inspector can keep showing them
                self.ui.render_without_layout(pass.descriptor(), !skip_layout);

                // Collect overlay commands AFTER UI render but in same pass
                // Note: dirty flags are cleared by ui.render(), but inspector
                // has already captured them in update()
                if self.middlewares.has_middlewares() {
                    let ctx = MiddlewareContext::new(
                        self.ui.tree(),
                        self.ui.core().events(),
                        self.ui.core().widget_registry(),
                        self.window.viewport(),
                    )
                    .with_delta_time(self.delta_time)
                    .with_frame_number(self.frame_count);

                    let draw_list = self.middlewares.post_render(&ctx);

                    if !draw_list.is_empty() {
                        // Clone commands to avoid borrow issues
                        let commands: Vec<_> = draw_list.commands().to_vec();

                        // Re-create draw list from collected commands for rendering
                        let mut render_list = astrelis_ui::OverlayDrawList::new();
                        for cmd in commands {
                            match cmd {
                                astrelis_ui::middleware::OverlayCommand::Quad(q) => {
                                    render_list.add_quad(q.position, q.size, q.fill_color, q.border_color, q.border_width, q.border_radius);
                                }
                                astrelis_ui::middleware::OverlayCommand::Text(t) => {
                                    render_list.add_text(t.position, t.text, t.color, t.size);
                                }
                                astrelis_ui::middleware::OverlayCommand::Line(l) => {
                                    render_list.add_line(l.start, l.end, l.color, l.thickness);
                                }
                            }
                        }

                        let viewport = self.window.viewport();
                        self.overlay_renderer.render(
                            &render_list,
                            pass.descriptor(),
                            viewport,
                        );
                    }
                }
            },
        );

        // Log inspector info periodically
        if self.middlewares.has_middlewares() {
            let now = Instant::now();
            if now.duration_since(self.last_metrics_log).as_secs_f32() >= 2.0 {
                if let Some(inspector) = self.middlewares.get("inspector") {
                    if inspector.is_enabled() {
                        let is_frozen = self.middlewares.is_layout_frozen();
                        tracing::info!(
                            "Inspector (frame {}, frozen={}): {} middlewares active",
                            self.frame_count,
                            is_frozen,
                            self.middlewares.middleware_count()
                        );
                    }
                }
                self.last_metrics_log = now;
            }
        }

        frame.finish();
    }
}
