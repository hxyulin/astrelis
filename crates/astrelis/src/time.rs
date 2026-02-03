use std::time::{Duration, Instant};

use astrelis_core::profiling::profile_function;

/// Tracks timing information for the game loop
///
/// Provides delta time, elapsed time, frame counting, time scaling, and fixed timestep support.
/// Automatically inserted by `TimePlugin` in `DefaultPlugins`.
///
/// # Example
/// ```ignore
/// fn update(&mut self, ctx: &mut AppCtx, time: &Time) {
///     let dt = time.delta_seconds();
///     player_position += velocity * dt; // Frame-independent movement
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Time {
    /// Total time elapsed since app start
    elapsed: Duration,
    /// Time elapsed since last frame
    delta: Duration,
    /// Total number of frames rendered
    frame_count: u64,
    /// Time scale multiplier (1.0 = normal, 0.5 = half speed, 2.0 = double speed)
    time_scale: f32,
    /// Fixed timestep duration for physics (e.g., 50 FPS = 0.02s)
    fixed_timestep: Duration,
    /// Accumulated time for fixed timestep simulation
    fixed_accumulator: Duration,
    /// Maximum delta time to prevent spiral of death (default: 0.1s)
    max_delta: Duration,
    /// Start time of the application
    start_time: Instant,
    /// Last frame time
    last_frame_time: Instant,
}

