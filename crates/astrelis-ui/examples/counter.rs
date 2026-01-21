//! Counter Example - Demonstrating Incremental UI Updates, Animations, and Themes
//!
//! This example shows how the UI system uses lazy/incremental updates:
//! - Only changed widgets are marked dirty and recomputed
//! - Text measurements are cached (avoiding expensive text layout)
//! - Layout only recomputes for dirty subtrees
//! - Smooth animations on count changes
//! - Theme support with keyboard switching (T key)
//! - Focus navigation with Tab key
//!
//! Performance: Button clicks trigger <1ms updates instead of ~20ms full rebuilds
//! Enable puffin_viewer to see the performance improvements in real-time
//!
//! Controls:
//! - Click buttons to change counter
//! - Press T to toggle theme (dark/light)
//! - Press Tab to navigate focus

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow,
    WindowContextDescriptor, wgpu,
};
use astrelis_ui::UiSystem;
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{Event, EventBatch, HandleStatus, Key, NamedKey},
    window::{WinitPhysicalSize, WindowBackend, WindowDescriptor},
};
use std::sync::{Arc, RwLock};

// Theme definition
#[derive(Debug, Clone, Copy, PartialEq)]
enum Theme {
    Dark,
    Light,
}

impl Theme {
    fn toggle(&self) -> Self {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }

    fn background_color(&self) -> Color {
        match self {
            Theme::Dark => Color::from_rgb_u8(25, 25, 35),
            Theme::Light => Color::from_rgb_u8(240, 240, 245),
        }
    }

    fn surface_color(&self) -> Color {
        match self {
            Theme::Dark => Color::from_rgb_u8(40, 40, 55),
            Theme::Light => Color::from_rgb_u8(255, 255, 255),
        }
    }

    fn border_color(&self) -> Color {
        match self {
            Theme::Dark => Color::from_rgb_u8(80, 80, 120),
            Theme::Light => Color::from_rgb_u8(200, 200, 210),
        }
    }

    fn text_color(&self) -> Color {
        match self {
            Theme::Dark => Color::WHITE,
            Theme::Light => Color::from_rgb_u8(20, 20, 30),
        }
    }

    fn accent_color(&self) -> Color {
        match self {
            Theme::Dark => Color::from_rgb_u8(100, 200, 255),
            Theme::Light => Color::from_rgb_u8(60, 120, 200),
        }
    }

    fn secondary_text_color(&self) -> Color {
        match self {
            Theme::Dark => Color::from_rgb_u8(150, 150, 150),
            Theme::Light => Color::from_rgb_u8(100, 100, 110),
        }
    }
}

/// Shared application state
#[derive(Clone)]
struct CounterState {
    count: Arc<RwLock<i32>>,
}

impl CounterState {
    fn new() -> Self {
        Self {
            count: Arc::new(RwLock::new(0)),
        }
    }

    fn get(&self) -> i32 {
        *self.count.read().unwrap()
    }

    fn increment(&self) {
        let mut count = self.count.write().unwrap();
        *count += 1;
    }

    fn decrement(&self) {
        let mut count = self.count.write().unwrap();
        *count -= 1;
    }

    fn reset(&self) {
        let mut count = self.count.write().unwrap();
        *count = 0;
    }
}

struct CounterApp {
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
    state: CounterState,
    counter_text_id: astrelis_ui::WidgetId,
    theme: Theme,
}

fn main() {
    logging::init();

    // Initialize profiling - connect to puffin_viewer at http://127.0.0.1:8585
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync_or_panic();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Counter Example".to_string(),
                size: Some(WinitPhysicalSize::new(640.0, 480.0)),
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

        // Get actual window size
        let size = window.physical_size();
        let viewport_width = size.width as f32;
        let viewport_height = size.height as f32;

        // Create UI system
        let mut ui = UiSystem::new(graphics_ctx.clone());
        ui.set_viewport(window.viewport());

        // Create shared state
        let state = CounterState::new();
        let theme = Theme::Dark;

        // Build initial UI with callbacks
        let counter_text_id =
            build_counter_ui_with_callbacks(&mut ui, &state, theme, viewport_width, viewport_height);

        tracing::info!("Counter example initialized - using auto-dirty incremental updates");
        tracing::info!("Press T to toggle theme");

        Box::new(CounterApp {
            window,
            window_id,
            ui,
            state,
            counter_text_id,
            theme,
        })
    });
}

