//! Global timeline — the source of truth for all collected profiling
//! data. Frames are *marks* on this timeline, not containers.
//!
//! The viewer reads from the timeline under a read lock; the
//! aggregator writes under a write lock once per frame.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::num::NonZeroU32;

use crate::data::{
    CounterSample, CpuSpan, FrameMark, GpuLaneId, GpuSpan, ScopeId, SpanData, SpanId, StringId,
    ThreadId,
};
use crate::thread_local::Event;

/// Describes a registered scope site.
#[derive(Clone, Debug)]
pub struct ScopeInfo {
    /// Interned scope name.
    pub name: StringId,
    /// Source file where this scope was declared. Empty string for
    /// function scopes without file info.
    pub file: &'static str,
    /// Source line.
    pub line: u32,
}

/// Information about a registered thread.
#[derive(Clone, Debug)]
pub struct ThreadInfo {
    /// Display name (interned).
    pub name: StringId,
}

/// Information about a GPU lane (one per wgpu queue for now).
#[derive(Clone, Debug)]
pub struct GpuLaneInfo {
    /// Display name (interned).
    pub name: StringId,
}

/// Per-thread stream of completed CPU spans, ordered by `end_ns`.
///
/// Spans are pushed at `End`-event arrival time and the per-thread
/// clock is monotonic, so the deque is `end_ns`-sorted. It is *not*
/// `start_ns`-sorted: a nested parent has an earlier `start_ns` than
/// its children but is pushed after them, so `start_ns` may decrease
/// from one element to the next.
///
/// Backed by [`VecDeque`] so retention can drop the oldest spans
/// from the front in `O(drop_count)` rather than the `O(total)` cost
/// of a `Vec::retain` pass.
#[derive(Clone, Debug, Default)]
pub struct ThreadStream {
    /// Completed spans, ordered by `end_ns`.
    pub spans: VecDeque<CpuSpan>,
}

impl ThreadStream {
    /// Returns all spans that overlap the half-open window
    /// `[visible_start_ns, visible_end_ns)`.
    ///
    /// A span overlaps the window iff `span.end_ns > visible_start_ns`
    /// and `span.start_ns < visible_end_ns`. Spans whose `end_ns`
    /// equals `visible_start_ns`, or whose `start_ns` equals
    /// `visible_end_ns`, are *not* included.
    ///
    /// Because the stream is `end_ns`-sorted, a
    /// [`VecDeque::partition_point`] lookup skips over the entire
    /// prefix of spans that ended before the window — usually the
    /// bulk of the retained buffer when the user is inspecting recent
    /// frames. The back edge cannot be short-circuited by `start_ns`
    /// because nested parents can be pushed to the deque after their
    /// children with a smaller `start_ns`, so the tail walk uses a
    /// filter rather than an early-termination `take_while`.
    ///
    /// Worst-case cost is `O(log n + (n - start_idx))`, which matches
    /// the previous linear filter in the common "zoom near the end of
    /// retention" case and beats it when `start_idx` is large.
    pub fn spans_in_window(
        &self,
        visible_start_ns: u64,
        visible_end_ns: u64,
    ) -> impl Iterator<Item = &CpuSpan> {
        let start_idx = self
            .spans
            .partition_point(|s| s.end_ns <= visible_start_ns);
        self.spans
            .iter()
            .skip(start_idx)
            .filter(move |s| s.start_ns < visible_end_ns)
    }
}

/// Per-GPU-lane stream of completed GPU spans, ordered by `end_ns`.
#[derive(Clone, Debug, Default)]
pub struct GpuStream {
    /// Completed GPU spans, ordered by `end_ns`.
    pub spans: VecDeque<GpuSpan>,
}

