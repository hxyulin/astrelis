//! Comprehensive metrics collection system for UI performance monitoring.
//!
//! The `MetricsCollector` provides detailed performance insights:
//! - Frame-by-frame timing breakdown
//! - Per-widget statistics and dirty counts
//! - Memory usage tracking
//! - Automatic performance warning detection
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::metrics_collector::{MetricsCollector, MetricsConfig};
//!
//! let mut collector = MetricsCollector::new(MetricsConfig::default());
//!
//! // In render loop:
//! collector.begin_frame();
//!
//! collector.begin_phase(MetricsPhase::Layout);
//! // ... layout computation ...
//! collector.end_phase(MetricsPhase::Layout);
//!
//! collector.end_frame();
//!
//! // Check for warnings
//! for warning in collector.warnings() {
//!     tracing::warn!("Performance warning: {:?}", warning);
//! }
//! ```

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use astrelis_core::alloc::HashMap;

use crate::dirty::DirtyFlags;
use crate::tree::NodeId;

/// Configuration for the metrics collector.
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Number of frames to keep in the history ring buffer.
    pub history_size: usize,
    /// Target frame time for warning detection (e.g., 16.67ms for 60fps).
    pub target_frame_time: Duration,
    /// Threshold percentage for excessive rebuilds warning.
    pub excessive_rebuild_threshold: f32,
    /// Minimum text cache hit rate before warning.
    pub min_text_cache_hit_rate: f32,
    /// Whether to collect per-widget metrics (more detailed but higher overhead).
    pub collect_per_widget_metrics: bool,
    /// Whether to track memory usage.
    pub track_memory: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            history_size: 120, // 2 seconds at 60fps
            target_frame_time: Duration::from_secs_f64(1.0 / 60.0),
            excessive_rebuild_threshold: 0.5, // 50% of nodes dirty is excessive
            min_text_cache_hit_rate: 0.8,     // 80% hit rate is acceptable
            collect_per_widget_metrics: true,
            track_memory: true,
        }
    }
}

/// Metrics phases for timing different parts of the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricsPhase {
    /// Dirty flag detection and propagation.
    DirtyDetection,
    /// Taffy layout computation.
    TaffyCompute,
    /// Layout cache operations.
    LayoutCache,
    /// Text shaping operations.
    TextShaping,
    /// Glyph atlas uploads.
    GlyphUpload,
    /// Draw list generation.
    DrawListGeneration,
    /// Instance buffer encoding.
    InstanceEncoding,
    /// GPU buffer uploads.
    BufferUpload,
    /// Draw call execution.
    DrawCalls,
    /// Event handling.
    EventHandling,
}

/// Detailed timing metrics for a single frame.
#[derive(Debug, Clone, Default)]
pub struct FrameTimingMetrics {
    /// Unique frame identifier.
    pub frame_id: u64,
    /// Timestamp when frame started.
    pub timestamp: Option<Instant>,

    // Layout phase timings
    /// Time spent detecting dirty nodes.
    pub dirty_detection_time: Duration,
    /// Time spent in Taffy layout computation.
    pub taffy_compute_time: Duration,
    /// Time spent in layout cache operations.
    pub layout_cache_time: Duration,
    /// Total layout phase time.
    pub total_layout_time: Duration,

    // Text phase timings
    /// Time spent shaping text.
    pub text_shaping_time: Duration,
    /// Time spent uploading glyphs to atlas.
    pub glyph_upload_time: Duration,

    // Render phase timings
    /// Time spent generating draw list.
    pub draw_list_time: Duration,
    /// Time spent encoding instance buffers.
    pub instance_encoding_time: Duration,
    /// Time spent uploading buffers to GPU.
    pub buffer_upload_time: Duration,
    /// Time spent executing draw calls.
    pub draw_call_time: Duration,

    // Event phase timing
    /// Time spent handling events.
    pub event_handling_time: Duration,

