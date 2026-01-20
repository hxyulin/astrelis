//! Engine core - manages plugins and resources.

use std::collections::HashSet;

use crate::plugin::{Plugin, PluginDyn, PluginGroup, PluginGroupAdapter};
use crate::resource::Resources;

/// The main engine struct that holds all resources and manages plugins.
///
/// The engine is typically created using `EngineBuilder` and then
/// used throughout the application lifetime.
///
/// # Example
///
/// ```
/// use astrelis::{Engine, EngineBuilder, FnPlugin};
///
/// let engine = EngineBuilder::new()
///     .add_plugin(FnPlugin::new("setup", |resources| {
///         resources.insert(42i32);
///     }))
///     .build();
///
/// assert_eq!(*engine.resources().get::<i32>().unwrap(), 42);
/// ```
pub struct Engine {
    resources: Resources,
    plugin_names: HashSet<&'static str>,
    plugins: Vec<Box<dyn PluginDyn>>,
}

impl Engine {
    /// Create a new engine builder.
    pub fn builder() -> EngineBuilder {
        EngineBuilder::new()
    }

    /// Get a reference to the engine's resources.
    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    /// Get a mutable reference to the engine's resources.
    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    /// Get a resource by type.
    pub fn get<R: crate::resource::Resource>(&self) -> Option<&R> {
        self.resources.get::<R>()
    }

    /// Get a mutable resource by type.
    pub fn get_mut<R: crate::resource::Resource>(&mut self) -> Option<&mut R> {
        self.resources.get_mut::<R>()
    }

    /// Check if a plugin is registered.
    pub fn has_plugin(&self, name: &str) -> bool {
        self.plugin_names.contains(name)
    }

    /// Get the names of all registered plugins.
    pub fn plugin_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.plugin_names.iter().copied()
    }

    /// Shutdown the engine, calling cleanup() on all plugins in reverse order.
    ///
    /// This allows plugins to perform cleanup tasks such as:
    /// - Flushing assets to disk
    /// - Closing file handles
    /// - Persisting state
    /// - Releasing resources
    ///
    /// Plugins are cleaned up in reverse dependency order (opposite of build order).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use astrelis::prelude::*;
    ///
    /// let mut engine = Engine::builder()
    ///     .add_plugins(DefaultPlugins)
    ///     .build();
    ///
    /// // ... game runs ...
    ///
    /// engine.shutdown(); // Cleanup all plugins
    /// ```
    pub fn shutdown(&mut self) {
        tracing::info!("Shutting down engine with {} plugins", self.plugins.len());

        // Call cleanup() in reverse order (reverse of build order)
        for plugin in self.plugins.iter().rev() {
            tracing::debug!("Cleaning up plugin: {}", plugin.name());
            plugin.cleanup(&mut self.resources);
        }

        tracing::info!("Engine shutdown complete");
    }
}

impl Default for Engine {
    fn default() -> Self {
        EngineBuilder::new().build()
    }
}

/// Builder for constructing an Engine with plugins.
///
/// # Example
///
/// ```
/// use astrelis::{EngineBuilder, FnPlugin};
///
/// let engine = EngineBuilder::new()
///     .add_plugin(FnPlugin::new("config", |resources| {
///         resources.insert("game_title".to_string());
///     }))
///     .build();
/// ```
pub struct EngineBuilder {
    plugins: Vec<Box<dyn PluginDyn>>,
    resources: Resources,
}

