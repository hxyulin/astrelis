//! Batched renderer example demonstrating the unified instance rendering system.
//!
//! Renders various shapes using the `BatchRenderer2D`:
//! - Solid colored quads at different depths
//! - Rounded rectangles with SDF corners
//! - Border-only outlines
//! - Semi-transparent overlapping quads
//! - Animated position and color changes
//!
//! Run with: `cargo run -p astrelis-render --example batched_renderer`
//!
//! Select render tier with `--tier`:
//!   `cargo run -p astrelis-render --example batched_renderer -- --tier auto`
//!   `cargo run -p astrelis-render --example batched_renderer -- --tier 1`      (or `direct`)
//!   `cargo run -p astrelis-render --example batched_renderer -- --tier 2`      (or `indirect`)
//!   `cargo run -p astrelis-render --example batched_renderer -- --tier 3`      (or `bindless`)

use std::collections::HashMap;
use std::sync::Arc;

use astrelis_core::logging;
use astrelis_render::batched::{
    create_batch_renderer_2d, BatchRenderer2D, BestBatchCapability2D, BindlessBatchCapability2D,
    DirectBatchCapability2D, DrawBatch2D, DrawType2D, IndirectBatchCapability2D, RenderTier,
    UnifiedInstance2D,
};
use astrelis_render::{
    GraphicsContext, GraphicsContextDescriptor, RenderableWindow, WindowContextDescriptor,
};
use astrelis_winit::app::run_app;
use astrelis_winit::window::{WindowBackend, WindowDescriptor, WinitPhysicalSize};
use astrelis_winit::WindowId;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

struct App {
    context: Arc<GraphicsContext>,
    windows: HashMap<WindowId, RenderableWindow>,
    renderer: Box<dyn BatchRenderer2D>,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    depth_width: u32,
    depth_height: u32,
    frame_count: u64,
}

impl App {
    fn ensure_depth_buffer(&mut self, width: u32, height: u32) {
        if self.depth_width == width && self.depth_height == height {
            return;
        }
        let w = width.max(1);
        let h = height.max(1);
        let texture = self.context.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("example_depth"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.depth_texture = texture;
        self.depth_view = view;
        self.depth_width = w;
        self.depth_height = h;
    }

    /// Build an orthographic projection matrix for the given viewport size.
    /// Maps (0,0) at top-left to (width, height) at bottom-right.
    /// Z range: 0.0 (far) to 1.0 (near), matching GreaterEqual depth compare.
    fn ortho_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
        [
            [2.0 / width, 0.0, 0.0, 0.0],
            [0.0, -2.0 / height, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0, 1.0],
        ]
    }

