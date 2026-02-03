//! Profiling Demo — CPU + GPU Profiling with Puffin
//!
//! Demonstrates comprehensive profiling of the render pipeline:
//! - CPU profiling via `puffin` (profile_function / profile_scope macros)
//! - GPU profiling via `wgpu-profiler` (timestamp queries reported to puffin)
//!
//! Run with GPU profiling:
//!   cargo run -p astrelis-render --example profiling_demo --features gpu-profiling
//!
//! Run without GPU profiling (CPU only):
//!   cargo run -p astrelis-render --example profiling_demo
//!
//! Then open puffin_viewer and connect to 127.0.0.1:8585 to see the flame graph.
//! Install puffin_viewer with: cargo install puffin_viewer

use std::sync::Arc;

use astrelis_core::logging;
use astrelis_core::profiling::{init_profiling, new_frame, ProfilingBackend, profile_function, profile_scope};
use astrelis_render::{
    Color, GraphicsContext, GraphicsContextDescriptor, RenderTarget, RenderableWindow,
    WindowContextDescriptor, wgpu,
};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{WinitPhysicalSize, WindowBackend, WindowDescriptor},
};

struct ProfilingDemo {
    #[allow(dead_code)]
    context: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    frame_count: u64,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        profile_function!();

        // Request TIMESTAMP_QUERY for GPU profiling (best-effort, won't fail if unavailable)
        let graphics_ctx = pollster::block_on(GraphicsContext::new_owned_with_descriptor(
            GraphicsContextDescriptor::new()
                .request_capability::<astrelis_render::gpu_profiling::GpuFrameProfiler>(),
        ))
        .expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Profiling Demo — CPU + GPU".to_string(),
                size: Some(WinitPhysicalSize::new(800.0, 600.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        #[allow(unused_mut)]
        let mut window = RenderableWindow::new_with_descriptor(
            window,
            graphics_ctx.clone(),
            WindowContextDescriptor {
                format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                ..Default::default()
            },
        )
        .expect("Failed to create renderable window");

        let window_id = window.id();

        // Attach GPU profiler to the window — all frames will be automatically profiled
        let has_gpu_profiling;
        #[cfg(feature = "gpu-profiling")]
        {
            match astrelis_render::gpu_profiling::GpuFrameProfiler::new(&graphics_ctx) {
                Ok(profiler) => {
                    let has_timestamps = profiler.has_timestamp_queries();
                    window.set_gpu_profiler(Arc::new(profiler));
                    has_gpu_profiling = true;
                    if has_timestamps {
                        println!("  GPU profiling: enabled with TIMESTAMP_QUERY (full timing)");
                    } else {
                        println!("  GPU profiling: enabled (debug groups only, no timing data)");
                        println!("                 TIMESTAMP_QUERY not supported by this GPU");
                    }
                }
                Err(e) => {
                    has_gpu_profiling = false;
                    tracing::warn!("Failed to create GPU profiler: {e}. GPU profiling disabled.");
                    println!("  GPU profiling: failed to create profiler");
                }
            }
        }
        #[cfg(not(feature = "gpu-profiling"))]
        {
            has_gpu_profiling = false;
        }

        println!();
        println!("═══════════════════════════════════════════════════");
        println!("  PROFILING DEMO — CPU + GPU");
        println!("═══════════════════════════════════════════════════");
        println!();
        println!("  CPU profiling: enabled (puffin)");
        if !has_gpu_profiling {
            #[cfg(not(feature = "gpu-profiling"))]
            println!("  GPU profiling: disabled (compile with --features gpu-profiling)");
        }
        println!();
        println!("  Open puffin_viewer at 127.0.0.1:8585 to see the flame graph.");
        println!("  Install with: cargo install puffin_viewer");
        println!("═══════════════════════════════════════════════════");
        println!();

        Box::new(ProfilingDemo {
            context: graphics_ctx,
            window,
            window_id,
            frame_count: 0,
        })
    });
}

impl App for ProfilingDemo {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        profile_function!();

        // Simulate some work to show up in the profiler
        {
            profile_scope!("simulate_game_logic");
            let mut _sum = 0.0f64;
            for i in 0..1000 {
                _sum += (i as f64).sin();
            }
        }

        self.frame_count += 1;
        if self.frame_count % 300 == 0 {
            tracing::info!("Frame {}", self.frame_count);
        }
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        new_frame();
        profile_function!();

        if window_id != self.window_id {
            return;
        }

        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        // Cycle the background color to visualize frames
        let t = (self.frame_count as f32 * 0.01).sin() * 0.5 + 0.5;
        let clear_color = Color::rgb(0.05 + t * 0.1, 0.05, 0.15 + (1.0 - t) * 0.1);

        // GPU profiling is now automatic — no special handling needed!
        // If a profiler is attached, with_pass/clear_and_render auto-create GPU scopes,
        // and FrameContext::Drop auto-resolves queries and ends the profiler frame.
        let mut frame = self.window.begin_drawing();

        {
            profile_scope!("render_frame");
            frame.clear_and_render(RenderTarget::Surface, clear_color, |_pass| {
                profile_scope!("draw_commands");
                // In a real app, you would issue draw calls here.
            });
        }

        frame.finish();

        // Simulate some post-render work
        {
            profile_scope!("post_render");
            let _ = &self.context;
        }
    }
}
