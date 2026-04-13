//! Input state management plugin.

use astrelis_input::InputState;

use crate::phase::Phase;
use crate::plugin::Plugin;

/// Plugin that provides polling-style input state.
///
/// Inserts an [`InputState`] resource and calls `begin_frame()` at
/// the start of each frame. Window and device events are fed
/// automatically by the framework's event dispatch.
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut crate::app::App) {
        app.insert_resource(InputState::new());

        app.add_system(Phase::PreUpdate, |resources| {
            let mut input = resources.get_mut::<InputState>();
            input.begin_frame();
        });
    }
}