    // Counts
    /// Number of nodes with layout dirty flag.
    pub nodes_layout_dirty: usize,
    /// Number of nodes with text dirty flag.
    pub nodes_text_dirty: usize,
    /// Number of nodes with paint-only dirty flag.
    pub nodes_paint_dirty: usize,
    /// Total number of nodes in tree.
    pub total_nodes: usize,
    /// Number of draw commands generated.
    pub draw_commands: usize,
    /// Number of quad instances.
    pub quad_instances: usize,
    /// Number of text instances.
    pub text_instances: usize,
    /// Number of image instances.
    pub image_instances: usize,
}

impl FrameTimingMetrics {
    /// Create new frame timing metrics.
    pub fn new(frame_id: u64) -> Self {
        Self {
            frame_id,
            timestamp: Some(Instant::now()),
            ..Default::default()
        }
    }

    /// Calculate total frame time.
    pub fn total_frame_time(&self) -> Duration {
        self.total_layout_time
            + self.text_shaping_time
            + self.glyph_upload_time
            + self.draw_list_time
            + self.instance_encoding_time
            + self.buffer_upload_time
            + self.draw_call_time
            + self.event_handling_time
    }

    /// Check if this was a paint-only frame.
    pub fn is_paint_only_frame(&self) -> bool {
        self.nodes_layout_dirty == 0 && self.nodes_text_dirty == 0
    }

    /// Calculate the percentage of nodes that were dirty.
    pub fn dirty_percentage(&self) -> f32 {
        if self.total_nodes == 0 {
            0.0
        } else {
            let total_dirty =
                self.nodes_layout_dirty + self.nodes_text_dirty + self.nodes_paint_dirty;
            (total_dirty as f32 / self.total_nodes as f32) * 100.0
        }
    }
}

/// Per-widget performance statistics.
#[derive(Debug, Clone, Default)]
pub struct WidgetMetrics {
    /// Total number of times this widget was rendered.
    pub render_count: u64,
    /// Dirty flag counts.
    pub dirty_counts: DirtyFlagCounts,
    /// Total time spent in layout for this widget.
    pub layout_time: Duration,
    /// Text cache hits for this widget.
    pub text_cache_hits: u64,
    /// Text cache misses for this widget.
    pub text_cache_misses: u64,
    /// Last frame this widget was updated.
    pub last_update_frame: u64,
}

impl WidgetMetrics {
    /// Calculate text cache hit rate for this widget.
    pub fn text_cache_hit_rate(&self) -> f32 {
        let total = self.text_cache_hits + self.text_cache_misses;
        if total == 0 {
            1.0 // No text operations = perfect
        } else {
            self.text_cache_hits as f32 / total as f32
        }
    }
}

/// Counts of different dirty flag types.
#[derive(Debug, Clone, Default)]
pub struct DirtyFlagCounts {
    pub layout: u64,
    pub text_shaping: u64,
    pub children_order: u64,
    pub color: u64,
    pub opacity: u64,
    pub geometry: u64,
    pub image: u64,
    pub focus: u64,
    pub transform: u64,
    pub clip: u64,
    pub visibility: u64,
    pub scroll: u64,
}

impl DirtyFlagCounts {
    /// Add dirty flags to counts.
    pub fn add(&mut self, flags: DirtyFlags) {
        if flags.contains(DirtyFlags::LAYOUT) {
            self.layout += 1;
        }
        if flags.contains(DirtyFlags::TEXT_SHAPING) {
            self.text_shaping += 1;
        }
        if flags.contains(DirtyFlags::CHILDREN_ORDER) {
            self.children_order += 1;
        }
        if flags.contains(DirtyFlags::COLOR) {
            self.color += 1;
        }
        if flags.contains(DirtyFlags::OPACITY) {
            self.opacity += 1;
        }
        if flags.contains(DirtyFlags::GEOMETRY) {
            self.geometry += 1;
        }
        if flags.contains(DirtyFlags::IMAGE) {
            self.image += 1;
        }
        if flags.contains(DirtyFlags::FOCUS) {
            self.focus += 1;
        }
        if flags.contains(DirtyFlags::TRANSFORM) {
            self.transform += 1;
        }
        if flags.contains(DirtyFlags::CLIP) {
            self.clip += 1;
        }
        if flags.contains(DirtyFlags::VISIBILITY) {
            self.visibility += 1;
        }
        if flags.contains(DirtyFlags::SCROLL) {
            self.scroll += 1;
        }
    }

