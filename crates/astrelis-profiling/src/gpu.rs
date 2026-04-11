//! GPU profiling integration.
//!
//! `astrelis-gpu` collects raw wgpu timestamp queries via
//! `wgpu_profiler`, walks the resulting query tree into a
//! [`GpuFrame`] (a simple tree of labeled scopes in nanoseconds),
//! and hands it to [`report_gpu_frame`] once per processed frame.
//!
//! This module walks the tree, allocates `SpanId`s, and appends
//! [`GpuSpan`](crate::data::GpuSpan)s to the profiler's global
//! [`Timeline`](crate::timeline::Timeline) so they appear alongside
//! CPU spans on the same axis.
//!
//! # Clock alignment
//!
//! On the first call to `report_gpu_frame`, the profiler's clock is
//! calibrated by pinning the first GPU scope's start to the current
//! CPU `now_ns`. Stage 1 uses a single static offset — good enough
//! for the in-engine viewer, which presents spans relative to recent
//! frames. Stage 2 will periodically refresh the offset.

use std::sync::OnceLock;

use crate::data::{GpuLaneId, GpuSpan, SpanId, StringId};
use crate::profiler::Profiler;

/// Which graphics API produced a GPU frame. Used only to label the
/// default GPU lane at registration time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GpuBackend {
    /// Vulkan.
    Vulkan,
    /// Metal (macOS, iOS).
    Metal,
    /// DirectX 12.
    Dx12,
    /// OpenGL / OpenGL ES.
    Gl,
    /// WebGPU (browser).
    WebGpu,
    /// Unknown or unsupported backend.
    Unknown,
}

impl GpuBackend {
    fn default_lane_name(self) -> &'static str {
        match self {
            GpuBackend::Vulkan => "GPU (Vulkan)",
            GpuBackend::Metal => "GPU (Metal)",
            GpuBackend::Dx12 => "GPU (DX12)",
            GpuBackend::Gl => "GPU (GL)",
            GpuBackend::WebGpu => "GPU (WebGPU)",
            GpuBackend::Unknown => "GPU",
        }
    }
}

/// A single GPU timing scope produced by the collector, with
/// optional nested children.
///
/// Timestamps are raw GPU nanoseconds — `astrelis-gpu` multiplies
/// the wgpu queue's `timestamp_period` into ticks before it hands
/// them here. The profiler applies the CPU↔GPU alignment offset on
/// absorption, so callers don't need to.
#[derive(Clone, Debug)]
pub struct GpuScope {
    /// Human-readable label (typically the render/compute pass name).
    pub label: String,
    /// Raw GPU begin timestamp in nanoseconds.
    pub start_ns: i64,
    /// Raw GPU end timestamp in nanoseconds.
    pub end_ns: i64,
    /// Child scopes nested within this one.
    pub nested: Vec<GpuScope>,
}

/// A completed GPU frame's timing scopes, ready to submit to the
/// profiler.
#[derive(Clone, Debug, Default)]
pub struct GpuFrame {
    /// Top-level scopes for this frame, in chronological order.
    pub scopes: Vec<GpuScope>,
}

/// Handle for the default GPU lane. Allocated on the first call to
/// [`init_gpu_context`].
static DEFAULT_LANE: OnceLock<GpuLaneId> = OnceLock::new();

/// Registers the active GPU backend and allocates the default GPU
/// lane on the timeline.
///
/// Safe to call multiple times — the lane is allocated only once.
pub fn init_gpu_context(backend: GpuBackend) {
    let p = Profiler::get();
    DEFAULT_LANE.get_or_init(|| {
        let lane = p.next_gpu_lane_id();
        let name_id: StringId = p.strings.intern(backend.default_lane_name());
        p.timeline.write().unwrap().register_gpu_lane(lane, name_id);
        lane
    });
}

/// Reports a completed GPU frame to the profiler.
///
/// Called once per processed frame by `astrelis-gpu`'s
/// `process_profiling_frames`. Empty frames are ignored.
pub fn report_gpu_frame(frame: GpuFrame) {
    if frame.scopes.is_empty() {
        return;
    }
    let p = Profiler::get();
    let lane = match DEFAULT_LANE.get() {
        Some(&lane) => lane,
        // If the producer forgot to call init_gpu_context, allocate
        // a generic lane so the data isn't dropped.
        None => {
            let lane = p.next_gpu_lane_id();
            let name_id = p.strings.intern("GPU");
            p.timeline.write().unwrap().register_gpu_lane(lane, name_id);
            let _ = DEFAULT_LANE.set(lane);
            lane
        }
    };

    // Calibrate the CPU↔GPU offset on the first frame: pin the first
    // span's start to the current CPU `now_ns`.
    if !p.clock.gpu_calibrated()
        && let Some(first_gpu_ns) = earliest_gpu_start(&frame.scopes)
    {
        let cpu_ns = p.clock.now_ns();
        let offset = cpu_ns as i64 - first_gpu_ns;
        p.clock.set_gpu_epoch_offset_ns(offset);
    }

    let mut timeline = p.timeline.write().unwrap();
    for scope in &frame.scopes {
        absorb_scope(&mut timeline, p, scope, lane, None);
    }
}

fn earliest_gpu_start(scopes: &[GpuScope]) -> Option<i64> {
    let mut earliest: Option<i64> = None;
    for s in scopes {
        if s.start_ns >= 0 {
            earliest = Some(match earliest {
                Some(prev) => prev.min(s.start_ns),
                None => s.start_ns,
            });
        }
        if let Some(nested_earliest) = earliest_gpu_start(&s.nested) {
            earliest = Some(match earliest {
                Some(prev) => prev.min(nested_earliest),
                None => nested_earliest,
            });
        }
    }
    earliest
}

fn absorb_scope(
    timeline: &mut crate::timeline::Timeline,
    p: &Profiler,
    scope: &GpuScope,
    lane: GpuLaneId,
    parent: Option<SpanId>,
) {
    if scope.start_ns < 0 || scope.end_ns < scope.start_ns {
        return;
    }
    let name_id = p.strings.intern(&scope.label);
    let scope_id = timeline.register_scope(name_id, "<gpu>", 0);
    let span_id = p.next_span_id();
    let start_ns = p.clock.gpu_to_profiler_ns(scope.start_ns as u64);
    let end_ns = p.clock.gpu_to_profiler_ns(scope.end_ns as u64);

    timeline.absorb_gpu_span(GpuSpan {
        id: span_id,
        scope: scope_id,
        lane,
        parent,
        start_ns,
        end_ns,
    });

    for child in &scope.nested {
        absorb_scope(timeline, p, child, lane, Some(span_id));
    }
}
