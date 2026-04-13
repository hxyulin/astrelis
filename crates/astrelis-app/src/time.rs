//! Time tracking and fixed-timestep accumulator.

use std::time::{Duration, Instant};

/// Tracks frame timing and manages the fixed-timestep accumulator.
///
/// Inserted as a resource by [`TimePlugin`](crate::plugins::time::TimePlugin).
/// Systems read it to get delta time, elapsed time, and frame count.
pub struct Time {
    /// When the app started (first frame).
    start: Instant,
    /// When the current frame began.
    frame_start: Instant,
    /// Duration of the last frame.
    delta: Duration,
    /// Total time since app start.
    elapsed: Duration,
    /// Fixed timestep interval (default: 1/60s).
    fixed_delta: Duration,
    /// Accumulator for fixed-timestep updates.
    accumulator: Duration,
    /// Total frames rendered.
    frame_count: u64,
}

impl Time {
    /// Creates a new `Time` with the default fixed timestep of 60 Hz.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            frame_start: now,
            delta: Duration::ZERO,
            elapsed: Duration::ZERO,
            fixed_delta: Duration::from_secs_f64(1.0 / 60.0),
            accumulator: Duration::ZERO,
            frame_count: 0,
        }
    }

    /// Variable delta time since the last frame.
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// Variable delta time as `f32` seconds.
    pub fn delta_secs(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    /// Total elapsed time since app start.
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Total elapsed time as `f64` seconds (for precision over long sessions).
    pub fn elapsed_secs(&self) -> f64 {
        self.elapsed.as_secs_f64()
    }

    /// The fixed timestep interval.
    pub fn fixed_delta(&self) -> Duration {
        self.fixed_delta
    }

    /// The fixed timestep interval as `f32` seconds.
    pub fn fixed_delta_secs(&self) -> f32 {
        self.fixed_delta.as_secs_f32()
    }

    /// Sets the fixed timestep interval.
    pub fn set_fixed_delta(&mut self, delta: Duration) {
        self.fixed_delta = delta;
    }

    /// Total number of frames rendered.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Updates timing for a new frame. Called by the framework.
    pub(crate) fn update(&mut self) {
        let now = Instant::now();
        self.delta = now - self.frame_start;

        // Clamp to prevent spiral of death: if a frame took over 250ms,
        // cap the delta so FixedUpdate doesn't run hundreds of times.
        const MAX_DELTA: Duration = Duration::from_millis(250);
        if self.delta > MAX_DELTA {
            self.delta = MAX_DELTA;
        }

        self.frame_start = now;
        self.elapsed = now - self.start;
        self.accumulator += self.delta;
        self.frame_count += 1;
    }

    /// Returns `true` if the accumulator has enough time for one fixed step,
    /// and subtracts the fixed delta from the accumulator.
    pub(crate) fn consume_fixed_step(&mut self) -> bool {
        if self.accumulator >= self.fixed_delta {
            self.accumulator -= self.fixed_delta;
            true
        } else {
            false
        }
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}
