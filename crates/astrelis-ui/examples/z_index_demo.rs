//! Z-Index Interactive Hit Testing Demo
//!
//! Demonstrates that z-index works correctly for both rendering AND hit testing.
//! Three overlapping panes at different z-levels contain clickable buttons.
//! Higher-z panes block clicks on lower-z elements in overlap regions.
//!
//! Layout (all sizes in vw/vh for responsive scaling):
//! - Layer 0 (z=0, teal):  Leftmost pane
//! - Layer 1 (z=10, blue): Middle pane, overlaps Layer 0
//! - Layer 2 (z=20, red):  Rightmost pane, overlaps Layer 1
//!
//! A status panel shows which layer received the last click and per-layer
//! click counts, proving that hit testing respects z-order.

use astrelis_core::profiling::{ProfilingBackend, init_profiling};
use astrelis_render::{Color, GraphicsContext, RenderWindow, RenderWindowBuilder, wgpu};
use astrelis_ui::constraint::Constraint;
use astrelis_ui::{Style, UiSystem, WidgetId};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{WindowDescriptor, WinitPhysicalSize},
};
use std::sync::{Arc, RwLock};
use taffy::prelude::TaffyZero;
use taffy::style::LengthPercentageAuto as Lpa;

// ── Shared state ────────────────────────────────────────────────────────────

#[derive(Clone)]
struct DemoState {
    last_clicked: Arc<RwLock<Option<u8>>>,
    click_counts: Arc<RwLock<[u32; 3]>>,
}

impl DemoState {
    fn new() -> Self {
        Self {
            last_clicked: Arc::new(RwLock::new(None)),
            click_counts: Arc::new(RwLock::new([0; 3])),
        }
    }

    fn click(&self, layer: u8) {
        *self.last_clicked.write().unwrap() = Some(layer);
        self.click_counts.write().unwrap()[layer as usize] += 1;
    }

    fn last_clicked(&self) -> Option<u8> {
        *self.last_clicked.read().unwrap()
    }

    fn counts(&self) -> [u32; 3] {
        *self.click_counts.read().unwrap()
    }
}

// ── Widget IDs for incremental updates ──────────────────────────────────────

const STATUS_TEXT: &str = "status_text";
const LAYER0_COUNT: &str = "layer0_count";
const LAYER1_COUNT: &str = "layer1_count";
const LAYER2_COUNT: &str = "layer2_count";

// ── Layer definition ────────────────────────────────────────────────────────

struct LayerDef {
    layer: u8,
    z: u16,
    /// left/top as percentage of parent (0–100)
    left_frac: f32,
    top_frac: f32,
}

// ── Colors ──────────────────────────────────────────────────────────────────

struct LayerColors {
    pane_bg: Color,
    border: Color,
    button_bg: Color,
    button_hover: Color,
    text_tint: Color,
}

fn layer_colors(layer: u8) -> LayerColors {
    match layer {
        0 => LayerColors {
            pane_bg: Color::rgb(0.12, 0.25, 0.20),
            border: Color::rgb(0.2, 0.5, 0.35),
            button_bg: Color::rgb(0.2, 0.6, 0.3),
            button_hover: Color::rgb(0.3, 0.7, 0.4),
            text_tint: Color::rgb(0.7, 1.0, 0.8),
        },
        1 => LayerColors {
            pane_bg: Color::rgb(0.12, 0.20, 0.35),
            border: Color::rgb(0.2, 0.35, 0.6),
            button_bg: Color::rgb(0.2, 0.4, 0.7),
            button_hover: Color::rgb(0.3, 0.5, 0.8),
            text_tint: Color::rgb(0.7, 0.8, 1.0),
        },
        _ => LayerColors {
            pane_bg: Color::rgb(0.35, 0.12, 0.12),
            border: Color::rgb(0.6, 0.2, 0.2),
            button_bg: Color::rgb(0.7, 0.2, 0.2),
            button_hover: Color::rgb(0.8, 0.3, 0.3),
            text_tint: Color::rgb(1.0, 0.7, 0.7),
        },
    }
}

// ── App ─────────────────────────────────────────────────────────────────────

struct ZIndexDemo {
    window: RenderWindow,
    window_id: WindowId,
    ui: UiSystem,
    state: DemoState,
}

