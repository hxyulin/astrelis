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
///     .add_plugin(InputPlugin::default())
///     .build();
/// ```
pub struct InputPlugin {
    _private: (),
}

impl InputPlugin {
    /// Create a new InputPlugin with default settings.
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for InputPlugin {
    fn default() -> Self {
        Self::new()
    }
}

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
            .add_plugin(InputPlugin::default())
            .build();

        assert!(engine.get::<astrelis_input::InputState>().is_some());
    }
}