    /// Total dirty count.
    pub fn total(&self) -> u64 {
        self.layout
            + self.text_shaping
            + self.children_order
            + self.color
            + self.opacity
            + self.geometry
            + self.image
            + self.focus
            + self.transform
            + self.clip
            + self.visibility
            + self.scroll
    }
}

/// Memory usage metrics.
#[derive(Debug, Clone, Default)]
pub struct MemoryMetrics {
    /// Estimated bytes used by node arena/storage.
    pub node_arena_bytes: usize,
    /// Bytes used by instance buffers.
    pub instance_buffer_bytes: usize,
    /// Bytes used by glyph atlas.
    pub glyph_atlas_bytes: usize,
    /// Bytes used by draw list.
    pub draw_list_bytes: usize,
    /// Bytes used by text cache.
    pub text_cache_bytes: usize,
    /// Bytes used by image cache/bind groups.
    pub image_cache_bytes: usize,
}

impl MemoryMetrics {
    /// Calculate total memory usage.
    pub fn total_bytes(&self) -> usize {
        self.node_arena_bytes
            + self.instance_buffer_bytes
            + self.glyph_atlas_bytes
            + self.draw_list_bytes
            + self.text_cache_bytes
            + self.image_cache_bytes
    }

    /// Format as human-readable string.
    pub fn format_summary(&self) -> String {
        format!(
            "Memory: {:.2}MB total (nodes: {:.2}MB, instances: {:.2}MB, atlas: {:.2}MB)",
            self.total_bytes() as f64 / (1024.0 * 1024.0),
            self.node_arena_bytes as f64 / (1024.0 * 1024.0),
            self.instance_buffer_bytes as f64 / (1024.0 * 1024.0),
            self.glyph_atlas_bytes as f64 / (1024.0 * 1024.0),
        )
    }
}

/// Performance warning types.
#[derive(Debug, Clone)]
pub enum PerformanceWarning {
    /// Too many nodes were rebuilt this frame.
    ExcessiveRebuilds { dirty_percent: f32, threshold: f32 },
    /// Layout changes cascaded too deep.
    LayoutCascade { depth: usize, affected_nodes: usize },
    /// Text cache is thrashing.
    TextCacheThrashing { hit_rate: f32, min_rate: f32 },
    /// Frame took longer than target.
    FrameTimeExceeded { actual: Duration, target: Duration },
    /// Single phase took too long.
    PhaseTimeExceeded {
        phase: MetricsPhase,
        time: Duration,
        threshold: Duration,
    },
    /// Memory usage is high.
    HighMemoryUsage { bytes: usize, threshold: usize },
}

impl PerformanceWarning {
    /// Get severity level (0-2, higher is worse).
    pub fn severity(&self) -> u8 {
        match self {
            PerformanceWarning::ExcessiveRebuilds { dirty_percent, .. } => {
                if *dirty_percent > 0.8 {
                    2
                } else {
                    1
                }
            }
            PerformanceWarning::LayoutCascade { depth, .. } => {
                if *depth > 10 {
                    2
                } else {
                    1
                }
            }
            PerformanceWarning::TextCacheThrashing { hit_rate, .. } => {
                if *hit_rate < 0.5 {
                    2
                } else {
                    1
                }
            }
            PerformanceWarning::FrameTimeExceeded { actual, target } => {
                if *actual > *target * 2 {
                    2
                } else {
                    1
                }
            }
            PerformanceWarning::PhaseTimeExceeded {
                time, threshold, ..
            } => {
                if *time > *threshold * 2 {
                    2
                } else {
                    1
                }
            }
            PerformanceWarning::HighMemoryUsage { bytes, threshold } => {
                if *bytes > threshold * 2 {
                    2
                } else {
                    1
                }
            }
        }
    }

