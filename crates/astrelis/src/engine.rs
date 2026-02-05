//! Engine core - manages plugins and resources.

use std::collections::HashSet;

use astrelis_core::profiling::{profile_function, profile_scope};

use crate::plugin::{Plugin, PluginDyn, PluginGroup, PluginGroupAdapter};
use crate::resource::Resources;

/// Errors that can occur during engine construction.
#[derive(Debug, Clone)]
pub enum EngineError {
    /// A circular dependency was detected between plugins.
    CircularDependency {
        /// The plugin that caused the circular dependency to be detected.
        plugin: &'static str,
        /// The chain of plugins involved in the cycle (if available).
        chain: Vec<&'static str>,
    },
    /// A plugin dependency was not found.
    MissingDependency {
        /// The plugin that has the missing dependency.
        plugin: &'static str,
        /// The name of the missing dependency.
        dependency: &'static str,
    },
    /// A plugin with the same name was added twice.
    DuplicatePlugin {
        /// The name of the duplicate plugin.
        name: &'static str,
    },
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::CircularDependency { plugin, chain } => {
                write!(
                    f,
                    "Circular dependency detected involving plugin '{}'. Chain: {:?}",
                    plugin, chain
                )
            }
            EngineError::MissingDependency { plugin, dependency } => {
                write!(
                    f,
                    "Plugin '{}' requires dependency '{}' which was not added",
                    plugin, dependency
                )
            }
            EngineError::DuplicatePlugin { name } => {
                write!(f, "Plugin '{}' was added more than once", name)
            }
        }
    }
}

impl std::error::Error for EngineError {}

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
        profile_function!();
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
    ///
    /// # Errors
    ///
    /// Returns `EngineError` if:
    /// - A circular dependency is detected between plugins
    /// - A plugin dependency is not found
    /// - A plugin with the same name is added twice
    pub fn try_build(mut self) -> Result<Engine, EngineError> {
        profile_function!();
        // Sort plugins by dependencies and collect indices
        let sorted_indices = self.try_sort_plugins_by_dependency_indices()?;

        // Track plugin names
        let mut plugin_names = HashSet::new();

        // Build phase - call build() on each plugin
        profile_scope!("plugin_build_phase");
        for &idx in &sorted_indices {
            let plugin = &self.plugins[idx];
            tracing::debug!("Building plugin: {}", plugin.name());
            plugin.build(&mut self.resources);
            plugin_names.insert(plugin.name());
        }

        // Finish phase - call finish() on each plugin
        profile_scope!("plugin_finish_phase");
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

        Ok(Engine {
            resources: self.resources,
            plugin_names,
            plugins: sorted_plugins,
        })
    }

    /// Build the engine, panicking on error.
    ///
    /// This is a convenience method for examples and tests where error handling
    /// is not needed. For production code, prefer `try_build()` which returns
    /// a `Result`.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - A circular dependency is detected between plugins
    /// - A plugin dependency is not found
    /// - A plugin with the same name is added twice
    pub fn build(self) -> Engine {
        profile_function!();
        self.try_build().expect("Failed to build engine")
    }

    /// Reorder plugins according to sorted indices
    fn reorder_plugins(
        plugins: Vec<Box<dyn PluginDyn>>,
        sorted_indices: &[usize],
    ) -> Vec<Box<dyn PluginDyn>> {
        // Wrap in Option to allow taking elements
        let mut plugins_opt: Vec<Option<Box<dyn PluginDyn>>> =
            plugins.into_iter().map(Some).collect();

        // Extract in sorted order
        sorted_indices
            .iter()
            .map(|&idx| plugins_opt[idx].take().expect("Plugin already taken"))
            .collect()
    }

    /// Sort plugins by dependencies using topological sort, returning indices.
    ///
    /// # Errors
    ///
    /// Returns `EngineError` if:
    /// - A circular dependency is detected
    /// - A plugin dependency is not found among registered plugins
    /// - Duplicate plugin names are detected
    fn try_sort_plugins_by_dependency_indices(&self) -> Result<Vec<usize>, EngineError> {
        profile_function!();
        // Check for duplicate plugin names
        let mut seen_names = HashSet::new();
        for plugin in &self.plugins {
            if !seen_names.insert(plugin.name()) {
                return Err(EngineError::DuplicatePlugin {
                    name: plugin.name(),
                });
            }
        }

        // Create a name -> index map
        let plugin_map: std::collections::HashMap<_, _> = self
            .plugins
            .iter()
            .enumerate()
            .map(|(i, p)| (p.name(), i))
            .collect();

        // Check for missing dependencies
        for plugin in &self.plugins {
            for dep in plugin.dependencies() {
                if !plugin_map.contains_key(dep) {
                    return Err(EngineError::MissingDependency {
                        plugin: plugin.name(),
                        dependency: dep,
                    });
                }
            }
        }

        // Topological sort with cycle detection
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        let mut visit_stack = Vec::new();

        fn visit(
            name: &'static str,
            plugins: &[Box<dyn PluginDyn>],
            plugin_map: &std::collections::HashMap<&'static str, usize>,
            visited: &mut HashSet<&'static str>,
            visiting: &mut HashSet<&'static str>,
            visit_stack: &mut Vec<&'static str>,
            sorted: &mut Vec<usize>,
        ) -> Result<(), EngineError> {
            if visited.contains(name) {
                return Ok(());
            }

            if visiting.contains(name) {
                // Build the cycle chain
                let cycle_start = visit_stack.iter().position(|&n| n == name).unwrap_or(0);
                let mut chain: Vec<&'static str> = visit_stack[cycle_start..].to_vec();
                chain.push(name);
                return Err(EngineError::CircularDependency {
                    plugin: name,
                    chain,
                });
            }

            if let Some(&idx) = plugin_map.get(name) {
                visiting.insert(name);
                visit_stack.push(name);

                // Visit dependencies first
                for dep in plugins[idx].dependencies() {
                    visit(
                        dep,
                        plugins,
                        plugin_map,
                        visited,
                        visiting,
                        visit_stack,
                        sorted,
                    )?;
                }

                visit_stack.pop();
                visiting.remove(name);
                visited.insert(name);
                sorted.push(idx);
            }

            Ok(())
        }

        for plugin in &self.plugins {
            visit(
                plugin.name(),
                &self.plugins,
                &plugin_map,
                &mut visited,
                &mut visiting,
                &mut visit_stack,
                &mut sorted,
            )?;
        }

        Ok(sorted)
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
            .add_plugin(TestPlugin {
                name: "First",
                log: log1,
            })
            .add_plugin(TestPlugin {
                name: "Second",
                log: log2,
            })
            .add_plugin(TestPlugin {
                name: "Third",
                log: log3,
            })
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
