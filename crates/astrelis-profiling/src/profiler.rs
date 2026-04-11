//! The global profiler singleton and the low-level entry points that
//! the public macros expand to.
//!
//! There is exactly one `Profiler` per process, lazily initialised the
//! first time any macro fires. All hot-path functions are `#[inline]`
//! and designed to be fast enough (~100 ns per scope) to leave in
//! production code.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use crate::clock::Clock;
use crate::data::{
    CounterValue, FrameMark, GpuLaneId, ScopeId, SpanId, StringId, ThreadId, counter_to_f64,
};
use crate::string_table::StringTable;
use crate::thread_local::{Event, ThreadBuffer, ThreadRegistry, ThreadState, with_state};
use crate::timeline::Timeline;

/// The global profiler singleton.
pub struct Profiler {
    /// Monotonic clock and CPU/GPU alignment.
    pub clock: Clock,
    /// Interned string table for scope names, counter names, etc.
    pub strings: StringTable,
    /// The global timeline behind a reader-writer lock. The
    /// aggregator writes at frame boundaries; the viewer reads.
    pub timeline: RwLock<Timeline>,
    /// Atomic source of fresh `SpanId`s. Starts at 1; 0 is reserved.
    span_id_gen: AtomicU64,
    /// Atomic source of fresh `ThreadId`s.
    thread_id_gen: AtomicU32,
    /// Atomic source of fresh `GpuLaneId`s.
    gpu_lane_gen: AtomicU32,
    /// Registry of every thread's event buffer.
    thread_registry: ThreadRegistry,
    /// Frame counter, incremented by every `frame_mark!` call.
    frame_counter: AtomicU64,
    /// `end_ns` of the most recent frame (or profiler init time for
    /// the very first frame). Used as the `start_ns` of the next
    /// frame mark.
    last_frame_end_ns: AtomicU64,
}

static PROFILER: OnceLock<Profiler> = OnceLock::new();

impl Profiler {
    fn new() -> Self {
        let clock = Clock::new();
        let init_ns = clock.now_ns();
        Self {
            clock,
            strings: StringTable::new(),
            timeline: RwLock::new(Timeline::new()),
            span_id_gen: AtomicU64::new(0),
            thread_id_gen: AtomicU32::new(0),
            gpu_lane_gen: AtomicU32::new(0),
            thread_registry: ThreadRegistry::new(),
            frame_counter: AtomicU64::new(0),
            last_frame_end_ns: AtomicU64::new(init_ns),
        }
    }

    /// Returns the global profiler, initialising it on first call.
    #[inline]
    pub fn get() -> &'static Profiler {
        PROFILER.get_or_init(Profiler::new)
    }

    /// Returns the global profiler if it has been initialised, else
    /// `None`. Useful for callers that want to avoid the initial
    /// construction.
    #[inline]
    pub fn try_get() -> Option<&'static Profiler> {
        PROFILER.get()
    }

    /// Allocates a fresh span id.
    #[inline]
    pub(crate) fn next_span_id(&self) -> SpanId {
        SpanId(self.span_id_gen.fetch_add(1, Ordering::Relaxed) + 1)
    }

    /// Allocates a fresh thread id.
    fn next_thread_id(&self) -> ThreadId {
        ThreadId(self.thread_id_gen.fetch_add(1, Ordering::Relaxed))
    }

    /// Allocates a fresh GPU lane id.
    pub fn next_gpu_lane_id(&self) -> GpuLaneId {
        GpuLaneId(self.gpu_lane_gen.fetch_add(1, Ordering::Relaxed))
    }
}

/// Initialises the global profiler if it has not been already. Safe
/// to call multiple times. Equivalent to forcing lazy construction.
pub fn init() {
    let _ = Profiler::get();
}

/// Explicit shutdown hook. Kept for symmetry with [`init`]; currently
/// a no-op since the profiler has no resources that require explicit
/// teardown.
pub fn finish() {}

/// Registers the current thread with the profiler and returns its
/// thread-local state. Called lazily from [`with_state`] on first
/// use.
pub(crate) fn register_this_thread() -> ThreadState {
    let p = Profiler::get();
    let thread_id = p.next_thread_id();
    let buffer = Arc::new(Mutex::new(ThreadBuffer {
        thread_id,
        events: Vec::with_capacity(256),
    }));
    p.thread_registry.register(buffer.clone());

    let name = std::thread::current()
        .name()
        .map(|s| s.to_owned())
        .unwrap_or_else(|| format!("thread-{}", thread_id.0));
    let name_id = p.strings.intern(&name);
    p.timeline.write().unwrap().register_thread(thread_id, name_id);

    ThreadState {
        thread_id,
        buffer,
        stack: Vec::with_capacity(16),
    }
}

/// Sets (or updates) the display name of the current thread.
pub fn set_thread_name(name: &str) {
    let p = Profiler::get();
    let name_id = p.strings.intern(name);
    with_state(|state| {
        p.timeline.write().unwrap().rename_thread(state.thread_id, name_id);
    });
}

/// RAII guard that records a span-end when dropped.
///
/// Returned by [`enter_scope`]; users never name this type directly.
#[must_use = "scope guard must be held for the duration of the span"]
pub struct ScopeGuard {
    span_id: SpanId,
}

