//! Plugin system for extending engine functionality.
//!
//! Plugins are the primary way to add features to the engine.
//! Each plugin can register resources, set up systems, and hook
//! into the engine lifecycle.

use crate::resource::Resources;

/// Trait for engine plugins.
///
/// Plugins are building blocks that add functionality to the engine.
/// They can register resources, depend on other plugins, and hook
/// into the engine lifecycle.
///
/// # Example
///
/// ```
/// use astrelis::{Plugin, Resources};
///
/// struct MyPlugin;
///
/// impl Plugin for MyPlugin {
///     fn name(&self) -> &'static str {
///         "MyPlugin"
///     }
///
///     fn build(&self, resources: &mut Resources) {
///         // Register resources
///         resources.insert(MyResource::new());
///     }
/// }
///
/// struct MyResource {
///     value: i32,
/// }
///
/// impl MyResource {
///     fn new() -> Self {
///         Self { value: 42 }
///     }
/// }
/// ```
///
/// # Plugin Dependencies
///
/// Plugins can declare dependencies on other plugins by returning
/// their names from `dependencies()`. The engine will ensure that
/// dependencies are built before dependent plugins.
pub trait Plugin: Send + Sync {
    /// Returns the unique name of this plugin.
    ///
    /// This is used for dependency resolution and debugging.
    fn name(&self) -> &'static str;

    /// Returns the names of plugins this plugin depends on.
    ///
    /// The engine will ensure these plugins are built before this one.
    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    /// Called when the plugin is added to the engine.
    ///
    /// Use this to register resources and perform initial setup.
    fn build(&self, resources: &mut Resources);

    /// Called after all plugins have been built.
    ///
    /// Use this for cross-plugin setup that requires other plugins
    /// to be initialized first.
    #[allow(unused_variables)]
    fn finish(&self, resources: &mut Resources) {}

    /// Called when the plugin is removed from the engine.
    ///
    /// Use this for cleanup.
    #[allow(unused_variables)]
    fn cleanup(&self, resources: &mut Resources) {}
}

/// A plugin group that bundles multiple plugins together.
///
/// This is useful for creating plugin bundles that set up
/// common functionality.
///
/// # Example
///
/// ```ignore
/// use astrelis::{Plugin, PluginGroup, Resources};
///
/// struct DefaultPlugins;
///
/// impl PluginGroup for DefaultPlugins {
///     fn plugins(&self) -> Vec<Box<dyn Plugin>> {
///         vec![
///             Box::new(AssetPlugin),
///             Box::new(RenderPlugin),
///             Box::new(InputPlugin),
///         ]
///     }
/// }
/// ```
pub trait PluginGroup {
    /// Returns the plugins in this group.
    fn plugins(&self) -> Vec<Box<dyn Plugin>>;

    /// Returns the name of this plugin group.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Wrapper to make a PluginGroup usable as a single Plugin.
pub(crate) struct PluginGroupAdapter {
    name: &'static str,
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginGroupAdapter {
    pub fn new(group: impl PluginGroup) -> Self {
        Self {
            name: group.name(),
            plugins: group.plugins(),
        }
    }

    pub fn into_plugins(self) -> Vec<Box<dyn Plugin>> {
        self.plugins
    }
}

/// A function-based plugin for simple use cases.
///
/// # Example
///
/// ```
/// use astrelis::{FnPlugin, Resources, EngineBuilder};
///
/// let engine = EngineBuilder::new()
///     .add_plugin(FnPlugin::new("setup", |resources| {
///         resources.insert(42i32);
///     }))
///     .build();
/// ```
pub struct FnPlugin<F>
where
    F: Fn(&mut Resources) + Send + Sync + 'static,
{
    name: &'static str,
    build_fn: F,
}

impl<F> FnPlugin<F>
where
    F: Fn(&mut Resources) + Send + Sync + 'static,
{
    /// Create a new function-based plugin.
    pub fn new(name: &'static str, build_fn: F) -> Self {
        Self { name, build_fn }
    }
}

impl<F> Plugin for FnPlugin<F>
where
    F: Fn(&mut Resources) + Send + Sync + 'static,
{
    fn name(&self) -> &'static str {
        self.name
    }

    fn build(&self, resources: &mut Resources) {
        (self.build_fn)(resources);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlugin {
        value: i32,
    }

    impl Plugin for TestPlugin {
        fn name(&self) -> &'static str {
            "TestPlugin"
        }

        fn build(&self, resources: &mut Resources) {
            resources.insert(self.value);
        }
    }

    #[test]
    fn test_plugin_build() {
        let plugin = TestPlugin { value: 42 };
        let mut resources = Resources::new();

        plugin.build(&mut resources);

        assert_eq!(*resources.get::<i32>().unwrap(), 42);
    }

    #[test]
    fn test_fn_plugin() {
        let plugin = FnPlugin::new("test", |resources| {
            resources.insert("hello".to_string());
        });

        let mut resources = Resources::new();
        plugin.build(&mut resources);

        assert_eq!(resources.get::<String>().unwrap(), "hello");
    }

    struct DependentPlugin;

    impl Plugin for DependentPlugin {
        fn name(&self) -> &'static str {
            "DependentPlugin"
        }

        fn dependencies(&self) -> &[&'static str] {
            &["TestPlugin"]
        }

        fn build(&self, resources: &mut Resources) {
            // Double the value set by TestPlugin
            if let Some(val) = resources.get_mut::<i32>() {
                *val *= 2;
            }
        }
    }

    #[test]
    fn test_plugin_dependencies() {
        let deps = DependentPlugin.dependencies();
        assert_eq!(deps, &["TestPlugin"]);
    }
}