impl EngineBuilder {
    /// Create a new engine builder.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            resources: Resources::new(),
        }
    }

    /// Add a plugin to the engine.
    ///
    /// Plugins are built in the order they are added, with dependency
    /// ordering handled automatically.
    pub fn add_plugin(mut self, plugin: impl Plugin + 'static) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Add a plugin group (multiple plugins) to the engine.
    pub fn add_plugins(mut self, group: impl PluginGroup) -> Self {
        let adapter = PluginGroupAdapter::new(group);
        for plugin in adapter.into_plugins() {
            self.plugins.push(plugin);
        }
        self
    }

    /// Insert a resource directly before building.
    ///
    /// This is useful for resources that plugins might depend on
    /// but that aren't provided by a plugin.
    pub fn insert_resource<R: crate::resource::Resource>(mut self, resource: R) -> Self {
        self.resources.insert(resource);
        self
    }

    /// Build the engine, initializing all plugins.
    ///
    /// This will:
    /// 1. Sort plugins by dependencies
    /// 2. Call `build()` on each plugin
    /// 3. Call `finish()` on each plugin
    pub fn build(mut self) -> Engine {
        // Sort plugins by dependencies and collect indices
        let sorted_indices = self.sort_plugins_by_dependency_indices();

        // Track plugin names
        let mut plugin_names = HashSet::new();

        // Build phase - call build() on each plugin
        for &idx in &sorted_indices {
            let plugin = &self.plugins[idx];
            tracing::debug!("Building plugin: {}", plugin.name());
            plugin.build(&mut self.resources);
            plugin_names.insert(plugin.name());
        }

        // Finish phase - call finish() on each plugin
        for &idx in &sorted_indices {
            self.plugins[idx].finish(&mut self.resources);
        }

        tracing::info!(
            "Engine built with {} plugins: {:?}",
            plugin_names.len(),
            plugin_names
        );

        // Reorder plugins vec to match build order for proper cleanup
        // We need plugins in dependency order so cleanup can reverse it
        let sorted_plugins = Self::reorder_plugins(self.plugins, &sorted_indices);

        Engine {
            resources: self.resources,
            plugin_names,
            plugins: sorted_plugins,
        }
    }

    /// Reorder plugins according to sorted indices
    fn reorder_plugins(plugins: Vec<Box<dyn PluginDyn>>, sorted_indices: &[usize]) -> Vec<Box<dyn PluginDyn>> {
        // Wrap in Option to allow taking elements
        let mut plugins_opt: Vec<Option<Box<dyn PluginDyn>>> =
            plugins.into_iter().map(|p| Some(p)).collect();

        // Extract in sorted order
        sorted_indices
            .iter()
            .map(|&idx| plugins_opt[idx].take().expect("Plugin already taken"))
            .collect()
    }

    /// Sort plugins by dependencies using topological sort, returning indices.
    fn sort_plugins_by_dependency_indices(&self) -> Vec<usize> {
        // Simple topological sort
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        // Create a name -> index map
        let plugin_map: std::collections::HashMap<_, _> = self
            .plugins
            .iter()
            .enumerate()
            .map(|(i, p)| (p.name(), i))
            .collect();

        fn visit(
            name: &'static str,
            plugins: &[Box<dyn PluginDyn>],
            plugin_map: &std::collections::HashMap<&'static str, usize>,
            visited: &mut HashSet<&'static str>,
            visiting: &mut HashSet<&'static str>,
            sorted: &mut Vec<usize>,
        ) {
            if visited.contains(name) {
                return;
            }

            if visiting.contains(name) {
                tracing::warn!("Circular plugin dependency detected involving: {}", name);
                return;
            }

            if let Some(&idx) = plugin_map.get(name) {
                visiting.insert(name);

                // Visit dependencies first
                for dep in plugins[idx].dependencies() {
                    visit(dep, plugins, plugin_map, visited, visiting, sorted);
                }

                visiting.remove(name);
                visited.insert(name);
                sorted.push(idx);
            }
        }

        for plugin in &self.plugins {
            visit(
                plugin.name(),
                &self.plugins,
                &plugin_map,
                &mut visited,
                &mut visiting,
                &mut sorted,
            );
        }

        sorted
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::FnPlugin;

    #[test]
    fn test_engine_builder() {
        let engine = EngineBuilder::new()
            .add_plugin(FnPlugin::new("test", |resources| {
                resources.insert(42i32);
            }))
            .build();

        assert_eq!(*engine.get::<i32>().unwrap(), 42);
        assert!(engine.has_plugin("test"));
    }

    #[test]
    fn test_insert_resource() {
        let engine = EngineBuilder::new()
            .insert_resource("pre-inserted".to_string())
            .build();

        assert_eq!(engine.get::<String>().unwrap(), "pre-inserted");
    }

    #[test]
    fn test_plugin_order() {
        struct FirstPlugin;
        impl Plugin for FirstPlugin {
            type Dependencies = ();
            fn build(&self, resources: &mut Resources) {
                resources.insert(vec!["first"]);
            }
        }

        struct SecondPlugin;
        impl Plugin for SecondPlugin {
            type Dependencies = FirstPlugin;

            fn build(&self, resources: &mut Resources) {
                if let Some(v) = resources.get_mut::<Vec<&'static str>>() {
                    v.push("second");
                }
            }
        }

        // Add in reverse order - should still work due to dependencies
        let engine = EngineBuilder::new()
            .add_plugin(SecondPlugin)
            .add_plugin(FirstPlugin)
            .build();

        let order = engine.get::<Vec<&'static str>>().unwrap();
        assert_eq!(order, &vec!["first", "second"]);
    }

    #[test]
    fn test_default_engine() {
        let engine = Engine::default();
        assert!(engine.resources().is_empty());
    }

    #[test]
    fn test_engine_shutdown() {
        use std::sync::{Arc, Mutex};

        // Track cleanup calls
        let cleanup_log = Arc::new(Mutex::new(Vec::new()));

        struct TestPlugin {
            name: &'static str,
            log: Arc<Mutex<Vec<&'static str>>>,
        }

        impl Plugin for TestPlugin {
            type Dependencies = ();
            fn name(&self) -> &'static str {
                self.name
            }

            fn build(&self, resources: &mut Resources) {
                resources.insert(format!("{}_built", self.name));
            }

            fn cleanup(&self, _resources: &mut Resources) {
                self.log.lock().unwrap().push(self.name);
            }
        }

        let log1 = cleanup_log.clone();
        let log2 = cleanup_log.clone();
        let log3 = cleanup_log.clone();

        let mut engine = EngineBuilder::new()
            .add_plugin(TestPlugin { name: "First", log: log1 })
            .add_plugin(TestPlugin { name: "Second", log: log2 })
            .add_plugin(TestPlugin { name: "Third", log: log3 })
            .build();

        // Verify resources were created
        assert!(engine.get::<String>().is_some());

        // Call shutdown
        engine.shutdown();

        // Verify cleanup was called in reverse order
        let log = cleanup_log.lock().unwrap();
        assert_eq!(*log, vec!["Third", "Second", "First"]);
    }

    #[test]
    fn test_shutdown_with_dependencies() {
        use std::sync::{Arc, Mutex};

        let cleanup_log = Arc::new(Mutex::new(Vec::new()));

        struct BasePlugin {
            log: Arc<Mutex<Vec<&'static str>>>,
        }

        impl Plugin for BasePlugin {
            type Dependencies = ();

            fn build(&self, resources: &mut Resources) {
                resources.insert(vec!["base"]);
            }

            fn cleanup(&self, _resources: &mut Resources) {
                self.log.lock().unwrap().push("BasePlugin");
            }
        }

        struct DependentPlugin {
            log: Arc<Mutex<Vec<&'static str>>>,
        }

        impl Plugin for DependentPlugin {
            type Dependencies = BasePlugin;

            fn build(&self, resources: &mut Resources) {
                if let Some(v) = resources.get_mut::<Vec<&'static str>>() {
                    v.push("dependent");
                }
            }

            fn cleanup(&self, _resources: &mut Resources) {
                self.log.lock().unwrap().push("DependentPlugin");
            }
        }

        let log1 = cleanup_log.clone();
        let log2 = cleanup_log.clone();

        let mut engine = EngineBuilder::new()
            .add_plugin(DependentPlugin { log: log2 })
            .add_plugin(BasePlugin { log: log1 })
            .build();

        engine.shutdown();

        // Cleanup should be in reverse order: DependentPlugin first, then BasePlugin
        let log = cleanup_log.lock().unwrap();
        assert_eq!(*log, vec!["DependentPlugin", "BasePlugin"]);
    }
}