impl Drop for ScopeGuard {
    #[inline]
    fn drop(&mut self) {
        let p = Profiler::get();
        let ts_ns = p.clock.now_ns();
        with_state(|state| {
            // Pop the stack: in well-formed RAII code the popped id
            // equals our `span_id`. Mismatches can only happen if the
            // user explicitly keeps guards alive past their lexical
            // scope; we accept that and pop-to-this-id.
            while let Some(top) = state.stack.pop() {
                if top == self.span_id {
                    break;
                }
            }
            let mut buf = state.buffer.lock().unwrap();
            buf.events.push(Event::End {
                id: self.span_id,
                ts_ns,
            });
        });
    }
}

/// Opens a scope and returns a guard that closes it on drop.
///
/// `cache` is a call-site-local `OnceLock<ScopeId>` used to memoize
/// the scope registration so the hot path never locks the timeline.
#[inline]
pub fn enter_scope(
    cache: &OnceLock<ScopeId>,
    name: &'static str,
    file: &'static str,
    line: u32,
) -> ScopeGuard {
    let p = Profiler::get();
    let scope_id = *cache.get_or_init(|| {
        let name_id = p.strings.intern(name);
        let mut t = p.timeline.write().unwrap();
        t.register_scope(name_id, file, line)
    });
    let span_id = p.next_span_id();
    let ts_ns = p.clock.now_ns();
    with_state(|state| {
        let parent = state.stack.last().copied();
        state.stack.push(span_id);
        let mut buf = state.buffer.lock().unwrap();
        buf.events.push(Event::Begin {
            id: span_id,
            scope: scope_id,
            ts_ns,
            parent,
        });
    });
    ScopeGuard { span_id }
}

/// Records a counter sample on the current thread's event buffer.
#[inline]
pub fn record_counter_value(name_cache: &OnceLock<StringId>, name: &'static str, value: f64) {
    let p = Profiler::get();
    let counter = *name_cache.get_or_init(|| p.strings.intern(name));
    let ts_ns = p.clock.now_ns();
    with_state(|state| {
        let mut buf = state.buffer.lock().unwrap();
        buf.events.push(Event::Counter {
            counter,
            ts_ns,
            value,
        });
    });
}

/// Shim called by the `profile_counter!` macro.
#[inline]
pub fn record_counter_shim(
    name_cache: &OnceLock<StringId>,
    name: &'static str,
    value: impl Into<CounterValue>,
) {
    record_counter_value(name_cache, name, counter_to_f64(value));
}

/// Marks a frame boundary. Drains every thread's event buffer,
/// writes the paired spans into the global timeline, appends a
/// [`FrameMark`], and applies the retention policy.
///
/// Must be called from any thread — typically the main loop — once
/// per frame. Called from the `new_frame` public API for backwards
/// compatibility and directly by the `frame_mark!` macro.
pub fn frame_mark() {
    let p = Profiler::get();

    let batches = p.thread_registry.drain_all();

    let end_ns = p.clock.now_ns();
    let start_ns = p.last_frame_end_ns.swap(end_ns, Ordering::Relaxed);
    let index = p.frame_counter.fetch_add(1, Ordering::Relaxed);

    let mut timeline = p.timeline.write().unwrap();
    for (thread_id, events) in batches {
        timeline.absorb_thread_events(thread_id, events);
    }
    timeline.push_frame_mark(FrameMark {
        index,
        start_ns,
        end_ns,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn span_ids_are_unique_and_nonzero_under_concurrency() {
        // This test uses the global profiler. Since the profiler is a
        // process-wide singleton, we can rely on it being live
        // without needing an explicit setup — any test that runs
        // earlier may have already touched it, which is fine.
        let p = Profiler::get();
        let n_threads = 8;
        let per_thread = 1000;
        let mut handles = Vec::new();
        for _ in 0..n_threads {
            handles.push(thread::spawn(move || {
                let p = Profiler::get();
                let mut ids = Vec::with_capacity(per_thread);
                for _ in 0..per_thread {
                    ids.push(p.next_span_id());
                }
                ids
            }));
        }

        let mut all_ids: Vec<SpanId> = Vec::new();
        for h in handles {
            all_ids.extend(h.join().unwrap());
        }
        // Every id is non-zero (SpanId::NONE is reserved).
        assert!(all_ids.iter().all(|id| id.0 != 0));
        // All ids are unique within this batch.
        let unique: HashSet<SpanId> = all_ids.iter().copied().collect();
        assert_eq!(unique.len(), all_ids.len());
        let _ = p; // silence unused warning if Profiler::get becomes lazy
    }

    #[test]
    fn register_this_thread_assigns_distinct_ids_to_distinct_threads() {
        // Each child thread calls into with_state via a profile_scope;
        // the thread_local registration path allocates a fresh id.
        let counter = Arc::new(std::sync::Mutex::new(HashSet::new()));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let counter = counter.clone();
            handles.push(thread::spawn(move || {
                crate::profiler::init();
                crate::thread_local::with_state(|s| {
                    counter.lock().unwrap().insert(s.thread_id);
                });
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(counter.lock().unwrap().len(), 4);
    }
}
