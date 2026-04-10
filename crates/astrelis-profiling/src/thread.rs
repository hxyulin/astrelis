//! Thread naming and spawning utilities for profiling.
//!
//! Named threads appear with human-readable labels in the profiler viewer
//! instead of raw thread IDs.

/// Spawns a named thread with automatic profiler thread naming.
///
/// The thread name appears in the profiler viewer (e.g., puffin)
/// instead of a raw thread ID. This is the preferred way to spawn
/// threads that should be visible in profiling output.
///
/// # Example
///
/// ```rust
/// let handle = astrelis_profiling::spawn_profiled("asset_loader", || {
///     // work that will appear under "asset_loader" in the profiler
/// });
/// handle.join().unwrap();
/// ```
pub fn spawn_profiled<F, T>(name: &str, f: F) -> std::thread::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let name_owned = name.to_owned();
    std::thread::Builder::new()
        .name(name_owned.clone())
        .spawn(move || {
            crate::set_thread_name(&name_owned);
            f()
        })
        .expect("failed to spawn profiled thread")
}

/// Configures profiling for a rayon thread pool worker.
///
/// Call this in `rayon::ThreadPoolBuilder::start_handler` to give
/// rayon workers human-readable names in the profiler.
///
/// # Example
///
/// ```ignore
/// rayon::ThreadPoolBuilder::new()
///     .start_handler(|index| astrelis_profiling::thread::configure_pool_thread("rayon", index))
///     .build_global()
///     .unwrap();
/// ```
pub fn configure_pool_thread(pool_name: &str, index: usize) {
    let name = format!("{pool_name}-{index}");
    crate::set_thread_name(&name);
}
