//! Custom in-engine profiler for the Astrelis game engine.
//!
//! `astrelis-profiling` collects CPU and GPU timing data into a
//! global [`Timeline`](crate::timeline::Timeline) that can be read
//! by an in-process viewer (see the `astrelis-profiling-egui`
//! crate). It replaces the previous Tracy-based backend entirely —
//! there is no external profiler binary to attach; everything lives
//! in-engine.
//!
//! # Overview
//!
//! ```rust
//! fn update_physics() {
//!     astrelis_profiling::profile_function!();
//!     {
//!         astrelis_profiling::profile_scope!("broad_phase");
//!         // ...
//!     }
//! }
//!
//! fn main_loop() {
//!     astrelis_profiling::init();
//!     for _ in 0..60 {
//!         astrelis_profiling::new_frame();
//!         update_physics();
//!     }
//! }
//! ```
//!
//! # Data model
//!
//! All profiling data lives on a single global timeline keyed by
//! absolute nanoseconds. Spans are identified by `SpanId` rather
//! than by stack position so the same API can later extend to
//! async spans that begin on one thread and end on another. Frames
//! are *marks* on the timeline, not containers, and the retention
//! policy keeps a rolling window of recent frames.
//!
//! # Performance
//!
//! The hot path for `profile_scope!` and `profile_function!` costs
//! roughly ~100 ns per scope on modern hardware: one `OnceLock`
//! read, one atomic increment, one `Instant::now`, one thread-local
//! access, one uncontended mutex lock, and one `Vec` push.

#![warn(missing_docs)]

pub mod clock;
pub mod data;
pub mod gpu;
pub mod profiler;
pub mod string_table;
pub mod thread;
pub(crate) mod thread_local;
pub mod timeline;

// ============================================================================
// Public API — backwards-compatible function surface
// ============================================================================

pub use profiler::{Profiler, ScopeGuard, finish, frame_mark, init, set_thread_name};
pub use thread::{configure_pool_thread, spawn_profiled};

/// Signals a frame boundary. Equivalent to calling [`frame_mark`].
///
/// Kept as a free function for backwards compatibility with code
/// that called the older `new_frame` API.
#[inline]
pub fn new_frame() {
    profiler::frame_mark();
}

// ============================================================================
// Macro-reachable internals
// ============================================================================

/// Items reachable from macros but not part of the stable public API.
/// Users should never reference anything in this module directly.
#[doc(hidden)]
pub mod private {
    pub use std::sync::OnceLock;

    pub use crate::data::{CounterValue, ScopeId, StringId};
    pub use crate::profiler::{
        ScopeGuard, enter_scope, record_counter_shim, record_counter_value,
    };
}

// ============================================================================
// Macros
// ============================================================================

/// Profiles a named scope within a function.
///
/// The scope begins at the macro call site and ends when the returned
/// guard is dropped (at the end of the enclosing block).
///
/// A per-call-site `OnceLock<ScopeId>` caches the scope registration,
/// so only the *first* invocation of a given call site touches the
/// timeline write lock. Subsequent invocations pay one atomic load
/// plus the per-scope hot-path cost (see crate-level docs).
///
/// # Example
///
/// ```rust
/// fn process() {
///     astrelis_profiling::profile_scope!("step_1");
///     // ...
/// }
/// ```
#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {
        // The `static` is placed inside a block expression so that
        // multiple `profile_scope!` invocations within the same
        // enclosing scope do not collide on a single item name.
        // The returned guard is bound by the outer `let`, so its
        // drop point is the enclosing block — matching the old
        // `profiling::scope!` RAII semantics.
        let _astrelis_scope_guard = {
            static CACHE: $crate::private::OnceLock<$crate::private::ScopeId> =
                $crate::private::OnceLock::new();
            $crate::private::enter_scope(&CACHE, $name, file!(), line!())
        };
    };
    ($name:expr, $_data:expr) => {
        // Accept an optional second argument for forward compatibility
        // with structured span data; currently unused.
        $crate::profile_scope!($name);
    };
}

