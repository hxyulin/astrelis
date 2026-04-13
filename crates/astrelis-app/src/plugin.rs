//! Plugin trait for modular engine extensions.

use crate::app::App;

/// A modular extension that registers resources, systems, and events.
///
/// Plugins are the primary mechanism for adding functionality to the
/// application. They receive a mutable reference to the [`App`] builder
/// during setup and can register resources, systems, events, and
/// sub-plugins.
///
/// # Example
///
/// ```ignore
/// use astrelis_app::{App, Plugin, Phase};
///
/// struct HealthPlugin;
///
/// impl Plugin for HealthPlugin {
///     fn build(&self, app: &mut App) {
///         app.insert_resource(HealthSystem::new());
///         app.add_system(Phase::Update, update_health);
///     }
/// }
/// ```
pub trait Plugin {
    /// Registers this plugin's resources, systems, and events with the app.
    fn build(&self, app: &mut App);
}