    /// Format as human-readable string.
    pub fn format(&self) -> String {
        match self {
            PerformanceWarning::ExcessiveRebuilds {
                dirty_percent,
                threshold,
            } => {
                format!(
                    "Excessive rebuilds: {:.1}% dirty (threshold: {:.1}%)",
                    dirty_percent * 100.0,
                    threshold * 100.0
                )
            }
            PerformanceWarning::LayoutCascade {
                depth,
                affected_nodes,
            } => {
                format!(
                    "Layout cascade: depth {} affecting {} nodes",
                    depth, affected_nodes
                )
            }
            PerformanceWarning::TextCacheThrashing { hit_rate, min_rate } => {
                format!(
                    "Text cache thrashing: {:.1}% hit rate (minimum: {:.1}%)",
                    hit_rate * 100.0,
                    min_rate * 100.0
                )
            }
            PerformanceWarning::FrameTimeExceeded { actual, target } => {
                format!(
                    "Frame time exceeded: {:.2}ms (target: {:.2}ms)",
                    actual.as_secs_f64() * 1000.0,
                    target.as_secs_f64() * 1000.0
                )
            }
            PerformanceWarning::PhaseTimeExceeded {
                phase,
                time,
                threshold,
            } => {
                format!(
                    "{:?} phase exceeded: {:.2}ms (threshold: {:.2}ms)",
                    phase,
                    time.as_secs_f64() * 1000.0,
                    threshold.as_secs_f64() * 1000.0
                )
            }
            PerformanceWarning::HighMemoryUsage { bytes, threshold } => {
                format!(
                    "High memory usage: {:.2}MB (threshold: {:.2}MB)",
                    *bytes as f64 / (1024.0 * 1024.0),
                    *threshold as f64 / (1024.0 * 1024.0)
                )
            }
        }
    }
}

/// Comprehensive metrics collector for UI system.
pub struct MetricsCollector {
    /// Configuration.
    config: MetricsConfig,
    /// Ring buffer of frame timing metrics.
    frame_metrics: VecDeque<FrameTimingMetrics>,
    /// Per-widget metrics.
    widget_metrics: HashMap<NodeId, WidgetMetrics>,
    /// Current memory metrics.
    memory_metrics: MemoryMetrics,
    /// Active warnings from last frame.
    warnings: Vec<PerformanceWarning>,
    /// Current frame being recorded.
    current_frame: Option<FrameTimingMetrics>,
    /// Frame counter.
    frame_count: u64,
    /// Phase timers.
    phase_timers: HashMap<MetricsPhase, Instant>,
    /// Text cache stats from last update.
    text_cache_hits: u64,
    text_cache_misses: u64,
    /// Whether collection is enabled.
    enabled: bool,
}

impl MetricsCollector {
    /// Create a new metrics collector with default config.
    pub fn new(config: MetricsConfig) -> Self {
        Self {
            config,
            frame_metrics: VecDeque::new(),
            widget_metrics: HashMap::new(),
            memory_metrics: MemoryMetrics::default(),
            warnings: Vec::new(),
            current_frame: None,
            frame_count: 0,
            phase_timers: HashMap::new(),
            text_cache_hits: 0,
            text_cache_misses: 0,
            enabled: true,
        }
    }

    /// Enable or disable metrics collection.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if metrics collection is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get configuration.
    pub fn config(&self) -> &MetricsConfig {
        &self.config
    }

    /// Modify configuration.
    pub fn config_mut(&mut self) -> &mut MetricsConfig {
        &mut self.config
    }

    /// Begin a new frame.
    pub fn begin_frame(&mut self) {
        if !self.enabled {
            return;
        }

        self.frame_count += 1;
        self.current_frame = Some(FrameTimingMetrics::new(self.frame_count));
        self.warnings.clear();
    }

    /// End the current frame and analyze for warnings.
    pub fn end_frame(&mut self) {
        if !self.enabled {
            return;
        }

        if let Some(mut frame) = self.current_frame.take() {
            // Calculate total layout time
            frame.total_layout_time =
                frame.dirty_detection_time + frame.taffy_compute_time + frame.layout_cache_time;

            // Check for warnings
            self.analyze_frame(&frame);

            // Add to history
            self.frame_metrics.push_back(frame);

            // Trim history to configured size
            while self.frame_metrics.len() > self.config.history_size {
                self.frame_metrics.pop_front();
            }
        }
    }

