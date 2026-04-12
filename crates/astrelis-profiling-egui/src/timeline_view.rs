//! Scrollable multi-frame profiler viewer.
//!
//! [`ProfilerWindow`] is the primary profiler view. It reads a
//! user-controlled `[visible_start_ns,
//! visible_end_ns)` slice out of the global timeline each frame,
//! lays CPU and GPU spans into lanes, and draws frame-mark overlays
//! behind the bars. Pan is driven by dragging, zoom by the scroll
//! wheel anchored at the cursor.
//!
//! The widget is a pure reader of the profiler state: it takes a
//! read lock on the timeline to build a `Snapshot`, drops the lock,
//! then renders entirely from the owned snapshot. The aggregator's
//! write lock is therefore only blocked for the duration of the
//! data copy, never for rendering.

use std::collections::HashMap;

use astrelis_profiling::data::StringId;
use astrelis_profiling::profiler::Profiler;
use astrelis_profiling::timeline::Timeline;

use crate::layout::compute_depth;

/// Stateful widget for the scrollable timeline view.
pub struct ProfilerWindow {
    row_height: f32,
    min_rect_width: f32,
    lane_spacing: f32,
    header_height: f32,
    cpu_color: egui::Color32,
    gpu_color: egui::Color32,
    /// Visible window start, nanoseconds. `None` on first frame; a
    /// snapshot will initialise it to the data start.
    visible_start_ns: Option<u64>,
    /// Visible window end, nanoseconds. `None` on first frame.
    visible_end_ns: Option<u64>,
    /// When `true`, the visible window re-tracks the retained data
    /// range on every frame. The first pan or zoom flips this to
    /// `false` so the user's window is preserved across frames; the
    /// `Reset` button (or Home key) flips it back to `true`.
    auto_follow: bool,
}

impl Default for ProfilerWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfilerWindow {
    /// Creates a new `ProfilerWindow` with sensible defaults.
    pub fn new() -> Self {
        Self {
            row_height: 16.0,
            min_rect_width: 1.0,
            lane_spacing: 4.0,
            header_height: 16.0,
            cpu_color: egui::Color32::from_rgb(110, 170, 230),
            gpu_color: egui::Color32::from_rgb(230, 180, 80),
            visible_start_ns: None,
            visible_end_ns: None,
            auto_follow: true,
        }
    }

    /// Renders the widget into `ui`, reading from the global profiler.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Header runs FIRST so the checkbox / reset button take
        // effect on the same frame they are clicked.
        ui.horizontal(|ui| {
            ui.heading("Profiler timeline");
            ui.separator();
            if let (Some(s), Some(e)) = (self.visible_start_ns, self.visible_end_ns) {
                let visible_ms = e.saturating_sub(s) as f32 / 1_000_000.0;
                ui.label(format!("{visible_ms:.2} ms visible"));
                ui.separator();
            }
            if ui.button("Reset").clicked() {
                self.auto_follow = true;
                self.visible_start_ns = None;
                self.visible_end_ns = None;
            }
            ui.checkbox(&mut self.auto_follow, "auto-follow");
        });

        if ui.input(|i| i.key_pressed(egui::Key::Home)) {
            self.auto_follow = true;
            self.visible_start_ns = None;
            self.visible_end_ns = None;
        }

        // Snapshot is captured AFTER header so auto_follow / reset
        // changes take effect immediately.
        let snap = Snapshot::capture(
            self.visible_start_ns,
            self.visible_end_ns,
            self.auto_follow,
            self.cpu_color,
            self.gpu_color,
        );
        self.visible_start_ns = Some(snap.visible_start_ns);
        self.visible_end_ns = Some(snap.visible_end_ns);

        if snap.cpu_lanes.is_empty() && snap.gpu_lanes.is_empty() {
            ui.label("No spans in visible window yet — waiting for data.");
            return;
        }

        // Total canvas height.
        let lane_height = |lane: &Lane| {
            self.header_height
                + (lane.max_depth + 1) as f32 * self.row_height
                + self.lane_spacing
        };
        let total_height: f32 = snap
            .cpu_lanes
            .iter()
            .chain(snap.gpu_lanes.iter())
            .map(lane_height)
            .sum::<f32>()
            .max(80.0);