impl Time {
    /// Creates a new Time instance with default values
    ///
    /// - Fixed timestep: 50 FPS (0.02 seconds)
    /// - Time scale: 1.0 (normal speed)
    /// - Max delta: 0.1 seconds
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            elapsed: Duration::ZERO,
            delta: Duration::ZERO,
            frame_count: 0,
            time_scale: 1.0,
            fixed_timestep: Duration::from_millis(20), // 50 FPS
            fixed_accumulator: Duration::ZERO,
            max_delta: Duration::from_millis(100), // 100ms max
            start_time: now,
            last_frame_time: now,
        }
    }

    /// Updates the time for a new frame
    ///
    /// Should be called once per frame, typically in the game loop.
    pub fn update(&mut self) {
        profile_function!();
        let now = Instant::now();
        let raw_delta = now.duration_since(self.last_frame_time);

        // Cap delta time to prevent spiral of death
        self.delta = raw_delta.min(self.max_delta);
        self.elapsed = now.duration_since(self.start_time);
        self.last_frame_time = now;
        self.frame_count += 1;

        // Accumulate time for fixed timestep
        self.fixed_accumulator += self.delta;
    }

    /// Returns the elapsed time since the last frame as a Duration
    #[inline]
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// Returns the elapsed time since the last frame in seconds (f32)
    ///
    /// This is the most commonly used method for frame-independent movement.
    #[inline]
    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32() * self.time_scale
    }

    /// Returns the elapsed time since the last frame in seconds (f64)
    #[inline]
    pub fn delta_seconds_f64(&self) -> f64 {
        self.delta.as_secs_f64() * self.time_scale as f64
    }

    /// Returns the total elapsed time since app start
    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Returns the total elapsed time since app start in seconds (f32)
    #[inline]
    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    /// Returns the total elapsed time since app start in seconds (f64)
    #[inline]
    pub fn elapsed_seconds_f64(&self) -> f64 {
        self.elapsed.as_secs_f64()
    }

    /// Returns the total number of frames rendered
    #[inline]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Returns the current time scale
    #[inline]
    pub fn time_scale(&self) -> f32 {
        self.time_scale
    }

    /// Sets the time scale (affects delta_seconds but not delta)
    ///
    /// - 1.0 = normal speed
    /// - 0.5 = half speed (slow motion)
    /// - 2.0 = double speed (fast forward)
    /// - 0.0 = paused
    #[inline]
    pub fn set_time_scale(&mut self, scale: f32) {
        self.time_scale = scale.max(0.0);
    }

    /// Returns the fixed timestep duration
    #[inline]
    pub fn fixed_timestep(&self) -> Duration {
        self.fixed_timestep
    }

    /// Returns the fixed timestep in seconds (f32)
    #[inline]
    pub fn fixed_timestep_seconds(&self) -> f32 {
        self.fixed_timestep.as_secs_f32()
    }

    /// Sets the fixed timestep duration
    ///
    /// Common values:
    /// - 60 FPS: Duration::from_millis(16) or Duration::from_secs_f32(1.0 / 60.0)
    /// - 50 FPS: Duration::from_millis(20) or Duration::from_secs_f32(1.0 / 50.0)
    /// - 30 FPS: Duration::from_millis(33) or Duration::from_secs_f32(1.0 / 30.0)
    pub fn set_fixed_timestep(&mut self, timestep: Duration) {
        self.fixed_timestep = timestep;
    }

    /// Returns whether a fixed update should run
    ///
    /// Returns true if enough time has accumulated for at least one fixed timestep.
    #[inline]
    pub fn should_fixed_update(&self) -> bool {
        self.fixed_accumulator >= self.fixed_timestep
    }

    /// Consumes one fixed timestep from the accumulator
    ///
    /// Should be called after each fixed_update() to prevent accumulator overflow.
    pub fn consume_fixed_timestep(&mut self) {
        if self.fixed_accumulator >= self.fixed_timestep {
            self.fixed_accumulator -= self.fixed_timestep;
        }
    }

    /// Returns the number of fixed updates that should run this frame
    ///
    /// This is useful for catching up on physics when frames are slow.
    /// The value is capped at 5 to prevent spiral of death.
    pub fn fixed_update_count(&self) -> usize {
        let count = self.fixed_accumulator.as_secs_f32() / self.fixed_timestep.as_secs_f32();
        (count as usize).min(5) // Cap at 5 to prevent spiral of death
    }

    /// Resets the fixed timestep accumulator
    ///
    /// Useful when resuming from pause or after a long hitch.
    pub fn reset_fixed_accumulator(&mut self) {
        self.fixed_accumulator = Duration::ZERO;
    }

    /// Returns the maximum delta time cap
    #[inline]
    pub fn max_delta(&self) -> Duration {
        self.max_delta
    }

    /// Sets the maximum delta time cap
    ///
    /// Prevents "spiral of death" where a slow frame causes the next frame to be even slower.
    pub fn set_max_delta(&mut self, max_delta: Duration) {
        self.max_delta = max_delta;
    }

    /// Returns the start time of the application
    #[inline]
    pub fn start_time(&self) -> Instant {
        self.start_time
    }

    /// Returns the last frame time
    #[inline]
    pub fn last_frame_time(&self) -> Instant {
        self.last_frame_time
    }

    /// Pauses time (sets time scale to 0.0)
    #[inline]
    pub fn pause(&mut self) {
        self.time_scale = 0.0;
    }

    /// Resumes time (sets time scale to 1.0)
    #[inline]
    pub fn resume(&mut self) {
        self.time_scale = 1.0;
    }

    /// Returns whether time is paused
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.time_scale == 0.0
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_time_creation() {
        let time = Time::new();
        assert_eq!(time.frame_count(), 0);
        assert_eq!(time.elapsed(), Duration::ZERO);
        assert_eq!(time.delta(), Duration::ZERO);
        assert_eq!(time.time_scale(), 1.0);
        assert!(!time.is_paused());
    }

    #[test]
    fn test_time_update() {
        let mut time = Time::new();

        // Sleep a bit to ensure time passes
        thread::sleep(Duration::from_millis(10));
        time.update();

        assert_eq!(time.frame_count(), 1);
        assert!(time.delta() > Duration::ZERO);
        assert!(time.elapsed() > Duration::ZERO);
        assert!(time.delta_seconds() > 0.0);
    }

    #[test]
    fn test_time_scale() {
        let mut time = Time::new();
        thread::sleep(Duration::from_millis(10));
        time.update();

        let normal_dt = time.delta_seconds();

        // Half speed
        time.set_time_scale(0.5);
        assert_eq!(time.time_scale(), 0.5);
        assert!((time.delta_seconds() - normal_dt * 0.5).abs() < 0.001);

        // Double speed
        time.set_time_scale(2.0);
        assert_eq!(time.time_scale(), 2.0);
        assert!((time.delta_seconds() - normal_dt * 2.0).abs() < 0.001);

        // Pause
        time.pause();
        assert!(time.is_paused());
        assert_eq!(time.delta_seconds(), 0.0);

        // Resume
        time.resume();
        assert!(!time.is_paused());
        assert_eq!(time.time_scale(), 1.0);
    }

    #[test]
    fn test_fixed_timestep() {
        let mut time = Time::new();
        time.set_fixed_timestep(Duration::from_millis(16)); // ~60 FPS

        assert_eq!(time.fixed_timestep(), Duration::from_millis(16));
        assert!((time.fixed_timestep_seconds() - 0.016).abs() < 0.001);

        // Initially should not update
        assert!(!time.should_fixed_update());
        assert_eq!(time.fixed_update_count(), 0);

        // Simulate 32ms frame (should trigger 2 fixed updates)
        thread::sleep(Duration::from_millis(32));
        time.update();

        assert!(time.should_fixed_update());
        let count = time.fixed_update_count();
        assert!(count >= 1 && count <= 3); // Allow some tolerance

        // Consume one timestep
        time.consume_fixed_timestep();

        // Reset accumulator
        time.reset_fixed_accumulator();
        assert!(!time.should_fixed_update());
    }

    #[test]
    fn test_max_delta() {
        let mut time = Time::new();
        time.set_max_delta(Duration::from_millis(50));

        assert_eq!(time.max_delta(), Duration::from_millis(50));

        // Simulate a very long frame (100ms)
        thread::sleep(Duration::from_millis(100));
        time.update();

        // Delta should be capped at max_delta
        assert!(time.delta() <= Duration::from_millis(50));
    }

    #[test]
    fn test_multiple_frames() {
        let mut time = Time::new();

        for i in 1..=5 {
            thread::sleep(Duration::from_millis(10));
            time.update();
            assert_eq!(time.frame_count(), i);
            assert!(time.elapsed() > Duration::ZERO);
        }
    }
}
