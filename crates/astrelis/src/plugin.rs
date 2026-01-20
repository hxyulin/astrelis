//! Plugin system for extending engine functionality.
//!
//! Plugins are the primary way to add features to the engine.
//! Each plugin can register resources, set up systems, and hook
//! into the engine lifecycle.

use crate::resource::Resources;
use std::any::type_name;

/// Trait for compile-time type-safe plugin dependency specification.
///
/// This trait is implemented for plugin types and tuples of plugin types,
/// allowing dependencies to be checked at compile time.
///
/// # Example
///
/// ```ignore
/// impl Plugin for MyPlugin {
///     type Dependencies = (RenderPlugin, AssetPlugin);
///     // ...
/// }
/// ```
pub trait PluginSet {
    /// Returns the type names of all plugins in this set.
    fn names() -> Vec<&'static str>;
}

/// Empty dependency set (no dependencies).
impl PluginSet for () {
    fn names() -> Vec<&'static str> {
        vec![]
    }
}

/// Single plugin dependency.
impl<P: Plugin> PluginSet for P {
    fn names() -> Vec<&'static str> {
        vec![type_name::<P>()]
    }
}

/// Two plugin dependencies.
impl<P1: Plugin, P2: Plugin> PluginSet for (P1, P2) {
    fn names() -> Vec<&'static str> {
        vec![type_name::<P1>(), type_name::<P2>()]
    }
}

/// Three plugin dependencies.
impl<P1: Plugin, P2: Plugin, P3: Plugin> PluginSet for (P1, P2, P3) {
    fn names() -> Vec<&'static str> {
        vec![type_name::<P1>(), type_name::<P2>(), type_name::<P3>()]
    }
}

/// Four plugin dependencies.
impl<P1: Plugin, P2: Plugin, P3: Plugin, P4: Plugin> PluginSet for (P1, P2, P3, P4) {
    fn names() -> Vec<&'static str> {
        vec![
            type_name::<P1>(),
            type_name::<P2>(),
            type_name::<P3>(),
            type_name::<P4>(),
        ]
    }
}

/// Five plugin dependencies.
impl<P1: Plugin, P2: Plugin, P3: Plugin, P4: Plugin, P5: Plugin> PluginSet
    for (P1, P2, P3, P4, P5)
{
    fn names() -> Vec<&'static str> {
        vec![
            type_name::<P1>(),
            type_name::<P2>(),
            type_name::<P3>(),
            type_name::<P4>(),
            type_name::<P5>(),
        ]
    }
}

/// Six plugin dependencies.
impl<P1: Plugin, P2: Plugin, P3: Plugin, P4: Plugin, P5: Plugin, P6: Plugin> PluginSet
    for (P1, P2, P3, P4, P5, P6)
{
    fn names() -> Vec<&'static str> {
        vec![
            type_name::<P1>(),
            type_name::<P2>(),
            type_name::<P3>(),
            type_name::<P4>(),
            type_name::<P5>(),
            type_name::<P6>(),
        ]
    }
}

/// Object-safe plugin trait for runtime plugin management.
///
/// This trait is automatically implemented for all types that implement `Plugin`.
/// It allows plugins to be stored as trait objects (`Box<dyn PluginDyn>`).
pub trait PluginDyn: Send + Sync {
    /// Returns the unique name of this plugin.
    fn name(&self) -> &'static str;

    /// Returns the names of plugins this plugin depends on.
    fn dependencies(&self) -> Vec<&'static str>;

    /// Called when the plugin is added to the engine.
    fn build(&self, resources: &mut Resources);

    /// Called after all plugins have been built.
    fn finish(&self, resources: &mut Resources);

    /// Called when the plugin is removed from the engine.
    fn cleanup(&self, resources: &mut Resources);
}

/// Trait for engine plugins with compile-time type-safe dependencies.
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
///     type Dependencies = ();
///
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
/// Plugins can declare dependencies using the `Dependencies` associated type:
///
/// ```ignore
/// impl Plugin for MyPlugin {
///     type Dependencies = (RenderPlugin, AssetPlugin);
///     // ...
/// }
/// ```
///
/// This provides compile-time type checking of dependencies.
pub trait Plugin: Send + Sync + 'static {
    /// Type-safe plugin dependencies.
    ///
    /// Specify dependencies as a tuple of plugin types:
    /// - `()` for no dependencies
    /// - `P` for a single dependency
    /// - `(P1, P2)` for two dependencies
    /// - `(P1, P2, P3)` for three dependencies, etc.
    ///
    /// # Example
    ///
    /// ```ignore
    /// impl Plugin for MyPlugin {
    ///     type Dependencies = (RenderPlugin, AssetPlugin);
    ///     // ...
    /// }
    /// ```
    type Dependencies: PluginSet;

    /// Returns the unique name of this plugin.
    ///
    /// By default, this uses the type name. Override if you need a custom name.
    fn name(&self) -> &'static str {
        type_name::<Self>()
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

// Blanket implementation of PluginDyn for all Plugin types
impl<P: Plugin> PluginDyn for P {
    fn name(&self) -> &'static str {
        <Self as Plugin>::name(self)
    }

    fn dependencies(&self) -> Vec<&'static str> {
        P::Dependencies::names()
    }

    fn build(&self, resources: &mut Resources) {
        <Self as Plugin>::build(self, resources)
    }

    fn finish(&self, resources: &mut Resources) {
        <Self as Plugin>::finish(self, resources)
    }

    fn cleanup(&self, resources: &mut Resources) {
        <Self as Plugin>::cleanup(self, resources)
    }
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
    fn plugins(&self) -> Vec<Box<dyn PluginDyn>>;

    /// Returns the name of this plugin group.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Wrapper to make a PluginGroup usable as a single Plugin.
pub(crate) struct PluginGroupAdapter {
    /// Plugin group name for debugging
    #[allow(dead_code)]
    name: &'static str,
    plugins: Vec<Box<dyn PluginDyn>>,
}

impl PluginGroupAdapter {
    pub fn new(group: impl PluginGroup) -> Self {
        Self {
            name: group.name(),
            plugins: group.plugins(),
        }
    }

    pub fn into_plugins(self) -> Vec<Box<dyn PluginDyn>> {
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
    type Dependencies = ();

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
        type Dependencies = ();

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

        PluginDyn::build(&plugin, &mut resources);

        assert_eq!(*resources.get::<i32>().unwrap(), 42);
    }

    #[test]
    fn test_fn_plugin() {
        let plugin = FnPlugin::new("test", |resources| {
            resources.insert("hello".to_string());
        });

        let mut resources = Resources::new();
        PluginDyn::build(&plugin, &mut resources);

        assert_eq!(resources.get::<String>().unwrap(), "hello");
    }

    struct DependentPlugin;

    impl Plugin for DependentPlugin {
        type Dependencies = TestPlugin;

        fn name(&self) -> &'static str {
            "DependentPlugin"
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
        let plugin = DependentPlugin;
        let deps = PluginDyn::dependencies(&plugin);
        assert_eq!(deps, vec![type_name::<TestPlugin>()]);
    }
}