    /// Begin timing a phase.
    pub fn begin_phase(&mut self, phase: MetricsPhase) {
        if !self.enabled {
            return;
        }
        self.phase_timers.insert(phase, Instant::now());
    }

    /// End timing a phase.
    pub fn end_phase(&mut self, phase: MetricsPhase) {
        if !self.enabled {
            return;
        }

        if let Some(start) = self.phase_timers.remove(&phase) {
            let elapsed = start.elapsed();

            if let Some(frame) = &mut self.current_frame {
                match phase {
                    MetricsPhase::DirtyDetection => frame.dirty_detection_time = elapsed,
                    MetricsPhase::TaffyCompute => frame.taffy_compute_time = elapsed,
                    MetricsPhase::LayoutCache => frame.layout_cache_time = elapsed,
                    MetricsPhase::TextShaping => frame.text_shaping_time = elapsed,
                    MetricsPhase::GlyphUpload => frame.glyph_upload_time = elapsed,
                    MetricsPhase::DrawListGeneration => frame.draw_list_time = elapsed,
                    MetricsPhase::InstanceEncoding => frame.instance_encoding_time = elapsed,
                    MetricsPhase::BufferUpload => frame.buffer_upload_time = elapsed,
                    MetricsPhase::DrawCalls => frame.draw_call_time = elapsed,
                    MetricsPhase::EventHandling => frame.event_handling_time = elapsed,
                }
            }
        }
    }

    /// Record dirty node counts for current frame.
    pub fn record_dirty_counts(
        &mut self,
        layout_dirty: usize,
        text_dirty: usize,
        paint_dirty: usize,
        total_nodes: usize,
    ) {
        if !self.enabled {
            return;
        }

        if let Some(frame) = &mut self.current_frame {
            frame.nodes_layout_dirty = layout_dirty;
            frame.nodes_text_dirty = text_dirty;
            frame.nodes_paint_dirty = paint_dirty;
            frame.total_nodes = total_nodes;
        }
    }

    /// Record draw command counts.
    pub fn record_draw_counts(
        &mut self,
        commands: usize,
        quads: usize,
        text: usize,
        images: usize,
    ) {
        if !self.enabled {
            return;
        }

        if let Some(frame) = &mut self.current_frame {
            frame.draw_commands = commands;
            frame.quad_instances = quads;
            frame.text_instances = text;
            frame.image_instances = images;
        }
    }

    /// Update text cache statistics.
    pub fn update_text_cache_stats(&mut self, hits: u64, misses: u64) {
        self.text_cache_hits = hits;
        self.text_cache_misses = misses;
    }

    /// Update memory metrics.
    pub fn update_memory_metrics(&mut self, memory: MemoryMetrics) {
        self.memory_metrics = memory;
    }

    /// Record per-widget dirty flag.
    pub fn record_widget_dirty(&mut self, node_id: NodeId, flags: DirtyFlags) {
        if !self.enabled || !self.config.collect_per_widget_metrics {
            return;
        }

        let metrics = self.widget_metrics.entry(node_id).or_default();
        metrics.dirty_counts.add(flags);
        metrics.last_update_frame = self.frame_count;
    }

    /// Record widget render.
    pub fn record_widget_render(&mut self, node_id: NodeId) {
        if !self.enabled || !self.config.collect_per_widget_metrics {
            return;
        }

        let metrics = self.widget_metrics.entry(node_id).or_default();
        metrics.render_count += 1;
        metrics.last_update_frame = self.frame_count;
    }

    /// Record widget text cache result.
    pub fn record_widget_text_cache(&mut self, node_id: NodeId, hit: bool) {
        if !self.enabled || !self.config.collect_per_widget_metrics {
            return;
        }

        let metrics = self.widget_metrics.entry(node_id).or_default();
        if hit {
            metrics.text_cache_hits += 1;
        } else {
            metrics.text_cache_misses += 1;
        }
    }