        let width = ui.available_width().max(200.0);
        let (rect, response) = ui
            .allocate_exact_size(egui::vec2(width, total_height), egui::Sense::click_and_drag());

        // --- Input handling ---
        let visible_ns =
            snap.visible_end_ns.saturating_sub(snap.visible_start_ns).max(1) as f64;
        let px_per_ns = rect.width() as f64 / visible_ns;
        let ns_per_px = 1.0 / px_per_ns;

        if response.dragged() {
            let dx = response.drag_delta().x as f64;
            let shift_ns = dx * ns_per_px;
            let new_start = (snap.visible_start_ns as f64 - shift_ns).max(0.0) as u64;
            let span_ns = snap.visible_end_ns - snap.visible_start_ns;
            let (clamped_start, clamped_end) = clamp_window(
                new_start,
                new_start.saturating_add(span_ns),
                snap.data_start_ns,
                snap.data_end_ns,
            );
            self.visible_start_ns = Some(clamped_start);
            self.visible_end_ns = Some(clamped_end);
            self.auto_follow = false;
        }

        // Scroll + gesture input. Horizontal scroll = pan, vertical
        // scroll = zoom, pinch gesture = zoom (all cursor-anchored).
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            let pinch = ui.input(|i| i.zoom_delta());

            // Horizontal scroll → pan.
            if scroll.x.abs() > 0.1 {
                let shift_ns = scroll.x as f64 * ns_per_px;
                let new_start = (snap.visible_start_ns as f64 + shift_ns).max(0.0) as u64;
                let span_ns = snap.visible_end_ns - snap.visible_start_ns;
                let (cs, ce) = clamp_window(
                    new_start,
                    new_start.saturating_add(span_ns),
                    snap.data_start_ns,
                    snap.data_end_ns,
                );
                self.visible_start_ns = Some(cs);
                self.visible_end_ns = Some(ce);
                self.auto_follow = false;
            }

