///! Time plugin for tracking frame timing and game time.

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
///
/// let engine = Engine::builder()
///     .add_plugin(TimePlugin)
///     .build();
///
/// // In your app:
/// fn update(&mut self, ctx: &mut AppCtx, time: &Time) {
///     let dt = time.delta_seconds();
///     position += velocity * dt; // Frame-independent movement
/// }
/// ```
pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn name(&self) -> &'static str {
        "TimePlugin"
    }

    fn build(&self, resources: &mut Resources) {
        resources.insert(Time::new());
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
            .add_plugin(TimePlugin)
            .build();

        assert!(engine.get::<Time>().is_some());
    }

    #[test]
    fn test_time_starts_at_zero() {
        let engine = EngineBuilder::new()
            .add_plugin(TimePlugin)
            .build();

        let time = engine.get::<Time>().unwrap();
        assert_eq!(time.frame_count(), 0);
        assert_eq!(time.time_scale(), 1.0);
    }
}