    /// Generate the demo instances for the current frame.
    fn build_instances(&self, width: f32, height: f32) -> Vec<UnifiedInstance2D> {
        let t = self.frame_count as f32 / 60.0;
        let mut instances = Vec::new();

        // --- Background panel (gray, full viewport, furthest back) ---
        instances.push(UnifiedInstance2D {
            position: [10.0, 10.0],
            size: [width - 20.0, height - 20.0],
            color: [0.15, 0.15, 0.18, 1.0],
            border_radius: 12.0,
            z_depth: 0.01,
            draw_type: DrawType2D::Quad as u32,
            ..Default::default()
        });

        // --- Grid of colored quads ---
        let cols = 5;
        let rows = 3;
        let margin = 30.0;
        let gap = 10.0;
        let cell_w = (width - 2.0 * margin - (cols as f32 - 1.0) * gap) / cols as f32;
        let cell_h = (height * 0.5 - margin - (rows as f32 - 1.0) * gap) / rows as f32;

        for row in 0..rows {
            for col in 0..cols {
                let x = margin + col as f32 * (cell_w + gap);
                let y = margin + row as f32 * (cell_h + gap);
                let idx = row * cols + col;

                // Hue-shift color based on grid position
                let hue = (idx as f32 / (rows * cols) as f32) * 360.0;
                let (r, g, b) = hsl_to_rgb(hue, 0.7, 0.55);

                instances.push(UnifiedInstance2D {
                    position: [x, y],
                    size: [cell_w, cell_h],
                    color: [r, g, b, 1.0],
                    border_radius: 6.0,
                    z_depth: 0.1 + idx as f32 * 0.001,
                    draw_type: DrawType2D::Quad as u32,
                    ..Default::default()
                });
            }
        }

        // --- Animated floating rounded rect ---
        let float_x = width * 0.5 + (t * 0.8).sin() * width * 0.25 - 60.0;
        let float_y = height * 0.35 + (t * 1.2).cos() * 30.0;
        instances.push(UnifiedInstance2D {
            position: [float_x, float_y],
            size: [120.0, 50.0],
            color: [1.0, 0.85, 0.2, 0.9],
            border_radius: 25.0,
            z_depth: 0.8,
            draw_type: DrawType2D::Quad as u32,
            ..Default::default()
        });

        // --- Border-only outlines (bottom area) ---
        let outline_y = height * 0.6;
        for i in 0..4 {
            let x = margin + i as f32 * 140.0;
            let thickness = 1.0 + i as f32;
            let radius = 4.0 + i as f32 * 8.0;
            instances.push(UnifiedInstance2D {
                position: [x, outline_y],
                size: [120.0, 80.0],
                color: [0.4, 0.8, 1.0, 1.0],
                border_radius: radius,
                border_thickness: thickness,
                z_depth: 0.5,
                draw_type: DrawType2D::Quad as u32,
                ..Default::default()
            });
        }

        // --- Overlapping transparent quads (demonstrating depth + alpha) ---
        let overlap_x = width * 0.5 - 100.0;
        let overlap_y = height * 0.75;
        let colors = [
            [1.0, 0.3, 0.3, 0.6],
            [0.3, 1.0, 0.3, 0.6],
            [0.3, 0.3, 1.0, 0.6],
        ];
        for (i, color) in colors.iter().enumerate() {
            let offset = i as f32 * 40.0;
            instances.push(UnifiedInstance2D {
                position: [overlap_x + offset, overlap_y + offset * 0.5],
                size: [120.0, 80.0],
                color: *color,
                border_radius: 8.0,
                z_depth: 0.6 + i as f32 * 0.05,
                draw_type: DrawType2D::Quad as u32,
                ..Default::default()
            });
        }

        // --- Pulsing circle (via large border_radius) ---
        let pulse = ((t * 2.0).sin() * 0.5 + 0.5) * 0.4 + 0.6;
        let circle_size = 60.0 * pulse;
        instances.push(UnifiedInstance2D {
            position: [width - margin - circle_size, outline_y + 10.0],
            size: [circle_size, circle_size],
            color: [1.0, 0.5, 0.0, 0.95],
            border_radius: circle_size * 0.5,
            z_depth: 0.7,
            draw_type: DrawType2D::Quad as u32,
            ..Default::default()
        });

        // --- Small shader-clipped quad (demonstrating clip rect) ---
        let clip_x = margin;
        let clip_y = height * 0.75;
        instances.push(UnifiedInstance2D {
            position: [clip_x, clip_y],
            size: [200.0, 60.0],
            color: [0.9, 0.2, 0.7, 1.0],
            border_radius: 4.0,
            z_depth: 0.55,
            draw_type: DrawType2D::Quad as u32,
            // Clip to a smaller region to demonstrate shader clipping
            clip_min: [clip_x + 20.0, clip_y + 10.0],
            clip_max: [clip_x + 160.0, clip_y + 50.0],
            ..Default::default()
        });

        instances
    }
}

/// Simple HSL to RGB conversion.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c * 0.5;
    (r1 + m, g1 + m, b1 + m)
}