            // Zoom from either vertical scroll or pinch gesture,
            // anchored at the cursor.
            let scroll_zoom = if scroll.y.abs() > 0.1 {
                (-scroll.y as f64 / 100.0).exp()
            } else {
                1.0
            };
            let pinch_zoom = if (pinch - 1.0).abs() > 0.001 {
                1.0 / pinch as f64
            } else {
                1.0
            };
            let combined_zoom = scroll_zoom * pinch_zoom;
            if (combined_zoom - 1.0).abs() > 0.001
                && let Some(hover_pos) = response.hover_pos()
            {
                let rel_x = (hover_pos.x - rect.left()) as f64;
                let ns_at_cursor = snap.visible_start_ns as f64 + rel_x * ns_per_px;
                let new_visible_ns = (visible_ns * combined_zoom).clamp(1_000.0, 1e12);
                let new_ns_per_px = new_visible_ns / rect.width() as f64;
                let new_start = (ns_at_cursor - rel_x * new_ns_per_px).max(0.0);
                let new_end = new_start + new_visible_ns;
                let (cs, ce) = clamp_window(
                    new_start as u64,
                    new_end as u64,
                    snap.data_start_ns,
                    snap.data_end_ns,
                );
                self.visible_start_ns = Some(cs);
                self.visible_end_ns = Some(ce);
                self.auto_follow = false;
            }
        }

        // --- Painting ---
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(28, 28, 32));

        // Frame-mark overlay first so bars paint on top. Labels are
        // only drawn when there's enough horizontal space between
        // consecutive marks to avoid overlapping text.
        const LABEL_MIN_SPACING: f32 = 90.0;
        let mut last_label_x = f32::NEG_INFINITY;
        for fm in &snap.frame_marks {
            let rel_ns = fm.end_ns.saturating_sub(snap.visible_start_ns) as f64;
            let x = rect.left() as f64 + rel_ns * px_per_ns;
            if x < rect.left() as f64 || x > rect.right() as f64 {
                continue;
            }
            let xf = x as f32;
            painter.vline(
                xf,
                rect.top()..=rect.bottom(),
                egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(120, 120, 160, 140),
                ),
            );
            if xf - last_label_x >= LABEL_MIN_SPACING {
                painter.text(
                    egui::pos2(xf + 2.0, rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    format!("f{} ({:.2}ms)", fm.index, fm.duration_ms),
                    egui::FontId::monospace(9.0),
                    egui::Color32::from_rgba_unmultiplied(200, 200, 220, 200),
                );
                last_label_x = xf;
            }
        }

        // Bars, lane by lane. Track the bar the cursor is over so we
        // can show a single tooltip after the paint pass.
        let mut hovered: Option<HoverInfo> = None;
        let hover_pos = response.hover_pos();
        let mut y = rect.top();

        for lane in snap.cpu_lanes.iter().chain(snap.gpu_lanes.iter()) {
            painter.text(
                egui::pos2(rect.left() + 4.0, y + 1.0),
                egui::Align2::LEFT_TOP,
                &lane.title,
                egui::FontId::monospace(11.0),
                egui::Color32::WHITE,
            );
            let body_top = y + self.header_height;

            for bar in &lane.bars {
                let start_rel = bar.start_ns.saturating_sub(snap.visible_start_ns) as f64;
                let end_rel = bar.end_ns.saturating_sub(snap.visible_start_ns) as f64;
                let x0 = rect.left() as f64 + start_rel * px_per_ns;
                let x1 = rect.left() as f64 + end_rel * px_per_ns;
                // Skip bars fully off-screen.
                if x1 < rect.left() as f64 || x0 > rect.right() as f64 {
                    continue;
                }
                let x0_f = (x0 as f32).max(rect.left());
                let x1_f = (x1 as f32)
                    .min(rect.right())
                    .max(x0_f + self.min_rect_width);
                let y0 = body_top + bar.depth as f32 * self.row_height;
                let bar_rect = egui::Rect::from_min_max(
                    egui::pos2(x0_f, y0),
                    egui::pos2(x1_f, y0 + self.row_height - 1.0),
                );
                painter.rect_filled(bar_rect, 2.0, lane.color);
                if bar_rect.width() > 30.0
                    && let Some(name) = snap.string_by_id.get(&bar.name_id)
                {
                    painter.text(
                        bar_rect.left_center() + egui::vec2(4.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        name,
                        egui::FontId::monospace(10.0),
                        egui::Color32::BLACK,
                    );
                }
                if let Some(pos) = hover_pos
                    && bar_rect.contains(pos)
                {
                    hovered = Some(HoverInfo {
                        name: snap
                            .string_by_id
                            .get(&bar.name_id)
                            .cloned()
                            .unwrap_or_else(|| "<unknown>".into()),
                        duration_ms: (bar.end_ns - bar.start_ns) as f32 / 1_000_000.0,
                        lane: lane.title.clone(),
                    });
                }
            }

            y = body_top + (lane.max_depth + 1) as f32 * self.row_height + self.lane_spacing;
        }

        if let Some(h) = hovered {
            response.on_hover_ui_at_pointer(|ui| {
                ui.label(egui::RichText::new(&h.name).strong());
                ui.label(format!("{:.3} ms", h.duration_ms));
                ui.label(&h.lane);
            });
        }
    }
}

/// Clamp a visible window to the retained data range plus 10%
/// padding on each side, so the user can always pan even at full
/// zoom-out.
fn clamp_window(start: u64, end: u64, data_start: u64, data_end: u64) -> (u64, u64) {
    let data_span = data_end.saturating_sub(data_start);
    let pad = data_span / 10;
    let padded_start = data_start.saturating_sub(pad);
    let padded_end = data_end.saturating_add(pad);
    let padded_span = padded_end - padded_start;
    let span = end.saturating_sub(start);
    if span >= padded_span {
        return (padded_start, padded_end);
    }
    if start < padded_start {
        return (padded_start, padded_start + span);
    }
    if end > padded_end {
        return (padded_end - span, padded_end);
    }
    (start, end)
}

#[derive(Clone, Debug)]
struct Bar {
    start_ns: u64,
    end_ns: u64,
    depth: u32,
    name_id: StringId,
}

struct Lane {
    title: String,
    bars: Vec<Bar>,
    color: egui::Color32,
    max_depth: u32,
}