/// Profiles the enclosing function. The scope name is inferred from
/// `std::any::type_name` on a local nested function, giving the fully
/// qualified path of the caller.
///
/// # Example
///
/// ```rust
/// fn my_function() {
///     astrelis_profiling::profile_function!();
///     // ...
/// }
/// ```
#[macro_export]
macro_rules! profile_function {
    () => {
        // See `profile_scope!` for the reason the `static` lives
        // inside the `let` expression rather than at statement level.
        let _astrelis_fn_guard = {
            static CACHE: $crate::private::OnceLock<$crate::private::ScopeId> =
                $crate::private::OnceLock::new();
            fn __astrelis_f() {}
            fn type_name_of<T>(_: T) -> &'static str {
                ::std::any::type_name::<T>()
            }
            // `type_name_of(__astrelis_f)` returns something like
            // `crate::module::my_function::__astrelis_f`. Strip the
            // trailing "::__astrelis_f" to get the caller's path.
            const SUFFIX_LEN: usize = "::__astrelis_f".len();
            let full = type_name_of(__astrelis_f);
            let name = &full[..full.len() - SUFFIX_LEN];
            $crate::private::enter_scope(&CACHE, name, file!(), line!())
        };
    };
    ($_data:expr) => {
        $crate::profile_function!();
    };
}

/// Names the current thread for the profiler. Call at the start of
/// thread entry points (e.g. pool workers, rayon callbacks).
#[macro_export]
macro_rules! profile_thread {
    ($name:expr) => {
        $crate::set_thread_name($name);
    };
}

/// Records a counter sample on the current thread.
///
/// The `category` argument is accepted but currently unused. The
/// name is interned once per call site via a `OnceLock<StringId>`
/// cache.
///
/// # Example
///
/// ```rust
/// astrelis_profiling::profile_counter!("gpu_memory", "buffer_bytes", 1024u64);
/// ```
#[macro_export]
macro_rules! profile_counter {
    ($_category:expr, $name:expr, $value:expr) => {{
        static CACHE: $crate::private::OnceLock<$crate::private::StringId> =
            $crate::private::OnceLock::new();
        $crate::private::record_counter_shim(&CACHE, $name, $value);
    }};
}

/// Records a plot sample on the current thread. Equivalent to
/// `profile_counter!` without the category field.
///
/// # Example
///
/// ```rust
/// astrelis_profiling::profile_plot!("frame_time_ms", 16.3);
/// ```
#[macro_export]
macro_rules! profile_plot {
    ($name:expr, $value:expr) => {{
        static CACHE: $crate::private::OnceLock<$crate::private::StringId> =
            $crate::private::OnceLock::new();
        $crate::private::record_counter_value(&CACHE, $name, ($value) as f64);
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_function_compiles() {
        profile_function!();
    }

    #[test]
    fn profile_function_with_data_compiles() {
        profile_function!("some data");
    }

    #[test]
    fn profile_scope_compiles() {
        profile_scope!("test_scope");
    }

    #[test]
    fn profile_scope_with_data_compiles() {
        profile_scope!("test_scope", "extra data");
    }

    #[test]
    fn lifecycle_functions_are_callable() {
        init();
        new_frame();
        finish();
    }

    #[test]
    fn counter_macro_compiles() {
        profile_counter!("test", "counter", 42u64);
        profile_counter!("test", "float_counter", 2.5f64);
    }

    #[test]
    fn plot_macro_compiles() {
        profile_plot!("test_metric", 1.0);
    }

    #[test]
    fn scope_spans_are_recorded() {
        init();
        {
            profile_scope!("recorded_scope");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        new_frame();
        let p = Profiler::get();
        let timeline = p.timeline.read().unwrap();
        let has_span = timeline
            .thread_streams
            .values()
            .any(|s| !s.spans.is_empty());
        assert!(has_span, "expected at least one recorded span after frame_mark");
    }
}
