//! Last-frame flame graph widget: renders the most recent frame's CPU
//! and GPU spans as a nested set of colored rectangles, one lane per
//! thread or GPU queue.
//!
//! Depth computation is delegated to [`crate::layout::compute_depth`]
//! so [`crate::ProfilerWindow`] shares the same logic.

use std::collections::{HashMap, VecDeque};

use astrelis_profiling::data::{CpuSpan, FrameMark, GpuSpan, StringId};
use astrelis_profiling::profiler::Profiler;
use astrelis_profiling::timeline::Timeline;

use crate::layout::compute_depth;

/// Widget state for the last-frame flame graph.
///
/// The widget is a pure reader: it locks the profiler's timeline for
/// reading once per frame, copies out the data it needs, and drops the
/// lock before rendering. Nothing about rendering holds the lock.
#[derive(Default)]
pub struct LastFrameFlameGraph {
    /// Width of one row (lane-depth step) in points.
    row_height: f32,
    /// Minimum visible width in points (sub-pixel spans get clamped).
    min_rect_width: f32,
}

impl LastFrameFlameGraph {
    /// Creates a flame graph widget with sensible defaults.
    pub fn new() -> Self {
        Self {
            row_height: 18.0,
            min_rect_width: 1.0,
        }
    }

    /// Renders the widget into `ui`, reading from the global profiler.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let snapshot = Snapshot::capture();
        self.ui_with(ui, &snapshot);
    }

    /// Renders the widget using a pre-captured [`Snapshot`] instead of
    /// reading from the global profiler. Useful for tests.
    pub fn ui_with(&mut self, ui: &mut egui::Ui, snap: &Snapshot) {
        let Some(frame) = snap.frame else {
            ui.label("No frame captured yet");
            return;
        };
        let frame_ns = frame.end_ns.saturating_sub(frame.start_ns).max(1) as f32;
        let width = ui.available_width().max(100.0);
        let ns_to_px = width / frame_ns;

        ui.heading(format!(
            "Frame {} — {:.2} ms",
            frame.index,
            frame_ns / 1_000_000.0
        ));

        for lane in &snap.lanes {
            ui.separator();
            ui.label(&lane.title);
            render_lane(RenderLaneParams {
                ui,
                bars: &lane.bars,
                frame_start_ns: frame.start_ns,
                ns_to_px,
                row_height: self.row_height,
                min_rect_width: self.min_rect_width,
                strings: &snap.string_by_id,
                color: lane.color,
            });
        }
    }
}

/// A rectangle to draw on the flame graph: one span, with its depth
/// already computed and its name already looked up.
#[derive(Clone, Debug)]
struct Bar {
    start_ns: u64,
    end_ns: u64,
    depth: u32,
    name_id: StringId,
}

/// A pre-rendering snapshot of the data needed for one lane.
struct Lane {
    title: String,
    bars: Vec<Bar>,
    color: egui::Color32,
}

/// A pre-rendering snapshot of the whole timeline state that the
/// flame graph widget reads.
pub struct Snapshot {
    frame: Option<FrameMark>,
    lanes: Vec<Lane>,
    string_by_id: HashMap<StringId, String>,
}

impl Snapshot {
    /// Captures the current state of the global profiler's timeline.
    pub fn capture() -> Self {
        let p = Profiler::get();
        let timeline = p.timeline.read().unwrap();
        let frame = timeline.last_frame();
        let Some(frame) = frame else {
            return Self {
                frame: None,
                lanes: Vec::new(),
                string_by_id: HashMap::new(),
            };
        };

        let mut string_by_id: HashMap<StringId, String> = HashMap::new();
        let mut lanes = Vec::new();

        // CPU lanes, one per registered thread.
        for (tid, stream) in &timeline.thread_streams {
            let info = timeline.threads.get(tid);
            let thread_name = info
                .and_then(|i| p.strings.get(i.name))
                .unwrap_or_else(|| format!("thread-{}", tid.0));
            let bars = build_cpu_bars(&stream.spans, frame, &timeline, &mut string_by_id);
            if bars.is_empty() {
                continue;
            }
            lanes.push(Lane {
                title: format!("CPU: {thread_name}"),
                bars,
                color: egui::Color32::from_rgb(110, 170, 230),
            });
        }

        // GPU lanes, one per registered queue.
        for (lid, stream) in &timeline.gpu_streams {
            let info = timeline.gpu_lanes.get(lid);
            let lane_name = info
                .and_then(|i| p.strings.get(i.name))
                .unwrap_or_else(|| format!("gpu-{}", lid.0));
            let bars = build_gpu_bars(&stream.spans, frame, &timeline, &mut string_by_id);
            if bars.is_empty() {
                continue;
            }
            lanes.push(Lane {
                title: lane_name,
                bars,
                color: egui::Color32::from_rgb(230, 180, 80),
            });
        }

        Self {
            frame: Some(frame),
            lanes,
            string_by_id,
        }
    }
}

