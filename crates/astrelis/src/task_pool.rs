//! Async task execution pool.
//!
//! Provides a thread pool for executing async tasks in parallel.

use std::future::Future;
use std::sync::Arc;
use std::thread;

use async_executor::{Executor, Task};

/// A thread pool for executing async tasks.
///
/// The TaskPool provides a way to spawn async tasks that run on a background
/// thread pool, enabling parallel execution and non-blocking operations.
///
/// # Example
///
/// ```ignore
/// use astrelis::TaskPool;
///
/// let pool = TaskPool::new(4);
///
/// let task = pool.spawn(async {
///     // Some async work
///     42
/// });
///
/// // Continue with other work
///
/// let result = pollster::block_on(task);
/// assert_eq!(result, 42);
/// ```
pub struct TaskPool {
    executor: Arc<Executor<'static>>,
    threads: Vec<thread::JoinHandle<()>>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
}

impl TaskPool {
    /// Create a new task pool with the specified number of threads.
    ///
    /// # Panics
    ///
    /// Panics if num_threads is 0.
    pub fn new(num_threads: usize) -> Self {
        assert!(num_threads > 0, "TaskPool must have at least one thread");

        let executor = Arc::new(Executor::new());
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut threads = Vec::with_capacity(num_threads);

        for i in 0..num_threads {
            let exec = executor.clone();
            let shutdown_flag = shutdown.clone();

            let handle = thread::Builder::new()
                .name(format!("astrelis-task-{}", i))
                .spawn(move || {
                    while !shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                        // Run tasks until shutdown or no more tasks
                        if !exec.try_tick() {
                            // No tasks ready, sleep briefly
                            thread::sleep(std::time::Duration::from_millis(1));
                        }
                    }
                })
                .expect("Failed to spawn task pool thread");

            threads.push(handle);
        }

        tracing::debug!("TaskPool created with {} threads", num_threads);

        Self {
            executor,
            threads,
            shutdown,
        }
    }

    /// Create a task pool using the number of available CPU cores.
    pub fn with_num_cpus() -> Self {
        Self::new(num_cpus::get())
    }

    /// Create a task pool with a default number of threads.
    ///
    /// Uses max(1, num_cpus - 1) to leave one core free for the main thread.
    pub fn default_threads() -> Self {
        let num_threads = (num_cpus::get().saturating_sub(1)).max(1);
        Self::new(num_threads)
    }

    /// Spawn an async task on the pool.
    ///
    /// Returns a `Task` that can be awaited to get the result.
    pub fn spawn<T>(&self, future: impl Future<Output = T> + Send + 'static) -> Task<T>
    where
        T: Send + 'static,
    {
        self.executor.spawn(future)
    }

    /// Get the number of threads in this pool.
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    /// Shutdown the task pool and wait for all threads to finish.
    ///
    /// This will wait for currently executing tasks to complete, but will not
    /// execute any new tasks that are spawned after shutdown is called.
    pub fn shutdown(mut self) {
        tracing::debug!("Shutting down TaskPool with {} threads", self.threads.len());

        // Signal shutdown
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Wait for all threads to finish
        let threads = std::mem::take(&mut self.threads);
        for handle in threads {
            if let Err(e) = handle.join() {
                tracing::error!("Task pool thread panicked: {:?}", e);
            }
        }

        tracing::debug!("TaskPool shutdown complete");
    }
}

impl Default for TaskPool {
    fn default() -> Self {
        Self::default_threads()
    }
}

impl Drop for TaskPool {
    fn drop(&mut self) {
        // Signal shutdown
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_pool_creation() {
        let pool = TaskPool::new(2);
        assert_eq!(pool.thread_count(), 2);
    }

    #[test]
    fn test_spawn_and_await() {
        let pool = TaskPool::new(2);

        let task = pool.spawn(async { 42 });

        let result = pollster::block_on(task);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_multiple_tasks() {
        let pool = TaskPool::new(4);

        let tasks: Vec<_> = (0..10)
            .map(|i| pool.spawn(async move { i * 2 }))
            .collect();

        let results: Vec<_> = tasks
            .into_iter()
            .map(|t| pollster::block_on(t))
            .collect();

        assert_eq!(results, vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);
    }

    #[test]
    fn test_default_threads() {
        let pool = TaskPool::default_threads();
        assert!(pool.thread_count() >= 1);
        assert!(pool.thread_count() <= num_cpus::get());
    }

    #[test]
    #[should_panic(expected = "TaskPool must have at least one thread")]
    fn test_zero_threads_panics() {
        TaskPool::new(0);
    }

    #[test]
    fn test_shutdown() {
        let pool = TaskPool::new(2);

        // Spawn some tasks
        let _task1 = pool.spawn(async { 1 });
        let _task2 = pool.spawn(async { 2 });

        // Shutdown waits for tasks to complete
        pool.shutdown();
    }
}