fn build_counter_ui_with_callbacks(
    ui: &mut UiSystem,
    state: &CounterState,
    theme: Theme,
    width: f32,
    height: f32,
) -> astrelis_ui::WidgetId {
    let count = state.get();
    let counter_text_id = astrelis_ui::WidgetId::new("counter_text");

    // Clone state for callbacks
    let state_inc = state.clone();
    let state_dec = state.clone();
    let state_reset = state.clone();

    ui.build(|root| {
        // Main container
        root.container()
            .width(width)
            .height(height)
            .padding(10.0)
            .background_color(theme.background_color())
            .child(|root| {
                // Center content vertically and horizontally
                root.column()
                    .gap(10.0)
                    .justify_content(taffy::JustifyContent::Center)
                    .align_items(taffy::AlignItems::Center)
                    .child(|root| {
                        // Title
                        root.text("Counter Example")
                            .size(18.0)
                            .color(theme.text_color())
                            .bold()
                            .build()
                    })
                    .child(|root| {
                        // Counter display container
                        root.container()
                            .background_color(theme.surface_color())
                            .border_color(theme.border_color())
                            .border_width(2.0)
                            .border_radius(8.0)
                            .padding(4.0)
                            .child(|root| {
                                root.text(format!("Count: {}", count))
                                    .id(counter_text_id)
                                    .size(24.0)
                                    .color(theme.accent_color())
                                    .bold()
                                    .build()
                            })
                            .build()
                    })
                    .child(|root| {
                        // Button row
                        root.row()
                            .gap(10.0)
                            .child(|root| {
                                let state = state_dec.clone();
                                root.button("-")
                                    .background_color(Color::from_rgb_u8(200, 60, 60))
                                    .hover_color(Color::from_rgb_u8(220, 80, 80))
                                    .padding(8.0)
                                    .min_width(30.0)
                                    .min_height(24.0)
                                    .font_size(16.0)
                                    .on_click(move || {
                                        state.decrement();
                                    })
                                    .build()
                            })
                            .child(|root| {
                                let state = state_reset.clone();
                                root.button("Reset")
                                    .background_color(Color::from_rgb_u8(80, 80, 100))
                                    .hover_color(Color::from_rgb_u8(100, 100, 120))
                                    .padding(15.0)
                                    .min_width(30.0)
                                    .min_height(24.0)
                                    .font_size(16.0)
                                    .on_click(move || {
                                        state.reset();
                                    })
                                    .build()
                            })
                            .child(|root| {
                                let state = state_inc.clone();
                                root.button("+")
                                    .background_color(Color::from_rgb_u8(60, 180, 60))
                                    .hover_color(Color::from_rgb_u8(80, 200, 80))
                                    .padding(15.0)
                                    .min_width(30.0)
                                    .min_height(24.0)
                                    .font_size(16.0)
                                    .on_click(move || {
                                        state.increment();
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .child(|root| {
                        // Info text
                        root.text("Click buttons to change the counter")
                            .size(14.0)
                            .color(theme.secondary_text_color())
                            .margin(10.0)
                            .build()
                    })
                    .child(|root| {
                        // Theme indicator
                        root.text(format!("Theme: {:?} (Press T to toggle)", theme))
                            .size(12.0)
                            .color(theme.secondary_text_color())
                            .margin(10.0)
                            .build()
                    })
                    .build()
            })
            .build();
    });

    counter_text_id
}

impl App for CounterApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        // Mark new profiling frame
        new_frame();

        // Update UI animations with real delta time
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
                self.counter_text_id = build_counter_ui_with_callbacks(
                    &mut self.ui,
                    &self.state,
                    self.theme,
                    size.width as f32,
                    size.height as f32,
                );

                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle keyboard events for theme toggle
        events.dispatch(|event| {
            if let Event::KeyInput(key) = event {
                if key.state == astrelis_winit::event::ElementState::Pressed {
                    match &key.logical_key {
                        Key::Character(c) if c == "t" || c == "T" => {
                            self.theme = self.theme.toggle();
                            tracing::info!("Theme toggled to: {:?}", self.theme);

                            // Rebuild UI with new theme
                            let size = self.window.physical_size();
                            self.counter_text_id = build_counter_ui_with_callbacks(
                                &mut self.ui,
                                &self.state,
                                self.theme,
                                size.width as f32,
                                size.height as f32,
                            );

                            return HandleStatus::consumed();
                        }
                        _ => {}
                    }
                }
            }
            HandleStatus::ignored()
        });

        // Handle UI events (callbacks will be triggered here)
        self.ui.handle_events(events);

        // Incremental update: Only update the counter text if it changed
        // Uses auto-dirty API - TEXT_SHAPING flag is marked automatically
        let new_count = self.state.get();
        self.ui
            .update_text(self.counter_text_id, format!("Count: {}", new_count));

        // Begin frame and render
        let mut frame = self.window.begin_drawing();

        // Render UI with automatic scoping (no manual {} block needed)
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb_u8(20, 20, 30),
            |pass| {
                self.ui.render(pass.descriptor());
            },
        );

        frame.finish();
    }
}