#[derive(Clone, Debug)]
struct FrameMarkLabel {
    index: u64,
    end_ns: u64,
    duration_ms: f32,
}

struct HoverInfo {
    name: String,
    duration_ms: f32,
    lane: String,
}

/// A rendering-ready snapshot of the profiler's timeline inside a
/// `[visible_start_ns, visible_end_ns)` window.
struct Snapshot {
    data_start_ns: u64,
    data_end_ns: u64,
    visible_start_ns: u64,
    visible_end_ns: u64,
    cpu_lanes: Vec<Lane>,
    gpu_lanes: Vec<Lane>,
    frame_marks: Vec<FrameMarkLabel>,
    string_by_id: HashMap<StringId, String>,
}

impl Snapshot {
    fn capture(
        pending_start: Option<u64>,
        pending_end: Option<u64>,
        auto_follow: bool,
        cpu_color: egui::Color32,
        gpu_color: egui::Color32,
    ) -> Self {
        let p = Profiler::get();
        let timeline = p.timeline.read().unwrap();

        // Resolve the data range from retained frame marks. If there
        // are none, also check the streams in case spans exist before
        // any frame_mark has been called.
        let (data_start_ns, data_end_ns) = resolve_data_range(&timeline);
        if data_start_ns >= data_end_ns {
            return Self {
                data_start_ns,
                data_end_ns,
                visible_start_ns: data_start_ns,
                visible_end_ns: data_end_ns,
                cpu_lanes: Vec::new(),
                gpu_lanes: Vec::new(),
                frame_marks: Vec::new(),
                string_by_id: HashMap::new(),
            };
        }

        // Resolve the visible window. When auto-following, show the
        // last ~5 frames rather than the entire retained range so
        // individual spans are readable at the default zoom level.
        let (visible_start_ns, visible_end_ns) = if auto_follow {
            let follow_frames: usize = 5;
            let n = timeline.frame_marks.len();
            if n >= follow_frames {
                let oldest_visible = &timeline.frame_marks[n - follow_frames];
                (oldest_visible.start_ns, data_end_ns)
            } else {
                (data_start_ns, data_end_ns)
            }
        } else {
            let start = pending_start.unwrap_or(data_start_ns);
            let end = pending_end.unwrap_or(data_end_ns);
            clamp_window(start, end, data_start_ns, data_end_ns)
        };

        let mut string_by_id: HashMap<StringId, String> = HashMap::new();
        let mut cpu_lanes = Vec::new();
        let mut gpu_lanes = Vec::new();

        for (tid, stream) in &timeline.thread_streams {
            let info = timeline.threads.get(tid);
            let thread_name = info
                .and_then(|i| p.strings.get(i.name))
                .unwrap_or_else(|| format!("thread-{}", tid.0));
            let mut window: Vec<_> = stream
                .spans_in_window(visible_start_ns, visible_end_ns)
                .collect();
            if window.is_empty() {
                continue;
            }
            window.sort_by_key(|s| s.start_ns);
            let pairs: Vec<(u64, u64)> =
                window.iter().map(|s| (s.start_ns, s.end_ns)).collect();
            let depths = compute_depth(&pairs);
            let max_depth = depths.iter().copied().max().unwrap_or(0);
            let bars: Vec<Bar> = window
                .iter()
                .zip(depths)
                .map(|(s, depth)| {
                    let scope = &timeline.scopes[(s.scope.0.get() - 1) as usize];
                    string_by_id
                        .entry(scope.name)
                        .or_insert_with(|| p.strings.get(scope.name).unwrap_or_default());
                    Bar {
                        start_ns: s.start_ns,
                        end_ns: s.end_ns,
                        depth,
                        name_id: scope.name,
                    }
                })
                .collect();
            cpu_lanes.push(Lane {
                title: format!("CPU: {thread_name}"),
                bars,
                color: cpu_color,
                max_depth,
            });
        }

        for (lid, stream) in &timeline.gpu_streams {
            let info = timeline.gpu_lanes.get(lid);
            let lane_name = info
                .and_then(|i| p.strings.get(i.name))
                .unwrap_or_else(|| format!("gpu-{}", lid.0));
            let mut window: Vec<_> = stream
                .spans_in_window(visible_start_ns, visible_end_ns)
                .collect();
            if window.is_empty() {
                continue;
            }
            window.sort_by_key(|s| s.start_ns);
            let pairs: Vec<(u64, u64)> =
                window.iter().map(|s| (s.start_ns, s.end_ns)).collect();
            let depths = compute_depth(&pairs);
            let max_depth = depths.iter().copied().max().unwrap_or(0);
            let bars: Vec<Bar> = window
                .iter()
                .zip(depths)
                .map(|(s, depth)| {
                    let scope = &timeline.scopes[(s.scope.0.get() - 1) as usize];
                    string_by_id
                        .entry(scope.name)
                        .or_insert_with(|| p.strings.get(scope.name).unwrap_or_default());
                    Bar {
                        start_ns: s.start_ns,
                        end_ns: s.end_ns,
                        depth,
                        name_id: scope.name,
                    }
                })
                .collect();
            gpu_lanes.push(Lane {
                title: format!("GPU: {lane_name}"),
                bars,
                color: gpu_color,
                max_depth,
            });
        }

        let frame_marks = timeline
            .frame_marks
            .iter()
            .filter(|fm| fm.end_ns >= visible_start_ns && fm.end_ns <= visible_end_ns)
            .map(|fm| FrameMarkLabel {
                index: fm.index,
                end_ns: fm.end_ns,
                duration_ms: (fm.end_ns.saturating_sub(fm.start_ns)) as f32 / 1_000_000.0,
            })
            .collect();

        Self {
            data_start_ns,
            data_end_ns,
            visible_start_ns,
            visible_end_ns,
            cpu_lanes,
            gpu_lanes,
            frame_marks,
            string_by_id,
        }
    }
}

