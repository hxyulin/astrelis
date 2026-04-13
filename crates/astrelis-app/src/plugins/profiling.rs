//! Profiling integration plugin.

use crate::plugin::Plugin;

/// Plugin that initializes the in-engine profiler.
///
/// Profiler frame marks are handled by the framework's event loop,
/// so this plugin currently serves as a registration point. Future
/// extensions may add profiler configuration here.
pub struct ProfilingPlugin;

impl Plugin for ProfilingPlugin {
    fn build(&self, _app: &mut crate::app::App) {
        // Profiling init and frame marks are handled directly by App::run()
        // and App::on_events_cleared(). This plugin exists for symmetry
        // and future configuration.
    }
}
