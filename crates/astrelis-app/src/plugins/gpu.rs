//! GPU initialization plugin.

use astrelis_gpu::surface::SurfaceConfiguration;
use astrelis_gpu::types::PresentMode;
use astrelis_gpu::{Gpu, GpuConfig};

use crate::app::PrimaryWindowId;
use crate::phase::Phase;
use crate::plugin::Plugin;

/// Plugin that initializes the GPU backend and creates a presentation surface.
///
/// Depends on [`WindowPlugin`](super::window::WindowPlugin) — the primary
/// window must exist before GPU initialization.
pub struct GpuPlugin;

impl Plugin for GpuPlugin {
    fn build(&self, app: &mut crate::app::App) {
        app.add_startup(|resources, ctx| {
            let win_id = resources
                .get::<PrimaryWindowId>()
                .0;

            let gpu = Gpu::new(&GpuConfig::default()).expect("failed to create GPU backend");
            tracing::info!(
                adapter = %gpu.device().adapter_info().name,
                backend = ?gpu.device().adapter_info().backend,
                "GPU initialized"
            );

            let window = ctx.window(win_id).expect("primary window not found");
            let mut surface = gpu.create_surface(window).expect("failed to create surface");

            let size = window.inner_size().physical();
            let config = SurfaceConfiguration {
                format: surface.preferred_format(),
                width: size.width as u32,
                height: size.height as u32,
                present_mode: PresentMode::AutoVsync,
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&config);

            resources.insert(gpu);
            resources.insert(surface);
        });

        // Present phase: acquire frame, let Render systems draw, then present.
        // For now, the framework handles acquire/present in the App's event loop.
        // Individual render systems are responsible for encoding their own passes.
        app.add_system(Phase::Present, |resources| {
            let gpu = resources.get::<Gpu>();
            gpu.process_profiling_frames();
        });
    }
}