impl ZIndexDemo {
    fn new(ctx: &mut AppCtx) -> Self {
        init_profiling(ProfilingBackend::PuffinHttp);

        let graphics =
            GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Z-Index Interactive Hit Testing Demo".to_string(),
                size: Some(WinitPhysicalSize::new(900.0, 700.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderWindowBuilder::new()
            .color_format(wgpu::TextureFormat::Bgra8UnormSrgb)
            .with_depth_default()
            .build(window, graphics.clone())
            .expect("Failed to create render window");

        let window_id = window.id();

        let mut ui = UiSystem::from_window(graphics.clone(), &window);
        ui.set_viewport(window.viewport());

        let state = DemoState::new();
        build_ui(&mut ui, &state);

        Self {
            window,
            window_id,
            ui,
            state,
        }
    }
}

impl App for ZIndexDemo {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        self.ui.update(time.delta_seconds());
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window resize
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                self.ui.set_viewport(self.window.viewport());
                build_ui(&mut self.ui, &self.state);
                return astrelis_winit::event::HandleStatus::consumed();
            }
            astrelis_winit::event::HandleStatus::ignored()
        });

        // Handle UI events (button callbacks fire here)
        self.ui.handle_events(events);

        // Incremental text updates from shared state
        let status = match self.state.last_clicked() {
            Some(n) => format!("Last click: Layer {}", n),
            None => "No clicks yet".to_string(),
        };
        self.ui.update_text(WidgetId::new(STATUS_TEXT), status);

        let counts = self.state.counts();
        self.ui.update_text(
            WidgetId::new(LAYER0_COUNT),
            format!("Layer 0: {}", counts[0]),
        );
        self.ui.update_text(
            WidgetId::new(LAYER1_COUNT),
            format!("Layer 1: {}", counts[1]),
        );
        self.ui.update_text(
            WidgetId::new(LAYER2_COUNT),
            format!("Layer 2: {}", counts[2]),
        );

        // Render
        let clear_color = self.ui.theme().colors.background;
        let Some(frame) = self.window.begin_frame() else {
            return;
        };

        {
            let mut pass = frame
                .render_pass()
                .clear_color(clear_color)
                .with_window_depth()
                .clear_depth(0.0)
                .label("Z-Index Demo")
                .build();

            self.ui.render(pass.wgpu_pass());
        }
        // Frame auto-submits on drop
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Build an absolute-positioned style using percent-based insets and vw/vh sizing.
fn abs_style(left_pct: f32, top_pct: f32, w_vw: f32, h_vh: f32, z: u16) -> Style {
    let mut s = Style::new()
        .width(Constraint::vw(w_vw))
        .height(Constraint::vh(h_vh))
        .display(taffy::Display::Flex)
        .flex_direction(taffy::FlexDirection::Column)
        .position(taffy::Position::Absolute)
        .z_index(z)
        .padding(15.0);
    s.layout.inset = taffy::Rect {
        left: Lpa::Percent(left_pct / 100.0),
        top: Lpa::Percent(top_pct / 100.0),
        right: Lpa::ZERO,
        bottom: Lpa::ZERO,
    };
    s
}

// ── UI construction ─────────────────────────────────────────────────────────

