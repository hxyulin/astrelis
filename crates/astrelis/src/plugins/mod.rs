//! Default plugins for the Astrelis engine.
//!
//! This module contains the built-in plugins that provide common
//! game engine functionality.

mod asset_plugin;

#[cfg(all(feature = "render", feature = "winit"))]
mod render_plugin;

#[cfg(feature = "input")]
mod input_plugin;

#[cfg(feature = "text")]
mod text_plugin;

pub use asset_plugin::AssetPlugin;

#[cfg(all(feature = "render", feature = "winit"))]
pub use render_plugin::RenderPlugin;

#[cfg(all(feature = "render", feature = "winit"))]
pub use render_plugin::RenderContexts;

#[cfg(feature = "input")]
pub use input_plugin::InputPlugin;

#[cfg(feature = "text")]
pub use text_plugin::TextPlugin;

use crate::plugin::{Plugin, PluginGroup};

/// Default plugins for a minimal game setup.
///
/// Includes:
/// - `AssetPlugin` - Asset loading and management
///
/// # Example
///
/// ```ignore
/// use astrelis::{Engine, DefaultPlugins};
///
/// let engine = Engine::builder()
///     .add_plugins(DefaultPlugins)
///     .build();
/// ```
pub struct MinimalPlugins;

impl PluginGroup for MinimalPlugins {
    fn plugins(&self) -> Vec<Box<dyn Plugin>> {
        vec![Box::new(AssetPlugin::default())]
    }

    fn name(&self) -> &'static str {
        "MinimalPlugins"
    }
}

/// Default plugins for a typical game.
///
/// Includes:
/// - `AssetPlugin` - Asset loading and management
/// - `InputPlugin` - Input state management
///
/// # Example
///
/// ```ignore
/// use astrelis::{Engine, DefaultPlugins};
///
/// let engine = Engine::builder()
///     .add_plugins(DefaultPlugins)
///     .build();
/// ```
pub struct DefaultPlugins;

impl PluginGroup for DefaultPlugins {
    fn plugins(&self) -> Vec<Box<dyn Plugin>> {
        let mut plugins: Vec<Box<dyn Plugin>> = vec![Box::new(AssetPlugin::default())];

        #[cfg(feature = "input")]
        plugins.push(Box::new(InputPlugin));

        plugins
    }

    fn name(&self) -> &'static str {
        "DefaultPlugins"
    }
}
