//! GPU profiling integration.
//!
//! `astrelis-gpu` collects raw wgpu timestamp queries via
//! `wgpu_profiler`, walks the resulting query tree into a
//! [`GpuFrame`] (a simple tree of labeled scopes in nanoseconds),
//! and hands it to [`report_gpu_frame`] once per processed frame.
//!
//! This module walks the tree, allocates `SpanId`s, and appends
//! [`GpuSpan`] values to the profiler's global
//! [`Timeline`](crate::timeline::Timeline) so they appear alongside
//! CPU spans on the same axis.
//!
//! # Clock alignment
//!
//! The CPU↔GPU clock offset is installed by `astrelis-gpu` via a
//! synchronous calibration round-trip at device creation, and
//! refreshed periodically (every 5 s). This module assumes the
//! offset is already set when `report_gpu_frame` is called.

use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

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

#[derive(Clone, Copy, Debug, Default)]
struct LaneClock {
    offset_ns: i64,
    calibrated: bool,
}

static GPU_LANE_CLOCKS: OnceLock<RwLock<HashMap<GpuLaneId, LaneClock>>> = OnceLock::new();

/// Registers one GPU queue lane.
pub fn register_gpu_lane(backend: GpuBackend, label: Option<&str>) -> GpuLaneId {
    let p = Profiler::get();
    let lane = p.next_gpu_lane_id();
    let name_id: StringId = p
        .strings
        .intern(label.unwrap_or_else(|| backend.default_lane_name()));
    p.timeline.write().unwrap().register_gpu_lane(lane, name_id);
    GPU_LANE_CLOCKS
        .get_or_init(Default::default)
        .write()
        .expect("GPU lane clocks poisoned")
        .insert(lane, LaneClock::default());
    lane
}

/// Installs or refreshes the CPU-to-GPU clock offset for one queue.
pub fn set_gpu_lane_offset_ns(lane: GpuLaneId, offset_ns: i64) {
    GPU_LANE_CLOCKS
        .get_or_init(Default::default)
        .write()
        .expect("GPU lane clocks poisoned")
        .insert(
            lane,
            LaneClock {
                offset_ns,
                calibrated: true,
            },
        );
}

/// Reports a completed GPU frame on a registered queue lane.
pub fn report_gpu_frame(lane: GpuLaneId, frame: GpuFrame) {
    if frame.scopes.is_empty() {
        return;
    }
    let p = Profiler::get();
    let clock = GPU_LANE_CLOCKS
        .get_or_init(Default::default)
        .read()
        .expect("GPU lane clocks poisoned")
        .get(&lane)
        .copied()
        .unwrap_or_default();

    let mut timeline = p.timeline.write().unwrap();
    for scope in &frame.scopes {
        absorb_scope(&mut timeline, p, scope, lane, clock, None);
    }
}

fn absorb_scope(
    timeline: &mut crate::timeline::Timeline,
    p: &Profiler,
    scope: &GpuScope,
    lane: GpuLaneId,
    clock: LaneClock,
    parent: Option<SpanId>,
) {
    if scope.start_ns < 0 || scope.end_ns < scope.start_ns {
        return;
    }
    let name_id = p.strings.intern(&scope.label);
    let scope_id = timeline.register_scope(name_id, "<gpu>", 0);
    let span_id = p.next_span_id();
    let start_ns = convert_timestamp(scope.start_ns as u64, clock);
    let end_ns = convert_timestamp(scope.end_ns as u64, clock);

    timeline.absorb_gpu_span(GpuSpan {
        id: span_id,
        scope: scope_id,
        lane,
        parent,
        start_ns,
        end_ns,
    });

    for child in &scope.nested {
        absorb_scope(timeline, p, child, lane, clock, Some(span_id));
    }
}

fn convert_timestamp(timestamp_ns: u64, clock: LaneClock) -> u64 {
    if !clock.calibrated {
        return timestamp_ns;
    }
    if clock.offset_ns >= 0 {
        timestamp_ns.saturating_add(clock.offset_ns as u64)
    } else {
        timestamp_ns.saturating_sub((-clock.offset_ns) as u64)
    }
}