fn build_cpu_bars(
    spans: &VecDeque<CpuSpan>,
    frame: FrameMark,
    timeline: &Timeline,
    strings: &mut HashMap<StringId, String>,
) -> Vec<Bar> {
    // Only spans overlapping the current frame window. The deque is
    // `end_ns`-sorted, not `start_ns`-sorted, so we must sort after
    // filtering before handing to `compute_depth`.
    let mut window: Vec<&CpuSpan> = spans
        .iter()
        .filter(|s| s.end_ns > frame.start_ns && s.start_ns < frame.end_ns)
        .collect();
    window.sort_by_key(|s| s.start_ns);

    let pairs: Vec<(u64, u64)> = window.iter().map(|s| (s.start_ns, s.end_ns)).collect();
    let depths = compute_depth(&pairs);

    let p = Profiler::get();
    window
        .iter()
        .zip(depths)
        .map(|(s, depth)| {
            let scope = &timeline.scopes[(s.scope.0.get() - 1) as usize];
            strings
                .entry(scope.name)
                .or_insert_with(|| p.strings.get(scope.name).unwrap_or_default());
            Bar {
                start_ns: s.start_ns,
                end_ns: s.end_ns,
                depth,
                name_id: scope.name,
            }
        })
        .collect()
}

fn build_gpu_bars(
    spans: &VecDeque<GpuSpan>,
    frame: FrameMark,
    timeline: &Timeline,
    strings: &mut HashMap<StringId, String>,
) -> Vec<Bar> {
    let mut window: Vec<&GpuSpan> = spans
        .iter()
        .filter(|s| s.end_ns > frame.start_ns && s.start_ns < frame.end_ns)
        .collect();
    window.sort_by_key(|s| s.start_ns);

    let pairs: Vec<(u64, u64)> = window.iter().map(|s| (s.start_ns, s.end_ns)).collect();
    let depths = compute_depth(&pairs);

    let p = Profiler::get();
    window
        .iter()
        .zip(depths)
        .map(|(s, depth)| {
            let scope = &timeline.scopes[(s.scope.0.get() - 1) as usize];
            strings
                .entry(scope.name)
                .or_insert_with(|| p.strings.get(scope.name).unwrap_or_default());
            Bar {
                start_ns: s.start_ns,
                end_ns: s.end_ns,
                depth,
                name_id: scope.name,
            }
        })
        .collect()
}

struct RenderLaneParams<'a> {
    ui: &'a mut egui::Ui,
    bars: &'a [Bar],
    frame_start_ns: u64,
    ns_to_px: f32,
    row_height: f32,
    min_rect_width: f32,
    strings: &'a HashMap<StringId, String>,
    color: egui::Color32,
}

fn render_lane(params: RenderLaneParams<'_>) {
    let RenderLaneParams {
        ui,
        bars,
        frame_start_ns,
        ns_to_px,
        row_height,
        min_rect_width,
        strings,
        color,
    } = params;

    let max_depth = bars.iter().map(|b| b.depth).max().unwrap_or(0) + 1;
    let lane_height = max_depth as f32 * row_height;
    let (rect, _response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), lane_height), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    for bar in bars {
        let start_offset_ns = bar.start_ns.saturating_sub(frame_start_ns);
        let width_ns = bar.end_ns.saturating_sub(bar.start_ns);
        let x0 = rect.left() + (start_offset_ns as f32) * ns_to_px;
        let width = ((width_ns as f32) * ns_to_px).max(min_rect_width);
        let y0 = rect.top() + (bar.depth as f32) * row_height;
        let bar_rect = egui::Rect::from_min_size(
            egui::pos2(x0, y0),
            egui::vec2(width, row_height - 1.0),
        );
        painter.rect_filled(bar_rect, 2.0, color);

        if width > 40.0
            && let Some(name) = strings.get(&bar.name_id)
        {
            painter.text(
                bar_rect.left_center() + egui::vec2(4.0, 0.0),
                egui::Align2::LEFT_CENTER,
                name,
                egui::FontId::monospace(10.0),
                egui::Color32::BLACK,
            );
        }
    }
}