/// Parse the `--tier` CLI argument to select a render tier.
///
/// The three tiers represent increasing levels of GPU capability:
/// - **Tier 1 (Direct):** One `draw()` call per texture group. Works on all hardware.
/// - **Tier 2 (Indirect):** Uses `multi_draw_indirect()` per texture group, batching
///   draw calls into GPU-side indirect buffers. Requires `MULTI_DRAW_INDIRECT`.
/// - **Tier 3 (Bindless):** Single `multi_draw_indirect()` per frame using texture
///   binding arrays. Requires `TEXTURE_BINDING_ARRAY` + `MULTI_DRAW_INDIRECT`.
///
/// Passing `--tier auto` (or omitting the flag) lets the engine choose the best
/// tier supported by the current GPU.
fn parse_tier() -> Option<RenderTier> {
    let args: Vec<String> = std::env::args().collect();
    for (i, arg) in args.iter().enumerate() {
        if arg == "--tier" {
            if let Some(value) = args.get(i + 1) {
                return match value.as_str() {
                    "1" | "direct" => Some(RenderTier::Direct),
                    "2" | "indirect" => Some(RenderTier::Indirect),
                    "3" | "bindless" => Some(RenderTier::Bindless),
                    "auto" => None,
                    other => {
                        eprintln!(
                            "Unknown tier '{other}'. Options: 1|direct, 2|indirect, 3|bindless, auto"
                        );
                        std::process::exit(1);
                    }
                };
            }
        }
    }
    // Default: auto-detect
    None
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let tier_override = parse_tier();

        // Use the capability API to configure GPU requirements.
        // For auto-detect, request the best capability (graceful degradation).
        // For a specific tier, require that tier's capability.
        let descriptor = match tier_override {
            None => GraphicsContextDescriptor::new()
                .request_capability::<BestBatchCapability2D>(),
            Some(RenderTier::Direct) => GraphicsContextDescriptor::new()
                .require_capability::<DirectBatchCapability2D>(),
            Some(RenderTier::Indirect) => GraphicsContextDescriptor::new()
                .require_capability::<IndirectBatchCapability2D>(),
            Some(RenderTier::Bindless) => GraphicsContextDescriptor::new()
                .require_capability::<BindlessBatchCapability2D>(),
        };
        let graphics_ctx =
            pollster::block_on(GraphicsContext::new_owned_with_descriptor(descriptor))
                .expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Batched Renderer Example".to_string(),
                size: Some(WinitPhysicalSize::new(800.0, 600.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let surface_format = wgpu::TextureFormat::Bgra8UnormSrgb;

        let renderable_window = RenderableWindow::new_with_descriptor(
            window,
            graphics_ctx.clone(),
            WindowContextDescriptor {
                format: Some(surface_format),
                ..Default::default()
            },
        )
        .expect("Failed to create renderable window");

        let window_id = renderable_window.id();

        let renderer = create_batch_renderer_2d(
            graphics_ctx.clone(),
            surface_format,
            tier_override,
        );

        tracing::info!("Using render tier: {}", renderer.tier());

        // Create initial depth buffer
        let depth_texture = graphics_ctx
            .device()
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("example_depth"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut windows = HashMap::new();
        windows.insert(window_id, renderable_window);

        Box::new(App {
            context: graphics_ctx,
            windows,
            renderer,
            depth_texture,
            depth_view,
            depth_width: 1,
            depth_height: 1,
            frame_count: 0,
        })
    });
}

impl astrelis_winit::app::App for App {
    fn update(
        &mut self,
        _ctx: &mut astrelis_winit::app::AppCtx,
        _time: &astrelis_winit::FrameTime,
    ) {
        self.frame_count += 1;
    }

    fn render(
        &mut self,
        _ctx: &mut astrelis_winit::app::AppCtx,
        window_id: WindowId,
        events: &mut astrelis_winit::event::EventBatch,
    ) {
        // Handle resize and get dimensions (scoped to release window borrow)
        let (phys_width, phys_height) = {
            let Some(window) = self.windows.get_mut(&window_id) else {
                return;
            };

            events.dispatch(|event| {
                if let astrelis_winit::event::Event::WindowResized(size) = event {
                    window.resized(*size);
                    astrelis_winit::event::HandleStatus::consumed()
                } else {
                    astrelis_winit::event::HandleStatus::ignored()
                }
            });

            let phys = window.physical_size();
            (phys.width, phys.height)
        };

        let width = phys_width as f32;
        let height = phys_height as f32;

        if width < 1.0 || height < 1.0 {
            return;
        }

        // Ensure depth buffer matches viewport
        self.ensure_depth_buffer(phys_width, phys_height);

        // Build instances and prepare GPU data
        let instances = self.build_instances(width, height);
        let batch = DrawBatch2D {
            instances,
            textures: vec![],
            projection: Self::ortho_projection(width, height),
        };
        self.renderer.prepare(&batch);

        let stats = self.renderer.stats();
        if self.frame_count % 120 == 0 {
            tracing::info!(
                "Frame {}: {} instances ({} opaque, {} transparent), {} draw calls",
                self.frame_count,
                stats.instance_count,
                stats.opaque_count,
                stats.transparent_count,
                stats.draw_calls,
            );
        }

        // Re-borrow window for rendering
        let window = self.windows.get_mut(&window_id).unwrap();
        let mut frame = window.begin_drawing();

        // Use RenderPassBuilder with depth stencil attachment
        frame.with_pass(
            astrelis_render::RenderPassBuilder::new()
                .label("batched_example_pass")
                .target(astrelis_render::RenderTarget::Surface)
                .clear_color(astrelis_render::Color::rgba(0.08, 0.08, 0.1, 1.0))
                .clear_depth(0.0) // 0.0 = far with GreaterEqual
                .depth_stencil_attachment(
                    &self.depth_view,
                    Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    None,
                ),
            |pass| {
                self.renderer.render(pass.wgpu_pass());
            },
        );

        frame.finish();
    }
}