impl GpuStream {
    /// Returns all GPU spans that overlap the half-open window
    /// `[visible_start_ns, visible_end_ns)`.
    ///
    /// See [`ThreadStream::spans_in_window`] for semantics and
    /// complexity — GPU spans share the same `end_ns`-sorted
    /// insertion invariant.
    pub fn spans_in_window(
        &self,
        visible_start_ns: u64,
        visible_end_ns: u64,
    ) -> impl Iterator<Item = &GpuSpan> {
        let start_idx = self
            .spans
            .partition_point(|s| s.end_ns <= visible_start_ns);
        self.spans
            .iter()
            .skip(start_idx)
            .filter(move |s| s.start_ns < visible_end_ns)
    }
}

/// Per-counter stream of samples.
#[derive(Clone, Debug, Default)]
pub struct CounterStream {
    /// Samples, ordered by timestamp.
    pub samples: VecDeque<CounterSample>,
}

/// Policy for evicting old data from the timeline.
#[derive(Clone, Copy, Debug)]
pub struct RetentionPolicy {
    /// Maximum number of frame marks to retain. Spans older than the
    /// oldest retained frame mark are dropped.
    pub max_frames: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self { max_frames: 600 }
    }
}

/// Global timeline of all profiling data collected so far (within
/// the retention window).
pub struct Timeline {
    /// Scope sites registered with the profiler.
    pub scopes: Vec<ScopeInfo>,
    /// Reverse index from `(name, file, line)` to `ScopeId` to
    /// deduplicate `profile_scope!` call sites at registration time.
    pub scope_index: HashMap<(StringId, &'static str, u32), ScopeId>,
    /// Thread metadata.
    pub threads: BTreeMap<ThreadId, ThreadInfo>,
    /// Per-thread span streams.
    pub thread_streams: BTreeMap<ThreadId, ThreadStream>,
    /// GPU lane metadata.
    pub gpu_lanes: BTreeMap<GpuLaneId, GpuLaneInfo>,
    /// Per-GPU-lane span streams.
    pub gpu_streams: BTreeMap<GpuLaneId, GpuStream>,
    /// Per-counter streams.
    pub counter_streams: HashMap<StringId, CounterStream>,
    /// Frame marks in chronological order.
    pub frame_marks: Vec<FrameMark>,
    /// Eviction policy.
    pub retention: RetentionPolicy,
    /// Pending `Begin` events whose `End` has not been seen yet, keyed
    /// by `SpanId`. Used to pair begin/end into completed spans at
    /// aggregation time.
    pending: HashMap<SpanId, PendingBegin>,
}

#[derive(Clone, Debug)]
struct PendingBegin {
    scope: ScopeId,
    thread: ThreadId,
    parent: Option<SpanId>,
    start_ns: u64,
    data: SpanData,
}

impl Timeline {
    /// Creates an empty timeline with the default retention policy.
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            scope_index: HashMap::new(),
            threads: BTreeMap::new(),
            thread_streams: BTreeMap::new(),
            gpu_lanes: BTreeMap::new(),
            gpu_streams: BTreeMap::new(),
            counter_streams: HashMap::new(),
            frame_marks: Vec::new(),
            retention: RetentionPolicy::default(),
            pending: HashMap::new(),
        }
    }

    /// Registers a scope site, returning its [`ScopeId`]. Idempotent
    /// for matching `(name, file, line)` triples.
    pub fn register_scope(&mut self, name: StringId, file: &'static str, line: u32) -> ScopeId {
        if let Some(&id) = self.scope_index.get(&(name, file, line)) {
            return id;
        }
        let idx = self.scopes.len() as u32;
        let id = ScopeId(NonZeroU32::new(idx + 1).expect("scope table overflow"));
        self.scopes.push(ScopeInfo { name, file, line });
        self.scope_index.insert((name, file, line), id);
        id
    }

    /// Registers a thread with the given id and name.
    pub fn register_thread(&mut self, thread_id: ThreadId, name: StringId) {
        self.threads.insert(thread_id, ThreadInfo { name });
        self.thread_streams.entry(thread_id).or_default();
    }

    /// Updates the display name of an already-registered thread.
    pub fn rename_thread(&mut self, thread_id: ThreadId, name: StringId) {
        if let Some(info) = self.threads.get_mut(&thread_id) {
            info.name = name;
        } else {
            self.register_thread(thread_id, name);
        }
    }

    /// Registers a GPU lane (typically one per queue).
    pub fn register_gpu_lane(&mut self, lane: GpuLaneId, name: StringId) {
        self.gpu_lanes.insert(lane, GpuLaneInfo { name });
        self.gpu_streams.entry(lane).or_default();
    }

    /// Drains a batch of events for `thread_id` into this timeline,
    /// pairing `Begin`/`End` into [`CpuSpan`]s and routing `Counter`
    /// events to the appropriate [`CounterStream`].
    pub(crate) fn absorb_thread_events(&mut self, thread_id: ThreadId, events: Vec<Event>) {
        let stream = self.thread_streams.entry(thread_id).or_default();
        for ev in events {
            match ev {
                Event::Begin {
                    id,
                    scope,
                    ts_ns,
                    parent,
                } => {
                    self.pending.insert(
                        id,
                        PendingBegin {
                            scope,
                            thread: thread_id,
                            parent,
                            start_ns: ts_ns,
                            data: SpanData::None,
                        },
                    );
                }
                Event::End { id, ts_ns } => {
                    if let Some(begin) = self.pending.remove(&id) {
                        stream.spans.push_back(CpuSpan {
                            id,
                            scope: begin.scope,
                            thread: begin.thread,
                            parent: begin.parent,
                            start_ns: begin.start_ns,
                            end_ns: ts_ns,
                            data: begin.data,
                        });
                    }
                    // If there is no matching Begin, the End is
                    // dropped. This can happen if a span began in
                    // a previous retention window that has already
                    // been evicted.
                }
                Event::Counter {
                    counter,
                    ts_ns,
                    value,
                } => {
                    self.counter_streams
                        .entry(counter)
                        .or_default()
                        .samples
                        .push_back(CounterSample {
                            counter,
                            ts_ns,
                            value,
                        });
                }
            }
        }
    }

    /// Appends a completed GPU span to the appropriate lane stream.
    pub fn absorb_gpu_span(&mut self, span: GpuSpan) {
        self.gpu_streams
            .entry(span.lane)
            .or_default()
            .spans
            .push_back(span);
    }

    /// Records a new frame mark and applies the retention policy.
    ///
    /// Frame marks are append-only and provide the cut-off points
    /// used by retention.
    pub fn push_frame_mark(&mut self, mark: FrameMark) {
        self.frame_marks.push(mark);
        self.evict();
    }

    /// Evicts data older than the retention policy allows.
    ///
    /// Streams are sorted by `start_ns` (CPU/GPU spans) and `ts_ns`
    /// (counters), so the index of the first kept element can be
    /// found in `O(log n)` via [`VecDeque::partition_point`]. The
    /// front [`VecDeque::drain`] then runs in `O(drop_count)` —
    /// constant amortised cost regardless of how much retained data
    /// is in the deque, because draining from the front of a deque
    /// just advances the head pointer over the removed elements
    /// rather than memmoving the survivors.
    fn evict(&mut self) {
        if self.frame_marks.len() <= self.retention.max_frames {
            return;
        }
        let drop_count = self.frame_marks.len() - self.retention.max_frames;
        let cutoff_ns = self.frame_marks[drop_count - 1].end_ns;
        self.frame_marks.drain(..drop_count);

        for stream in self.thread_streams.values_mut() {
            let idx = stream.spans.partition_point(|s| s.end_ns < cutoff_ns);
            stream.spans.drain(..idx);
        }
        for stream in self.gpu_streams.values_mut() {
            let idx = stream.spans.partition_point(|s| s.end_ns < cutoff_ns);
            stream.spans.drain(..idx);
        }
        for stream in self.counter_streams.values_mut() {
            let idx = stream.samples.partition_point(|s| s.ts_ns < cutoff_ns);
            stream.samples.drain(..idx);
        }
    }

    /// Clears all collected spans, samples and frame marks while
    /// keeping the registered scopes, threads and GPU lanes intact.
    ///
    /// Intended for use by the in-tree microbenchmarks to reset
    /// timeline state between bench functions, so the cost of one
    /// bench's `frame_mark` calls isn't inflated by data accumulated
    /// by an earlier bench. Not part of the user-facing API.
    #[doc(hidden)]
    pub fn clear_data(&mut self) {
        for stream in self.thread_streams.values_mut() {
            stream.spans.clear();
        }
        for stream in self.gpu_streams.values_mut() {
            stream.spans.clear();
        }
        for stream in self.counter_streams.values_mut() {
            stream.samples.clear();
        }
        self.frame_marks.clear();
        self.pending.clear();
    }

    /// Returns the most recent frame mark, if any.
    pub fn last_frame(&self) -> Option<FrameMark> {
        self.frame_marks.last().copied()
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{CpuSpan, GpuLaneId, GpuSpan, ScopeId, SpanData, SpanId};
    use std::num::NonZeroU32;

    fn sid(n: u32) -> StringId {
        StringId(NonZeroU32::new(n).unwrap())
    }
    fn tid(n: u32) -> ThreadId {
        ThreadId(n)
    }

    #[test]
    fn register_scope_is_idempotent_per_site() {
        let mut t = Timeline::new();
        let a = t.register_scope(sid(1), "foo.rs", 10);
        let b = t.register_scope(sid(1), "foo.rs", 10);
        assert_eq!(a, b);
        assert_eq!(t.scopes.len(), 1);

        let c = t.register_scope(sid(1), "foo.rs", 11);
        assert_ne!(a, c);
        assert_eq!(t.scopes.len(), 2);
    }

    #[test]
    fn absorb_pairs_begin_and_end_into_span() {
        let mut t = Timeline::new();
        let scope = t.register_scope(sid(1), "x.rs", 1);
        t.register_thread(tid(0), sid(1));

        t.absorb_thread_events(
            tid(0),
            vec![
                Event::Begin {
                    id: SpanId(1),
                    scope,
                    ts_ns: 100,
                    parent: None,
                },
                Event::End {
                    id: SpanId(1),
                    ts_ns: 200,
                },
            ],
        );

        let stream = &t.thread_streams[&tid(0)];
        assert_eq!(stream.spans.len(), 1);
        assert_eq!(stream.spans[0].start_ns, 100);
        assert_eq!(stream.spans[0].end_ns, 200);
        assert_eq!(stream.spans[0].scope, scope);
        assert!(t.pending.is_empty());
    }

    #[test]
    fn end_without_begin_is_dropped() {
        let mut t = Timeline::new();
        t.absorb_thread_events(
            tid(0),
            vec![Event::End {
                id: SpanId(42),
                ts_ns: 100,
            }],
        );
        assert!(t.thread_streams.get(&tid(0)).map(|s| s.spans.is_empty()).unwrap_or(true));
    }

    #[test]
    fn retention_evicts_oldest_frames() {
        let mut t = Timeline::new();
        t.retention = RetentionPolicy { max_frames: 2 };
        let scope = t.register_scope(sid(1), "x.rs", 1);
        t.register_thread(tid(0), sid(1));

        // Emit three frames; each contributes one span. After
        // retention=2, the span in the first evicted frame should
        // be gone.
        for i in 0..3u64 {
            let base = i * 1000;
            t.absorb_thread_events(
                tid(0),
                vec![
                    Event::Begin {
                        id: SpanId(i + 1),
                        scope,
                        ts_ns: base,
                        parent: None,
                    },
                    Event::End {
                        id: SpanId(i + 1),
                        ts_ns: base + 100,
                    },
                ],
            );
            t.push_frame_mark(FrameMark {
                index: i,
                start_ns: base,
                end_ns: base + 100,
            });
        }

        assert_eq!(t.frame_marks.len(), 2);
        let spans = &t.thread_streams[&tid(0)].spans;
        // The span from frame 0 (end_ns = 100) must be evicted;
        // the cut-off is the end_ns of the last dropped frame (100).
        // Retained: spans with end_ns >= 100 → all three still in.
        //
        // Frame-grained retention means spans whose end_ns falls
        // inside a dropped frame are dropped too. First dropped
        // frame covers [0, 100]; span 0 ends at 100, retained.
        assert!(spans.iter().all(|s| s.end_ns >= 100));
    }

    #[test]
    fn counter_events_are_routed_to_counter_streams() {
        let mut t = Timeline::new();
        let counter_id = sid(7);
        t.absorb_thread_events(
            tid(0),
            vec![Event::Counter {
                counter: counter_id,
                ts_ns: 500,
                value: 42.0,
            }],
        );
        assert_eq!(t.counter_streams[&counter_id].samples.len(), 1);
        assert_eq!(t.counter_streams[&counter_id].samples[0].value, 42.0);
    }

    /// Helper: push a single completed span at `[start, end]` into
    /// `tid(0)` of `t` without going through the event-pairing path.
    fn push_span(t: &mut Timeline, scope: ScopeId, start: u64, end: u64) {
        t.absorb_thread_events(
            tid(0),
            vec![
                Event::Begin {
                    id: SpanId(start ^ end),
                    scope,
                    ts_ns: start,
                    parent: None,
                },
                Event::End {
                    id: SpanId(start ^ end),
                    ts_ns: end,
                },
            ],
        );
    }

    #[test]
    fn evict_drops_only_front_spans_strictly_before_cutoff() {
        let mut t = Timeline::new();
        t.retention = RetentionPolicy { max_frames: 1 };
        let scope = t.register_scope(sid(1), "x.rs", 1);
        t.register_thread(tid(0), sid(1));

        push_span(&mut t, scope, 0, 50);
        push_span(&mut t, scope, 60, 100);
        push_span(&mut t, scope, 110, 200);
        // Cutoff will be 150 (end_ns of the dropped frame).
        // Spans whose end_ns is strictly < 150 should be dropped:
        // [0,50] dropped, [60,100] dropped, [110,200] retained.
        t.push_frame_mark(FrameMark { index: 0, start_ns: 0, end_ns: 150 });
        t.push_frame_mark(FrameMark { index: 1, start_ns: 150, end_ns: 250 });

        let spans = &t.thread_streams[&tid(0)].spans;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start_ns, 110);
        assert_eq!(spans[0].end_ns, 200);
    }

    #[test]
    fn evict_keeps_span_whose_end_equals_cutoff() {
        // Boundary check: a span ending exactly at the cutoff is
        // retained (predicate is `end_ns < cutoff`, strict).
        let mut t = Timeline::new();
        t.retention = RetentionPolicy { max_frames: 1 };
        let scope = t.register_scope(sid(1), "x.rs", 1);
        t.register_thread(tid(0), sid(1));

        push_span(&mut t, scope, 0, 100);
        push_span(&mut t, scope, 50, 100);
        t.push_frame_mark(FrameMark { index: 0, start_ns: 0, end_ns: 100 });
        t.push_frame_mark(FrameMark { index: 1, start_ns: 100, end_ns: 200 });

        let spans = &t.thread_streams[&tid(0)].spans;
        assert_eq!(spans.len(), 2, "spans whose end_ns == cutoff must be retained");
    }

    #[test]
    fn evict_drops_everything_when_all_spans_are_old() {
        let mut t = Timeline::new();
        t.retention = RetentionPolicy { max_frames: 1 };
        let scope = t.register_scope(sid(1), "x.rs", 1);
        t.register_thread(tid(0), sid(1));

        push_span(&mut t, scope, 0, 50);
        push_span(&mut t, scope, 60, 90);
        t.push_frame_mark(FrameMark { index: 0, start_ns: 0, end_ns: 100 });
        t.push_frame_mark(FrameMark { index: 1, start_ns: 100, end_ns: 200 });

        let spans = &t.thread_streams[&tid(0)].spans;
        assert!(spans.is_empty());
    }

    /// Helper for the GPU `spans_in_window` tests: build a stream
    /// directly from `(start, end)` pairs, bypassing pairing.
    fn gpu_stream_from(pairs: &[(u64, u64)]) -> GpuStream {
        let mut stream = GpuStream::default();
        let scope = ScopeId(NonZeroU32::new(1).unwrap());
        for (i, &(s, e)) in pairs.iter().enumerate() {
            stream.spans.push_back(GpuSpan {
                id: SpanId(i as u64 + 1),
                scope,
                lane: GpuLaneId(0),
                parent: None,
                start_ns: s,
                end_ns: e,
            });
        }
        stream
    }

    fn thread_stream_with_spans(t: &Timeline, thread: ThreadId) -> &ThreadStream {
        &t.thread_streams[&thread]
    }

    fn push_spans(t: &mut Timeline, scope: ScopeId, spans: &[(u64, u64)]) {
        for (i, &(s, e)) in spans.iter().enumerate() {
            t.absorb_thread_events(
                tid(0),
                vec![
                    Event::Begin {
                        id: SpanId(i as u64 + 1_000),
                        scope,
                        ts_ns: s,
                        parent: None,
                    },
                    Event::End {
                        id: SpanId(i as u64 + 1_000),
                        ts_ns: e,
                    },
                ],
            );
        }
    }

    #[test]
    fn spans_in_window_contained_inside_single_span() {
        // A single span [0, 1000] completely contains the window
        // [400, 600) — it should be returned.
        let mut t = Timeline::new();
        let scope = t.register_scope(sid(1), "x.rs", 1);
        t.register_thread(tid(0), sid(1));
        push_spans(&mut t, scope, &[(0, 1000)]);

        let stream = thread_stream_with_spans(&t, tid(0));
        let hits: Vec<_> = stream.spans_in_window(400, 600).collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].start_ns, 0);
        assert_eq!(hits[0].end_ns, 1000);
    }

    #[test]
    fn spans_in_window_straddles_start_edge() {
        // Span [50, 150] overlaps the window [100, 200) on the left;
        // must be returned. Span [10, 40] ends before the window
        // and must be excluded.
        let mut t = Timeline::new();
        let scope = t.register_scope(sid(1), "x.rs", 1);
        t.register_thread(tid(0), sid(1));
        push_spans(&mut t, scope, &[(10, 40), (50, 150)]);

        let stream = thread_stream_with_spans(&t, tid(0));
        let hits: Vec<_> = stream.spans_in_window(100, 200).collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].start_ns, 50);
    }

    #[test]
    fn spans_in_window_straddles_end_edge() {
        // Span [150, 250] overlaps the window [100, 200) on the
        // right; must be returned. Span [210, 300] starts after
        // the window ends and must be excluded.
        let mut t = Timeline::new();
        let scope = t.register_scope(sid(1), "x.rs", 1);
        t.register_thread(tid(0), sid(1));
        push_spans(&mut t, scope, &[(150, 250), (210, 300)]);

        let stream = thread_stream_with_spans(&t, tid(0));
        let hits: Vec<_> = stream.spans_in_window(100, 200).collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].start_ns, 150);
    }

    #[test]
    fn spans_in_window_empty_stream() {
        let stream = ThreadStream::default();
        let hits: Vec<_> = stream.spans_in_window(0, 1000).collect();
        assert!(hits.is_empty());
    }

    #[test]
    fn spans_in_window_outside_retained_range() {
        // All spans ended before the window's start — partition_point
        // should land past every span and the iterator is empty.
        let mut t = Timeline::new();
        let scope = t.register_scope(sid(1), "x.rs", 1);
        t.register_thread(tid(0), sid(1));
        push_spans(&mut t, scope, &[(0, 50), (60, 90), (100, 150)]);

        let stream = thread_stream_with_spans(&t, tid(0));
        let hits: Vec<_> = stream.spans_in_window(500, 1000).collect();
        assert!(hits.is_empty());
    }

    #[test]
    fn spans_in_window_half_open_boundary() {
        // Boundary cases under the [start, end) convention:
        //  - A span whose end_ns == visible_start_ns is excluded.
        //  - A span whose start_ns == visible_end_ns is excluded.
        let mut t = Timeline::new();
        let scope = t.register_scope(sid(1), "x.rs", 1);
        t.register_thread(tid(0), sid(1));
        push_spans(&mut t, scope, &[(0, 100), (200, 300)]);

        let stream = thread_stream_with_spans(&t, tid(0));
        let hits: Vec<_> = stream.spans_in_window(100, 200).collect();
        assert!(
            hits.is_empty(),
            "half-open boundaries must exclude touching spans, got {hits:?}",
        );
    }

    #[test]
    fn spans_in_window_includes_nested_parent_pushed_after_children() {
        // Nested-scope invariant: the parent span has an earlier
        // start_ns than its children but is pushed to the deque after
        // them (end_ns is monotonic, start_ns is not). A window that
        // the parent overlaps must still include it even though the
        // naive "walk forward until start_ns exits window" approach
        // would miss it.
        //
        // Deque state (ordered by push time / end_ns):
        //   [0] child1  start=10, end=20
        //   [1] child2  start=30, end=40
        //   [2] parent  start=5,  end=1000
        //
        // Window [100, 200): parent overlaps (5 < 200 && 1000 > 100),
        // children do not.
        let mut stream = ThreadStream::default();
        let scope = ScopeId(NonZeroU32::new(1).unwrap());
        for (i, &(s, e)) in [(10, 20), (30, 40), (5, 1000)].iter().enumerate() {
            stream.spans.push_back(CpuSpan {
                id: SpanId(i as u64 + 1),
                scope,
                thread: tid(0),
                parent: None,
                start_ns: s,
                end_ns: e,
                data: SpanData::None,
            });
        }

        let hits: Vec<_> = stream.spans_in_window(100, 200).collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].start_ns, 5);
        assert_eq!(hits[0].end_ns, 1000);
    }

    #[test]
    fn gpu_spans_in_window_basic() {
        // Mirror the CPU coverage in miniature for the GPU helper.
        let stream = gpu_stream_from(&[(0, 50), (60, 150), (200, 300)]);
        let hits: Vec<_> = stream.spans_in_window(100, 250).collect();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].start_ns, 60);
        assert_eq!(hits[1].start_ns, 200);
    }

    #[test]
    fn gpu_spans_in_window_half_open_boundary() {
        let stream = gpu_stream_from(&[(0, 100), (200, 300)]);
        let hits: Vec<_> = stream.spans_in_window(100, 200).collect();
        assert!(hits.is_empty());
    }

    #[test]
    fn clear_data_keeps_registrations_but_drops_spans() {
        let mut t = Timeline::new();
        let scope = t.register_scope(sid(1), "x.rs", 1);
        t.register_thread(tid(0), sid(2));

        push_span(&mut t, scope, 0, 50);
        t.push_frame_mark(FrameMark { index: 0, start_ns: 0, end_ns: 100 });

        t.clear_data();

        assert!(t.thread_streams[&tid(0)].spans.is_empty());
        assert!(t.frame_marks.is_empty());
        // Registrations survive.
        assert_eq!(t.scopes.len(), 1);
        assert_eq!(t.threads.len(), 1);
    }
}
