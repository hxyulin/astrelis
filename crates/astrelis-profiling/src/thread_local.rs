//! Per-thread event collection.
//!
//! Each thread that uses profiling lazily registers itself and gets an
//! `Arc<Mutex<ThreadBuffer>>` that it pushes events into. A clone of
//! that `Arc` lives in the global [`ThreadRegistry`] so the frame
//! aggregator can drain every thread's events under a short-lived
//! lock.
//!
//! **Contention model.** The owning thread pushes under a `Mutex` on
//! the hot path. The aggregator only contends with owners for the
//! brief moment it swaps out the event buffer at a frame boundary,
//! which happens at most once per frame. A `std::sync::Mutex` is
//! uncontended in steady state (~20 ns on modern hardware), which is
//! acceptable. A lock-free swap could reduce this further if needed.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use crate::data::{ScopeId, SpanId, StringId, ThreadId};

/// A single event pushed by a profiled thread. Parents of begin events
/// and all end events are resolved into [`CpuSpan`](crate::data::CpuSpan)s
/// at frame aggregation time.
#[derive(Clone, Debug)]
pub(crate) enum Event {
    /// A span begin. Parent is the caller's own current-stack top at
    /// begin time — resolved on the pushing thread so async cases can
    /// later fill it explicitly.
    Begin {
        /// Globally-unique id issued by the profiler's atomic counter.
        id: SpanId,
        /// Interned scope site.
        scope: ScopeId,
        /// Timestamp in profiler-epoch nanoseconds.
        ts_ns: u64,
        /// Parent span id, if any.
        parent: Option<SpanId>,
    },
    /// A span end.
    End {
        /// Id of the corresponding `Begin`.
        id: SpanId,
        /// Timestamp in profiler-epoch nanoseconds.
        ts_ns: u64,
    },
    /// A counter / plot sample.
    Counter {
        /// Interned counter name.
        counter: StringId,
        /// Timestamp in profiler-epoch nanoseconds.
        ts_ns: u64,
        /// Sample value.
        value: f64,
    },
}

/// A per-thread buffer of pending events plus this thread's identity.
pub(crate) struct ThreadBuffer {
    /// Assigned at registration, stable for the thread's lifetime.
    pub thread_id: ThreadId,
    /// Events pushed since the last drain.
    pub events: Vec<Event>,
}


/// Thread-local state holding the shared buffer handle and the
/// current synchronous span stack.
pub(crate) struct ThreadState {
    pub thread_id: ThreadId,
    pub buffer: Arc<Mutex<ThreadBuffer>>,
    /// Stack of currently-open span ids on this thread. Pushed by
    /// `scope_begin`, popped by the RAII guard's `Drop`.
    pub stack: Vec<SpanId>,
}

thread_local! {
    static STATE: RefCell<Option<ThreadState>> = const { RefCell::new(None) };
}

/// Global registry of every thread's buffer handle, so the aggregator
/// can drain them independent of any thread-local access.
pub(crate) struct ThreadRegistry {
    inner: Mutex<Vec<Arc<Mutex<ThreadBuffer>>>>,
}

impl ThreadRegistry {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Vec::new()),
        }
    }

    pub fn register(&self, buffer: Arc<Mutex<ThreadBuffer>>) {
        self.inner.lock().unwrap().push(buffer);
    }

    /// Drains events from every registered thread's buffer.
    ///
    /// For each buffer, acquires its mutex briefly, swaps the event
    /// vec out, releases the mutex, and returns `(thread_id, events)`
    /// pairs to the caller. The caller (Timeline aggregator) owns
    /// the drained events.
    pub fn drain_all(&self) -> Vec<(ThreadId, Vec<Event>)> {
        let buffers = self.inner.lock().unwrap();
        let mut out = Vec::with_capacity(buffers.len());
        for buf in buffers.iter() {
            let mut guard = buf.lock().unwrap();
            if guard.events.is_empty() {
                continue;
            }
            let events = std::mem::take(&mut guard.events);
            let tid = guard.thread_id;
            drop(guard);
            out.push((tid, events));
        }
        out
    }
}

impl Default for ThreadRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs `f` with a mutable reference to this thread's [`ThreadState`],
/// lazily registering the thread on first use.
///
/// The registration closure is called only on first access; it
/// allocates a `ThreadBuffer`, pushes a clone of its `Arc` into the
/// global registry, and stores the other clone in thread-local state.
pub(crate) fn with_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut ThreadState) -> R,
{
    STATE.with(|slot| {
        let mut slot_mut = slot.borrow_mut();
        if slot_mut.is_none() {
            let state = crate::profiler::register_this_thread();
            *slot_mut = Some(state);
        }
        f(slot_mut.as_mut().unwrap())
    })
}
