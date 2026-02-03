//! Async runtime plugin for background task execution.

use crate::plugin::Plugin;
use crate::resource::Resources;
use crate::task_pool::TaskPool;

/// Plugin that provides async task execution.
///
/// This plugin registers a `TaskPool` resource that can be used to spawn
/// async tasks for parallel execution. The task pool runs on background threads
/// and is suitable for:
///
/// - Async asset loading
/// - Network requests
/// - File I/O
/// - Computationally expensive operations
///
/// # Resources Provided
///
/// - `TaskPool` - Thread pool for executing async tasks
///
/// # Example
///
/// ```ignore
/// use astrelis::prelude::*;
///
/// let engine = Engine::builder()
///     .add_plugin(AsyncRuntimePlugin::default())
///     .build();
///
/// let pool = engine.get::<TaskPool>().unwrap();
/// let task = pool.spawn(async {
///     // Async work here
///     42
/// });
/// ```
#[derive(Default)]
pub struct AsyncRuntimePlugin {
    /// Number of threads for the task pool.
    /// If None, uses default (num_cpus - 1).
    pub num_threads: Option<usize>,
}


impl AsyncRuntimePlugin {
    /// Create a new async runtime plugin with default thread count.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of threads for the task pool.
    pub fn with_threads(mut self, num_threads: usize) -> Self {
        self.num_threads = Some(num_threads);
        self
    }
}

impl Plugin for AsyncRuntimePlugin {
    type Dependencies = ();

    fn name(&self) -> &'static str {
        "AsyncRuntimePlugin"
    }

    fn build(&self, resources: &mut Resources) {
        let pool = match self.num_threads {
            Some(n) => TaskPool::new(n),
            None => TaskPool::default_threads(),
        };

        tracing::debug!(
            "AsyncRuntimePlugin: Created TaskPool with {} threads",
            pool.thread_count()
        );

        resources.insert(pool);
    }

    fn cleanup(&self, resources: &mut Resources) {
        // Gracefully shutdown the task pool
        if let Some(pool) = resources.remove::<TaskPool>() {
            tracing::debug!("AsyncRuntimePlugin: Shutting down TaskPool");
            pool.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EngineBuilder;

    #[test]
    fn test_async_runtime_plugin_registers_task_pool() {
        let engine = EngineBuilder::new()
            .add_plugin(AsyncRuntimePlugin::default())
            .build();

        assert!(engine.get::<TaskPool>().is_some());
    }

    #[test]
    fn test_async_runtime_plugin_with_custom_threads() {
        let engine = EngineBuilder::new()
            .add_plugin(AsyncRuntimePlugin::new().with_threads(2))
            .build();

        let pool = engine.get::<TaskPool>().unwrap();
        assert_eq!(pool.thread_count(), 2);
    }

    #[test]
    fn test_task_pool_spawn() {
        let engine = EngineBuilder::new()
            .add_plugin(AsyncRuntimePlugin::default())
            .build();

        let pool = engine.get::<TaskPool>().unwrap();

        let task = pool.spawn(async { 42 });
        let result = pollster::block_on(task);

        assert_eq!(result, 42);
    }
}
