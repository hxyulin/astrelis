//! Time tracking plugin.

use crate::plugin::Plugin;
use crate::time::Time;

/// Plugin that provides frame timing and fixed-timestep management.
///
/// Inserts a [`Time`] resource. The framework updates it automatically
/// at the start of each frame.
pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut crate::app::App) {
        app.insert_resource(Time::new());
    }
}