    /// Analyze a frame for performance warnings.
    fn analyze_frame(&mut self, frame: &FrameTimingMetrics) {
        // Check for excessive rebuilds
        if frame.total_nodes > 0 {
            let dirty_percent = (frame.nodes_layout_dirty + frame.nodes_text_dirty) as f32
                / frame.total_nodes as f32;
            if dirty_percent > self.config.excessive_rebuild_threshold {
                self.warnings.push(PerformanceWarning::ExcessiveRebuilds {
                    dirty_percent,
                    threshold: self.config.excessive_rebuild_threshold,
                });
            }
        }

        // Check frame time
        let total_time = frame.total_frame_time();
        if total_time > self.config.target_frame_time {
            self.warnings.push(PerformanceWarning::FrameTimeExceeded {
                actual: total_time,
                target: self.config.target_frame_time,
            });
        }

        // Check text cache hit rate
        let total_text_ops = self.text_cache_hits + self.text_cache_misses;
        if total_text_ops > 0 {
            let hit_rate = self.text_cache_hits as f32 / total_text_ops as f32;
            if hit_rate < self.config.min_text_cache_hit_rate {
                self.warnings.push(PerformanceWarning::TextCacheThrashing {
                    hit_rate,
                    min_rate: self.config.min_text_cache_hit_rate,
                });
            }
        }

        // Check memory if tracking enabled
        if self.config.track_memory {
            const MEMORY_THRESHOLD: usize = 100 * 1024 * 1024; // 100MB
            if self.memory_metrics.total_bytes() > MEMORY_THRESHOLD {
                self.warnings.push(PerformanceWarning::HighMemoryUsage {
                    bytes: self.memory_metrics.total_bytes(),
                    threshold: MEMORY_THRESHOLD,
                });
            }
        }
    }

    /// Get active warnings.
    pub fn warnings(&self) -> &[PerformanceWarning] {
        &self.warnings
    }

    /// Get frame timing history.
    pub fn frame_history(&self) -> &VecDeque<FrameTimingMetrics> {
        &self.frame_metrics
    }

    /// Get the most recent frame metrics.
    pub fn current_metrics(&self) -> Option<&FrameTimingMetrics> {
        self.frame_metrics.back()
    }

    /// Get per-widget metrics.
    pub fn widget_metrics(&self) -> &HashMap<NodeId, WidgetMetrics> {
        &self.widget_metrics
    }

    /// Get metrics for a specific widget.
    pub fn get_widget_metrics(&self, node_id: NodeId) -> Option<&WidgetMetrics> {
        self.widget_metrics.get(&node_id)
    }

    /// Get memory metrics.
    pub fn memory_metrics(&self) -> &MemoryMetrics {
        &self.memory_metrics
    }

    /// Get average frame time over history.
    pub fn average_frame_time(&self) -> Duration {
        if self.frame_metrics.is_empty() {
            return Duration::ZERO;
        }

        let total: Duration = self
            .frame_metrics
            .iter()
            .map(|f| f.total_frame_time())
            .sum();
        total / self.frame_metrics.len() as u32
    }

    /// Get average FPS over history.
    pub fn average_fps(&self) -> f32 {
        let avg_time = self.average_frame_time();
        if avg_time.is_zero() {
            0.0
        } else {
            1.0 / avg_time.as_secs_f32()
        }
    }

    /// Get percentage of paint-only frames in history.
    pub fn paint_only_frame_percentage(&self) -> f32 {
        if self.frame_metrics.is_empty() {
            return 0.0;
        }

        let paint_only_count = self
            .frame_metrics
            .iter()
            .filter(|f| f.is_paint_only_frame())
            .count();
        paint_only_count as f32 / self.frame_metrics.len() as f32 * 100.0
    }

    /// Clear all collected metrics.
    pub fn clear(&mut self) {
        self.frame_metrics.clear();
        self.widget_metrics.clear();
        self.warnings.clear();
        self.current_frame = None;
    }

