//! Input plugin for handling keyboard, mouse, and gamepad input.

use crate::plugin::Plugin;
use crate::resource::Resources;

/// Plugin that provides input state management.
///
/// This plugin sets up input handling infrastructure for keyboard,
/// mouse, and gamepad input.
///
/// # Resources Provided
///
/// - `InputState` - Current state of all input devices
///
/// # Example
///
/// ```ignore
/// use astrelis::prelude::*;
///
/// let engine = Engine::builder()
///     .add_plugin(InputPlugin)
///     .build();
/// ```
pub struct InputPlugin;

impl Plugin for InputPlugin {
    type Dependencies = ();
    fn name(&self) -> &'static str {
        "InputPlugin"
    }

    fn build(&self, resources: &mut Resources) {
        resources.insert(astrelis_input::InputState::new());
        tracing::debug!("InputPlugin: Registered InputState");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EngineBuilder;

    #[test]
    fn test_input_plugin_registers_state() {
        let engine = EngineBuilder::new()
            .add_plugin(InputPlugin)
            .build();

        assert!(engine.get::<astrelis_input::InputState>().is_some());
    }
}
