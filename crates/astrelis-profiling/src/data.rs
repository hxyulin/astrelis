//! Core profiling data types.
//!
//! These types are always compiled regardless of which backend is active.
//! GPU backends produce [`GpuScope`] values; profiling backends consume them.

/// Nanosecond timestamp relative to an arbitrary epoch.
pub type NanoSecond = i64;

/// A completed GPU profiling scope.
///
/// Produced by GPU backend implementations after resolving timestamp queries.
/// Consumed by the active profiling backend (e.g., puffin, tracy) for display.
///
/// Scopes can be nested: a render pass scope may contain child scopes for
/// individual draw calls (when the GPU supports in-pass timestamps).
#[derive(Clone, Debug)]
pub struct GpuScope {
    /// Human-readable label for this scope (e.g., "shadow_pass", "draw_meshes").
    pub label: String,
    /// Start timestamp in nanoseconds.
    pub start_ns: NanoSecond,
    /// End timestamp in nanoseconds.
    pub end_ns: NanoSecond,
    /// Child scopes nested within this one (e.g., draw calls within a pass).
    pub nested: Vec<GpuScope>,
}

/// A profiling counter value.
///
/// Counters track discrete values that change over time, such as memory usage,
/// object counts, or cache hit rates.
#[derive(Clone, Debug)]
pub enum CounterValue {
    /// Unsigned 64-bit integer counter.
    U64(u64),
    /// Signed 64-bit integer counter.
    I64(i64),
    /// 64-bit floating-point counter.
    F64(f64),
}

impl From<u64> for CounterValue {
    fn from(v: u64) -> Self {
        Self::U64(v)
    }
}

impl From<i64> for CounterValue {
    fn from(v: i64) -> Self {
        Self::I64(v)
    }
}

impl From<f64> for CounterValue {
    fn from(v: f64) -> Self {
        Self::F64(v)
    }
}

impl From<usize> for CounterValue {
    fn from(v: usize) -> Self {
        Self::U64(v as u64)
    }
}

impl From<f32> for CounterValue {
    fn from(v: f32) -> Self {
        Self::F64(v as f64)
    }
}