    /// Generate a summary report string.
    pub fn generate_summary(&self) -> String {
        let avg_frame_time = self.average_frame_time();
        let avg_fps = self.average_fps();
        let paint_only = self.paint_only_frame_percentage();

        let latest = self.current_metrics();
        let (layout_dirty, text_dirty, paint_dirty, total_nodes) = if let Some(f) = latest {
            (
                f.nodes_layout_dirty,
                f.nodes_text_dirty,
                f.nodes_paint_dirty,
                f.total_nodes,
            )
        } else {
            (0, 0, 0, 0)
        };

        format!(
            r#"=== UI Performance Summary ===
Frame Time: {:.2}ms avg | FPS: {:.1}
Paint-only frames: {:.1}%
Nodes: {} total | {} layout dirty | {} text dirty | {} paint dirty
Memory: {}
Warnings: {}
"#,
            avg_frame_time.as_secs_f64() * 1000.0,
            avg_fps,
            paint_only,
            total_nodes,
            layout_dirty,
            text_dirty,
            paint_dirty,
            self.memory_metrics.format_summary(),
            self.warnings.len()
        )
    }

    /// Generate detailed timing breakdown for most recent frame.
    pub fn generate_timing_breakdown(&self) -> String {
        let Some(frame) = self.current_metrics() else {
            return "No frame data available".to_string();
        };

        let total = frame.total_frame_time();
        let total_ms = total.as_secs_f64() * 1000.0;

        let fmt_phase = |name: &str, dur: Duration| -> String {
            let ms = dur.as_secs_f64() * 1000.0;
            let pct = if total.is_zero() {
                0.0
            } else {
                dur.as_secs_f64() / total.as_secs_f64() * 100.0
            };
            format!("  {}: {:.3}ms ({:.1}%)", name, ms, pct)
        };

        format!(
            r#"=== Frame {} Timing Breakdown ===
Total: {:.3}ms

Layout:
{}
{}
{}
  Total Layout: {:.3}ms

Text:
{}
{}

Render:
{}
{}
{}
{}

Events:
{}

Counts:
  Draw Commands: {}
  Quad Instances: {}
  Text Instances: {}
  Image Instances: {}
"#,
            frame.frame_id,
            total_ms,
            fmt_phase("Dirty Detection", frame.dirty_detection_time),
            fmt_phase("Taffy Compute", frame.taffy_compute_time),
            fmt_phase("Layout Cache", frame.layout_cache_time),
            frame.total_layout_time.as_secs_f64() * 1000.0,
            fmt_phase("Text Shaping", frame.text_shaping_time),
            fmt_phase("Glyph Upload", frame.glyph_upload_time),
            fmt_phase("Draw List Gen", frame.draw_list_time),
            fmt_phase("Instance Encode", frame.instance_encoding_time),
            fmt_phase("Buffer Upload", frame.buffer_upload_time),
            fmt_phase("Draw Calls", frame.draw_call_time),
            fmt_phase("Event Handling", frame.event_handling_time),
            frame.draw_commands,
            frame.quad_instances,
            frame.text_instances,
            frame.image_instances,
        )
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new(MetricsConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_basic() {
        let mut collector = MetricsCollector::new(MetricsConfig::default());

        collector.begin_frame();
        collector.begin_phase(MetricsPhase::TaffyCompute);
        std::thread::sleep(Duration::from_millis(1));
        collector.end_phase(MetricsPhase::TaffyCompute);
        collector.record_dirty_counts(5, 2, 10, 100);
        collector.end_frame();

        assert_eq!(collector.frame_history().len(), 1);
        let frame = collector.current_metrics().unwrap();
        assert_eq!(frame.nodes_layout_dirty, 5);
        assert_eq!(frame.nodes_text_dirty, 2);
        assert_eq!(frame.total_nodes, 100);
    }

    #[test]
    fn test_frame_timing_metrics() {
        let mut frame = FrameTimingMetrics::new(1);
        frame.nodes_layout_dirty = 10;
        frame.nodes_text_dirty = 5;
        frame.nodes_paint_dirty = 20;
        frame.total_nodes = 100;

        assert_eq!(frame.dirty_percentage(), 35.0);
        assert!(!frame.is_paint_only_frame());

        let paint_only = FrameTimingMetrics {
            nodes_layout_dirty: 0,
            nodes_text_dirty: 0,
            nodes_paint_dirty: 10,
            total_nodes: 100,
            ..Default::default()
        };
        assert!(paint_only.is_paint_only_frame());
    }

    #[test]
    fn test_dirty_flag_counts() {
        let mut counts = DirtyFlagCounts::default();
        counts.add(DirtyFlags::LAYOUT);
        counts.add(DirtyFlags::TEXT_SHAPING);
        counts.add(DirtyFlags::COLOR);

        assert_eq!(counts.layout, 1);
        assert_eq!(counts.text_shaping, 1);
        assert_eq!(counts.color, 1);
        assert_eq!(counts.total(), 3);
    }

    #[test]
    fn test_performance_warnings() {
        let mut collector = MetricsCollector::new(MetricsConfig {
            excessive_rebuild_threshold: 0.3,
            ..Default::default()
        });

        collector.begin_frame();
        collector.record_dirty_counts(50, 10, 5, 100);
        collector.end_frame();

        assert!(!collector.warnings().is_empty());
        assert!(matches!(
            &collector.warnings()[0],
            PerformanceWarning::ExcessiveRebuilds { .. }
        ));
    }

    #[test]
    fn test_memory_metrics() {
        let memory = MemoryMetrics {
            node_arena_bytes: 1024 * 1024,
            instance_buffer_bytes: 512 * 1024,
            glyph_atlas_bytes: 4 * 1024 * 1024,
            draw_list_bytes: 256 * 1024,
            text_cache_bytes: 128 * 1024,
            image_cache_bytes: 64 * 1024,
        };

        assert!(memory.total_bytes() > 5 * 1024 * 1024);
        let summary = memory.format_summary();
        assert!(summary.contains("Memory:"));
    }

    #[test]
    fn test_widget_metrics() {
        let mut collector = MetricsCollector::new(MetricsConfig::default());
        let node_id = NodeId(42);

        collector.record_widget_dirty(node_id, DirtyFlags::LAYOUT);
        collector.record_widget_render(node_id);
        collector.record_widget_text_cache(node_id, true);
        collector.record_widget_text_cache(node_id, true);
        collector.record_widget_text_cache(node_id, false);

        let metrics = collector.get_widget_metrics(node_id).unwrap();
        assert_eq!(metrics.render_count, 1);
        assert_eq!(metrics.dirty_counts.layout, 1);
        assert_eq!(metrics.text_cache_hits, 2);
        assert_eq!(metrics.text_cache_misses, 1);
        assert!((metrics.text_cache_hit_rate() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_average_calculations() {
        let mut collector = MetricsCollector::new(MetricsConfig::default());

        for _ in 0..10 {
            collector.begin_frame();
            // Record some phase time so total_frame_time is non-zero
            collector.begin_phase(MetricsPhase::TaffyCompute);
            std::thread::sleep(Duration::from_millis(1));
            collector.end_phase(MetricsPhase::TaffyCompute);
            collector.end_frame();
        }

        assert_eq!(collector.frame_history().len(), 10);
        // average_frame_time should be positive if we recorded layout time
        assert!(collector.average_frame_time() > Duration::ZERO);
        assert!(collector.average_fps() > 0.0);
    }

    #[test]
    fn test_disabled_collector() {
        let mut collector = MetricsCollector::new(MetricsConfig::default());
        collector.set_enabled(false);

        collector.begin_frame();
        collector.record_dirty_counts(100, 50, 25, 200);
        collector.end_frame();

        assert!(collector.frame_history().is_empty());
    }

    #[test]
    fn test_warning_severity() {
        let warning = PerformanceWarning::ExcessiveRebuilds {
            dirty_percent: 0.9,
            threshold: 0.5,
        };
        assert_eq!(warning.severity(), 2);

        let warning = PerformanceWarning::ExcessiveRebuilds {
            dirty_percent: 0.6,
            threshold: 0.5,
        };
        assert_eq!(warning.severity(), 1);
    }
}
