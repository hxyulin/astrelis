//! Text rendering plugin.

use crate::plugin::Plugin;
use crate::resource::Resources;

use astrelis_text::FontSystem;

/// Plugin that provides text rendering capabilities.
///
/// This plugin sets up the font system for text rendering.
/// Note: For actual rendering, you also need the RenderPlugin
/// and to create a FontRenderer with a GraphicsContext.
///
/// # Resources Provided
///
/// - `FontSystem` - Font management and text shaping
///
/// # Example
///
/// ```ignore
/// use astrelis::prelude::*;
///
/// let engine = Engine::builder()
///     .add_plugin(TextPlugin::default())
///     .build();
///
/// let font_system = engine.get::<FontSystem>().unwrap();
/// ```
pub struct TextPlugin {
    /// Whether to load system fonts automatically.
    pub load_system_fonts: bool,
}

impl Default for TextPlugin {
    fn default() -> Self {
        Self {
            load_system_fonts: true,
        }
    }
}

impl TextPlugin {
    /// Create a new text plugin.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to load system fonts.
    pub fn with_system_fonts(mut self, load: bool) -> Self {
        self.load_system_fonts = load;
        self
    }
}

impl Plugin for TextPlugin {
    type Dependencies = ();
    fn name(&self) -> &'static str {
        "TextPlugin"
    }

    fn build(&self, resources: &mut Resources) {
        let font_system = if self.load_system_fonts {
            FontSystem::with_system_fonts()
        } else {
            FontSystem::new(astrelis_text::FontDatabase::empty())
        };

        tracing::debug!(
            "TextPlugin: FontSystem created (system fonts: {})",
            self.load_system_fonts
        );

        resources.insert(font_system);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EngineBuilder;

    #[test]
    fn test_text_plugin_registers_font_system() {
        let engine = EngineBuilder::new()
            .add_plugin(TextPlugin::default())
            .build();

        assert!(engine.get::<FontSystem>().is_some());
    }

    #[test]
    fn test_text_plugin_no_system_fonts() {
        let engine = EngineBuilder::new()
            .add_plugin(TextPlugin::new().with_system_fonts(false))
            .build();

        assert!(engine.get::<FontSystem>().is_some());
    }
}