fn resolve_data_range(timeline: &Timeline) -> (u64, u64) {
    // Prefer frame_mark bounds because that's the natural "data
    // region" from the user's perspective. Fall back to the raw
    // span streams if profiling has started but no frame_mark has
    // fired yet.
    if let (Some(first), Some(last)) = (timeline.frame_marks.first(), timeline.frame_marks.last()) {
        return (first.start_ns, last.end_ns);
    }
    let mut min = u64::MAX;
    let mut max = 0u64;
    for stream in timeline.thread_streams.values() {
        for s in &stream.spans {
            min = min.min(s.start_ns);
            max = max.max(s.end_ns);
        }
    }
    for stream in timeline.gpu_streams.values() {
        for s in &stream.spans {
            min = min.min(s.start_ns);
            max = max.max(s.end_ns);
        }
    }
    if min == u64::MAX {
        (0, 0)
    } else {
        (min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_window_passes_through_when_inside_padded_range() {
        // data=[0,1000], padded=[-100,1100]. Window [200,400] fits.
        assert_eq!(clamp_window(200, 400, 0, 1000), (200, 400));
    }

    #[test]
    fn clamp_window_slides_right_when_before_padded_start() {
        // data=[1000,2000], pad=100, padded=[900,2100].
        // Window [800,900] is before padded start → slide to [900,1000].
        assert_eq!(clamp_window(800, 900, 1000, 2000), (900, 1000));
    }

    #[test]
    fn clamp_window_slides_left_when_past_padded_end() {
        // data=[0,1000], pad=100, padded=[-100,1100] (but saturated
        // to [0,1100] since u64). Window [1050,1150] past padded end.
        assert_eq!(clamp_window(1050, 1150, 0, 1000), (1000, 1100));
    }

    #[test]
    fn clamp_window_snaps_to_padded_when_window_larger() {
        // data=[100,500], pad=40, padded=[60,540].
        // Window [0,10000] wider than padded → snap to padded.
        assert_eq!(clamp_window(0, 10_000, 100, 500), (60, 540));
    }

    #[test]
    fn clamp_window_allows_pan_at_full_data_range() {
        // The key case: view span = data span. With padding the user
        // can still pan because padded range is wider than data range.
        // data=[0,1000], pad=100, padded=[0,1100] (saturated left).
        // Window [100,1100] has span=1000 < padded_span=1100. Fits.
        assert_eq!(clamp_window(100, 1100, 0, 1000), (100, 1100));
    }
}
