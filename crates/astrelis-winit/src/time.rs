use std::time::{Duration, Instant};

use astrelis_core::profiling::profile_function;

/// Frame timing information for the app lifecycle.
///
/// This is a simplified time struct used by the winit app loop.
/// For more advanced time features (time scale, fixed timestep, etc.),
/// use the `Time` resource from the main `astrelis` crate.
///
/// # Example
///
/// ```no_run
/// use astrelis_winit::app::{App, AppCtx};
/// use astrelis_winit::{FrameTime, WindowId};
/// use astrelis_winit::event::EventBatch;
/// use astrelis_winit::window::WindowBackend;
///
/// struct MyApp;
///
/// impl App for MyApp {
///     fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
///         let dt = time.delta_seconds();
///         // Use dt for frame-independent movement
///         let _ = dt; // silence unused warning
///     }
///
///     fn render(&mut self, _ctx: &mut AppCtx, _window_id: WindowId, _events: &mut EventBatch) {
///         // rendering
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct FrameTime {
    /// Time elapsed since the last frame
    pub delta: Duration,
    /// Total time elapsed since app start
    pub elapsed: Duration,
    /// Total number of frames rendered
    pub frame_count: u64,
}

impl FrameTime {
    pub fn new() -> Self {
        Self {
            delta: Duration::ZERO,
            elapsed: Duration::ZERO,
            frame_count: 0,
        }
    }

    /// Returns delta time in seconds (f32)
    #[inline]
    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    /// Returns elapsed time in seconds (f32)
    #[inline]
    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }
}

impl Default for FrameTime {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks time for the app loop
pub(crate) struct TimeTracker {
    start_time: Instant,
    last_frame_time: Instant,
    frame_count: u64,
}

impl TimeTracker {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start_time: now,
            last_frame_time: now,
            frame_count: 0,
        }
    }

    pub fn tick(&mut self) -> FrameTime {
        profile_function!();
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame_time);
        let elapsed = now.duration_since(self.start_time);

        self.last_frame_time = now;
        self.frame_count += 1;

        FrameTime {
            delta,
            elapsed,
            frame_count: self.frame_count,
        }
    }
}
