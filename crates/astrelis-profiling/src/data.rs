//! Core profiling data types.
//!
//! These types define the shape of the data stored in the global
//! [`Timeline`](crate::timeline::Timeline). Span identifiers are
//! explicit (`SpanId`) rather than implicit (stack position) so that
//! spans which begin and end on different threads — the async case —
//! fit the model without a rewrite.

use std::num::NonZeroU32;

/// Globally unique identifier for a single span instance.
///
/// Issued by the profiler's atomic counter on every `span_begin`.
/// Zero is reserved as a sentinel for "no span".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpanId(pub u64);

impl SpanId {
    /// Sentinel value meaning "no span".
    pub const NONE: SpanId = SpanId(0);
}

/// Identifier for a scope *site* — a `(name, file, line)` triple that
/// has been interned and deduplicated. Multiple span instances share
/// the same `ScopeId` when they come from the same call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScopeId(pub NonZeroU32);

/// Identifier for a string in the global string table (scope names,
/// counter names, thread names, categories).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StringId(pub NonZeroU32);

/// Identifier for a registered thread. Assigned on first use by the
/// profiler; stable for the thread's lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThreadId(pub u32);

/// Identifier for a GPU lane — currently one lane per wgpu queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GpuLaneId(pub u32);

/// Structured payload attached to a span.
///
/// Currently only carries strings, but the enum is open so future
/// extensions can add `U64`, `EntityId`, or bespoke typed payloads
/// without touching call sites.
#[derive(Clone, Debug, Default)]
pub enum SpanData {
    /// No payload.
    #[default]
    None,
    /// A short `&str` turned into an owned `String` at collection time.
    String(String),
}

/// A completed CPU span — one begin/end pair from a single thread.
#[derive(Clone, Debug)]
pub struct CpuSpan {
    /// Unique identifier for this span instance.
    pub id: SpanId,
    /// Scope site (dedup'd across instances).
    pub scope: ScopeId,
    /// Thread on which the span begin/end were recorded.
    pub thread: ThreadId,
    /// Parent span, if any. `None` at the top of a thread's stack.
    pub parent: Option<SpanId>,
    /// Begin timestamp, nanoseconds since profiler epoch.
    pub start_ns: u64,
    /// End timestamp, nanoseconds since profiler epoch.
    pub end_ns: u64,
    /// Optional structured payload.
    pub data: SpanData,
}

/// A completed GPU span — one begin/end pair on a GPU lane.
#[derive(Clone, Debug)]
pub struct GpuSpan {
    /// Unique identifier for this span instance.
    pub id: SpanId,
    /// Scope site (dedup'd across instances).
    pub scope: ScopeId,
    /// GPU lane (queue) on which the work ran.
    pub lane: GpuLaneId,
    /// Parent span, if any. Populated from the wgpu_profiler query tree.
    pub parent: Option<SpanId>,
    /// Begin timestamp, nanoseconds since profiler epoch (aligned to CPU clock).
    pub start_ns: u64,
    /// End timestamp, nanoseconds since profiler epoch (aligned to CPU clock).
    pub end_ns: u64,
}

/// A point-in-time counter sample.
#[derive(Clone, Debug)]
pub struct CounterSample {
    /// Counter name (interned).
    pub counter: StringId,
    /// Timestamp, nanoseconds since profiler epoch.
    pub ts_ns: u64,
    /// Sample value. All counters are stored as `f64` internally; the
    /// macro API accepts integers and floats transparently.
    pub value: f64,
}

/// A frame boundary mark on the global timeline.
#[derive(Clone, Copy, Debug)]
pub struct FrameMark {
    /// Monotonically increasing frame index.
    pub index: u64,
    /// Begin timestamp of the frame (the previous `frame_mark!` call,
    /// or profiler init for the first frame), nanoseconds.
    pub start_ns: u64,
    /// End timestamp of the frame (this `frame_mark!` call), nanoseconds.
    pub end_ns: u64,
}

/// A counter value accepted by `profile_counter!` and `profile_plot!`.
///
/// All variants convert to `f64` at record time.
#[derive(Clone, Copy, Debug)]
pub enum CounterValue {
    /// Unsigned 64-bit integer.
    U64(u64),
    /// Signed 64-bit integer.
    I64(i64),
    /// 64-bit floating-point.
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
impl From<u32> for CounterValue {
    fn from(v: u32) -> Self {
        Self::U64(v as u64)
    }
}
impl From<i32> for CounterValue {
    fn from(v: i32) -> Self {
        Self::I64(v as i64)
    }
}
impl From<f32> for CounterValue {
    fn from(v: f32) -> Self {
        Self::F64(v as f64)
    }
}

/// Converts any counter value to an `f64` for storage.
#[inline]
pub fn counter_to_f64(value: impl Into<CounterValue>) -> f64 {
    match value.into() {
        CounterValue::U64(v) => v as f64,
        CounterValue::I64(v) => v as f64,
        CounterValue::F64(v) => v,
    }
}
