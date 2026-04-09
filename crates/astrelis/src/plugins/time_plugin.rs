//! Time plugin for tracking frame timing and game time.
use std::time::Duration;

use crate::plugin::Plugin;
use crate::resource::Resources;
use crate::time::Time;

/// Plugin that provides time tracking for the game loop.
///
/// This plugin sets up the `Time` resource which tracks delta time,
/// elapsed time, frame count, time scaling, and fixed timestep support.
///
/// # Resources Provided
///
/// - `Time` - Frame timing and game time information
///
/// # Example
///
/// ```ignore
/// use astrelis::prelude::*;
/// use std::time::Duration;
///
/// // Default configuration (50 FPS fixed timestep, 100ms max delta):
/// let engine = Engine::builder()
///     .add_plugin(TimePlugin::default())
///     .build();
///
/// // Custom configuration:
/// let engine = Engine::builder()
///     .add_plugin(
///         TimePlugin::with_fixed_timestep(Duration::from_millis(16)) // 60 FPS
///             .with_max_delta(Duration::from_millis(50))
///     )
///     .build();
/// ```
pub struct TimePlugin {
    fixed_timestep: Option<Duration>,
    max_delta: Option<Duration>,
}

impl TimePlugin {
    /// Create with default time settings.
    pub fn new() -> Self {
        Self {
            fixed_timestep: None,
            max_delta: None,
        }
    }

    /// Create with a custom fixed timestep.
    ///
    /// Common values:
    /// - 60 FPS: `Duration::from_millis(16)`
    /// - 50 FPS: `Duration::from_millis(20)` (default)
    /// - 30 FPS: `Duration::from_millis(33)`
    pub fn with_fixed_timestep(timestep: Duration) -> Self {
        Self {
            fixed_timestep: Some(timestep),
            max_delta: None,
        }
    }

    /// Set the maximum delta time cap.
    ///
    /// Prevents "spiral of death" where a slow frame causes the next
    /// frame to be even slower. Default is 100ms.
    pub fn with_max_delta(mut self, max_delta: Duration) -> Self {
        self.max_delta = Some(max_delta);
        self
    }
}

impl Default for TimePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TimePlugin {
    type Dependencies = ();
    fn name(&self) -> &'static str {
        "TimePlugin"
    }

    fn build(&self, resources: &mut Resources) {
        let mut time = Time::new();
        if let Some(timestep) = self.fixed_timestep {
            time.set_fixed_timestep(timestep);
        }
        if let Some(max_delta) = self.max_delta {
            time.set_max_delta(max_delta);
        }
        resources.insert(time);
        tracing::debug!("TimePlugin: Registered Time resource");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EngineBuilder;

    #[test]
    fn test_time_plugin_registers_time() {
        let engine = EngineBuilder::new()
            .add_plugin(TimePlugin::default())
            .build();

        assert!(engine.get::<Time>().is_some());
    }

    #[test]
    fn test_time_starts_at_zero() {
        let engine = EngineBuilder::new()
            .add_plugin(TimePlugin::default())
            .build();

        let time = engine.get::<Time>().unwrap();
        assert_eq!(time.frame_count(), 0);
        assert_eq!(time.time_scale(), 1.0);
    }

    #[test]
    fn test_time_plugin_custom_timestep() {
        let engine = EngineBuilder::new()
            .add_plugin(
                TimePlugin::with_fixed_timestep(Duration::from_millis(16))
                    .with_max_delta(Duration::from_millis(50)),
            )
            .build();

        let time = engine.get::<Time>().unwrap();
        assert_eq!(time.fixed_timestep(), Duration::from_millis(16));
        assert_eq!(time.max_delta(), Duration::from_millis(50));
    }
}
