//! Animation Showcase - Demonstrating Easing Functions
//!
//! This example showcases the animation system with various easing functions:
//! - Linear, EaseIn, EaseOut, EaseInOut
//! - Bounce, Elastic
//! - Quadratic and Cubic variations
//!
//! **Keyboard Controls:**
//! - **Space**: Start/restart all animations
//! - **R**: Reset all animations
//!
//! Watch how different easing functions affect the movement of boxes!

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{Color, GraphicsContext, RenderWindow, RenderWindowBuilder, wgpu};
use astrelis_ui::{ColorPalette, UiSystem};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{Event, EventBatch, HandleStatus, Key, NamedKey},
    window::{WindowDescriptor, WinitPhysicalSize},
};

struct AnimationShowcaseApp {
    window: RenderWindow,
    window_id: WindowId,
    ui: UiSystem,
    animation_time: f32,
    is_animating: bool,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx =
            GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Animation Showcase - Easing Functions".to_string(),
                size: Some(WinitPhysicalSize::new(1400.0, 800.0)),
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

        // Build initial UI
        build_animation_ui(&mut ui, size.width as f32, size.height as f32, 0.0);

        println!("\n═══════════════════════════════════════════════════════");
        println!("  🎬 ANIMATION SHOWCASE - Easing Functions Demo");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  CONTROLS:");
        println!("    [Space]  Start/pause animations");
        println!("    [R]      Reset animations to beginning");
        println!("\n  ⚠️  PRESS SPACE TO START THE ANIMATION!");
        println!("  Watch the boxes move with different easing curves!");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Animation showcase initialized");

        Box::new(AnimationShowcaseApp {
            window,
            window_id,
            ui,
            animation_time: 0.0,
            is_animating: false,
        })
    });
}