fn build_ui(ui: &mut UiSystem, state: &DemoState) {
    //
    // Positions as percentage of viewport (parent is 100%×100%):
    //   Layer 0: left ~4.5%, top ~6%
    //   Layer 1: left ~22%,  top ~20%
    //   Layer 2: left ~40%,  top ~9%
    //
    // Pane sizes: 42vw × 40vh  (overlap guaranteed)
    // Bottom panels sit at top ~73%
    //
    let layers = [
        LayerDef {
            layer: 0,
            z: 0,
            left_frac: 4.5,
            top_frac: 6.0,
        },
        LayerDef {
            layer: 1,
            z: 10,
            left_frac: 22.0,
            top_frac: 20.0,
        },
        LayerDef {
            layer: 2,
            z: 20,
            left_frac: 40.0,
            top_frac: 9.0,
        },
    ];

    ui.build(|root| {
        root.container()
            .style(
                Style::new()
                    .width(Constraint::Percent(100.0))
                    .height(Constraint::Percent(100.0)),
            )
            // Layer 0
            .child(|parent| build_layer_pane(parent, &layers[0], state))
            // Layer 1
            .child(|parent| build_layer_pane(parent, &layers[1], state))
            // Layer 2
            .child(|parent| build_layer_pane(parent, &layers[2], state))
            // Instructions panel (z=25)
            .child(|parent| {
                parent
                    .container()
                    .style(
                        abs_style(2.0, 73.0, 52.0, 25.0, 25)
                            .background_color(Color::rgb(0.15, 0.18, 0.15))
                            .border_radius(8.0)
                            .border_color(Color::rgb(0.3, 0.5, 0.3))
                            .border_width(2.0),
                    )
                    .overflow(astrelis_ui::Overflow::Hidden)
                    .child(|c| {
                        c.text("Instructions")
                            .size(15.0)
                            .bold()
                            .color(Color::rgb(0.8, 1.0, 0.8))
                            .build()
                    })
                    .child(|c| {
                        c.text("• Layers at z=0, z=10, z=20")
                            .padding_bottom(4.0)
                            .max_wrap_width(Constraint::vw(48.0))
                            .size(12.0)
                            .color(Color::rgb(0.7, 0.9, 0.7))
                            .build()
                    })
                    .child(|c| {
                        c.text("• Higher-z blocks lower-z clicks")
                            .padding_bottom(4.0)
                            .max_wrap_width(Constraint::vw(48.0))
                            .size(12.0)
                            .color(Color::rgb(0.7, 0.9, 0.7))
                            .build()
                    })
                    .child(|c| {
                        c.text("• Try clicking in overlap regions")
                            .padding_bottom(4.0)
                            .max_wrap_width(Constraint::vw(48.0))
                            .size(12.0)
                            .color(Color::rgb(0.7, 0.9, 0.7))
                            .build()
                    })
                    .child(|c| {
                        c.text("• Status panel shows the result")
                            .padding_bottom(4.0)
                            .max_wrap_width(Constraint::vw(48.0))
                            .size(12.0)
                            .color(Color::rgb(0.7, 0.9, 0.7))
                            .build()
                    })
                    .build()
            })
            // Status panel (z=25)
            .child(|parent| {
                parent
                    .container()
                    .style(
                        abs_style(56.0, 73.0, 42.0, 25.0, 25)
                            .background_color(Color::rgb(0.15, 0.15, 0.20))
                            .border_radius(8.0)
                            .border_color(Color::rgb(0.3, 0.3, 0.6))
                            .border_width(2.0),
                    )
                    .overflow(astrelis_ui::Overflow::Hidden)
                    .child(|c| {
                        c.text("Hit Test Results")
                            .size(15.0)
                            .bold()
                            .color(Color::rgb(0.8, 0.8, 1.0))
                            .build()
                    })
                    .child(|c| {
                        c.text("No clicks yet")
                            .id(WidgetId::new(STATUS_TEXT))
                            .padding_bottom(8.0)
                            .size(14.0)
                            .color(Color::rgb(1.0, 1.0, 0.7))
                            .build()
                    })
                    .child(|c| {
                        c.row()
                            .gap(16.0)
                            .child(|r| {
                                r.text("Layer 0: 0")
                                    .id(WidgetId::new(LAYER0_COUNT))
                                    .size(13.0)
                                    .color(layer_colors(0).text_tint)
                                    .build()
                            })
                            .child(|r| {
                                r.text("Layer 1: 0")
                                    .id(WidgetId::new(LAYER1_COUNT))
                                    .size(13.0)
                                    .color(layer_colors(1).text_tint)
                                    .build()
                            })
                            .child(|r| {
                                r.text("Layer 2: 0")
                                    .id(WidgetId::new(LAYER2_COUNT))
                                    .size(13.0)
                                    .color(layer_colors(2).text_tint)
                                    .build()
                            })
                            .build()
                    })
                    .build()
            })
            .build();
    });
}

fn build_layer_pane(
    parent: &mut astrelis_ui::builder::UiBuilder<'_>,
    def: &LayerDef,
    state: &DemoState,
) -> astrelis_ui::tree::NodeId {
    let colors = layer_colors(def.layer);
    let layer = def.layer;
    let z = def.z;
    let state = state.clone();

    let layer_names = ["teal", "blue", "red"];

    parent
        .container()
        .style(
            abs_style(def.left_frac, def.top_frac, 42.0, 40.0, z)
                .background_color(colors.pane_bg)
                .border_radius(12.0)
                .border_color(colors.border)
                .border_width(2.0),
        )
        .overflow(astrelis_ui::Overflow::Hidden)
        .child(|c| {
            c.text(format!("Layer {} (z={})", layer, z))
                .size(18.0)
                .bold()
                .color(Color::WHITE)
                .build()
        })
        .child(|c| {
            c.text(format!(
                "This {} pane is at z-index {}",
                layer_names[layer as usize], z
            ))
            .padding_bottom(8.0)
            .max_wrap_width(Constraint::vw(38.0))
            .size(13.0)
            .color(colors.text_tint)
            .build()
        })
        .child(|c| {
            c.button(format!("Click Layer {}", layer))
                .max_width(Constraint::vw(36.0))
                .background_color(colors.button_bg)
                .hover_color(colors.button_hover)
                .text_color(Color::WHITE)
                .font_size(14.0)
                .padding(10.0)
                .on_click(move || {
                    state.click(layer);
                })
                .build()
        })
        .build()
}

fn main() {
    run_app(|ctx| Box::new(ZIndexDemo::new(ctx)));
}