fn build_animation_ui(ui: &mut UiSystem, width: f32, height: f32, time: f32) {
    // Calculate progress (0.0 to 1.0) for 2-second animation cycle
    let progress = (time % 2.0) / 2.0;

    let theme = ui.theme().clone();
    let colors = theme.colors.clone();

    let bg = colors.background;
    let surface = colors.surface;
    let text_primary = colors.text_primary;
    let text_secondary = colors.text_secondary;
    let info = colors.info;

    ui.build(|root| {
        root.container()
            .width(width)
            .height(height)
            .padding(30.0)
            .background_color(bg)
            .child(|root| {
                root.column()
                    .gap(20.0)
                    .child(|root| {
                        // Header
                        root.container()
                            .child(|root| {
                                root.column()
                                    .gap(8.0)
                                    .child(|root| {
                                        root.text("Animation & Easing Showcase")
                                            .size(32.0)
                                            .color(text_primary)
                                            .bold()
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("Watch different easing functions in action")
                                            .size(14.0)
                                            .color(text_secondary)
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text(format!("Progress: {:.1}%", progress * 100.0))
                                            .size(16.0)
                                            .color(info)
                                            .build()
                                    })
                                    .build()
                            })
                            .build()
                    })
                    .child(|root| {
                        // Easing function demonstrations grid
                        root.column()
                            .gap(15.0)
                            .child(|root| {
                                build_easing_row(root, "Linear", progress, linear, &colors)
                            })
                            .child(|root| {
                                build_easing_row(root, "Ease In", progress, ease_in, &colors)
                            })
                            .child(|root| {
                                build_easing_row(root, "Ease Out", progress, ease_out, &colors)
                            })
                            .child(|root| {
                                build_easing_row(
                                    root,
                                    "Ease In-Out",
                                    progress,
                                    ease_in_out,
                                    &colors,
                                )
                            })
                            .child(|root| {
                                build_easing_row(root, "Bounce", progress, bounce, &colors)
                            })
                            .child(|root| {
                                build_easing_row(root, "Elastic", progress, elastic, &colors)
                            })
                            .build()
                    })
                    .child(|root| {
                        // Controls info
                        root.container()
                            .background_color(surface)
                            .border_radius(8.0)
                            .padding(15.0)
                            .child(|root| {
                                root.column()
                                    .gap(8.0)
                                    .child(|root| {
                                        root.text("Controls:")
                                            .size(16.0)
                                            .color(text_primary)
                                            .bold()
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("• Space: Start/restart animations")
                                            .size(13.0)
                                            .color(text_secondary)
                                            .build()
                                    })
                                    .child(|root| {
                                        root.text("• R: Reset animations")
                                            .size(13.0)
                                            .color(text_secondary)
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

fn build_easing_row(
    root: &mut astrelis_ui::UiBuilder,
    name: &str,
    progress: f32,
    easing_fn: fn(f32) -> f32,
    colors: &ColorPalette,
) -> astrelis_ui::NodeId {
    let eased = easing_fn(progress);
    let travel_distance = 580.0; // Max travel within the track
    let offset = eased * travel_distance;

    let surface = colors.surface;
    let bg = colors.background;
    let text_primary = colors.text_primary;
    let info = colors.info;

    root.container()
        .background_color(surface)
        .border_radius(8.0)
        .padding(15.0)
        .min_height(80.0)
        .child(|root| {
            root.row()
                .gap(20.0)
                .child(|root| {
                    // Label
                    root.container()
                        .min_width(120.0)
                        .child(|root| {
                            root.text(name)
                                .size(16.0)
                                .color(text_primary)
                                .bold()
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    // Track with positioned box
                    let mut track = root
                        .container()
                        .background_color(bg)
                        .border_radius(4.0)
                        .min_width(650.0)
                        .min_height(50.0)
                        .padding(5.0);

                    track = track.child(|root| {
                        let mut row = root.row();

                        // Add spacer to create offset effect
                        if offset > 0.0 {
                            row = row
                                .child(|root| root.container().width(offset).height(1.0).build());
                        }

                        // Add animated box
                        row = row.child(|root| {
                            root.container()
                                .background_color(info)
                                .border_radius(6.0)
                                .width(40.0)
                                .height(40.0)
                                .build()
                        });

                        row.build()
                    });

                    track.build()
                })
                .child(|root| {
                    // Progress value
                    root.container()
                        .min_width(80.0)
                        .child(|root| {
                            root.text(format!("{:.2}", eased))
                                .size(14.0)
                                .color(Color::from_rgb_u8(100, 200, 150))
                                .build()
                        })
                        .build()
                })
                .build()
        })
        .build()
}

// Easing function implementations
fn linear(t: f32) -> f32 {
    t
}

fn ease_in(t: f32) -> f32 {
    t * t
}

fn ease_out(t: f32) -> f32 {
    t * (2.0 - t)
}

fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        -1.0 + (4.0 - 2.0 * t) * t
    }
}

fn bounce(t: f32) -> f32 {
    if t < 1.0 / 2.75 {
        7.5625 * t * t
    } else if t < 2.0 / 2.75 {
        let t = t - 1.5 / 2.75;
        7.5625 * t * t + 0.75
    } else if t < 2.5 / 2.75 {
        let t = t - 2.25 / 2.75;
        7.5625 * t * t + 0.9375
    } else {
        let t = t - 2.625 / 2.75;
        7.5625 * t * t + 0.984375
    }
}

fn elastic(t: f32) -> f32 {
    if t == 0.0 || t == 1.0 {
        t
    } else {
        let p = 0.3;
        let s = p / 4.0;
        let t = t - 1.0;
        -(2.0f32.powf(10.0 * t) * ((t - s) * (2.0 * std::f32::consts::PI) / p).sin())
    }
}

impl App for AnimationShowcaseApp {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        new_frame();

        // Update animation time if animating
        if self.is_animating {
            self.animation_time += 0.016; // 60 FPS

            // Print debug info every second
            if (self.animation_time * 10.0) as i32 % 10 == 0 {
                let cycle = (self.animation_time % 2.0) / 2.0;
                tracing::debug!(
                    "Animation time: {:.2}s, cycle progress: {:.1}%",
                    self.animation_time,
                    cycle * 100.0
                );
            }
        }

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
                build_animation_ui(
                    &mut self.ui,
                    size.width as f32,
                    size.height as f32,
                    self.animation_time,
                );
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle keyboard events
        events.dispatch(|event| {
            if let Event::KeyInput(key) = event {
                if key.state == astrelis_winit::event::ElementState::Pressed {
                    match key.logical_key {
                        Key::Named(NamedKey::Space) => {
                            self.is_animating = !self.is_animating;
                            let status = if self.is_animating {
                                "STARTED"
                            } else {
                                "PAUSED"
                            };
                            println!(
                                "  ▶️  Animation {} (time: {:.2}s)",
                                status, self.animation_time
                            );
                            tracing::info!("Animation {}", status);
                            return HandleStatus::consumed();
                        }
                        Key::Character(ref c) if c.as_str() == "r" || c.as_str() == "R" => {
                            self.animation_time = 0.0;
                            self.is_animating = false;
                            println!("  🔄 Animation RESET");
                            tracing::info!("Animation reset");
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

        // Rebuild UI if animation time changed
        if self.is_animating {
            let size = self.window.physical_size();
            build_animation_ui(
                &mut self.ui,
                size.width as f32,
                size.height as f32,
                self.animation_time,
            );
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
