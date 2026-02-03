//! Performance metrics and instrumentation for UI system.

use std::time::{Duration, Instant};

use crate::dirty::DirtyFlags;
use crate::tree::NodeId;

/// Performance metrics collected during UI updates.
#[derive(Debug, Clone, Default)]
pub struct UiMetrics {
    /// Time spent computing layout with Taffy
    pub layout_time: Duration,

    /// Time spent shaping text
    pub text_shape_time: Duration,

    /// Time spent building geometry batches
    pub build_batches_time: Duration,

    /// Time spent uploading to GPU
    pub gpu_upload_time: Duration,

    /// Total frame time for UI update
    pub total_time: Duration,

    /// Number of nodes with LAYOUT dirty flag
    pub nodes_layout_dirty: usize,

    /// Number of nodes with TEXT_SHAPING dirty flag
    pub nodes_text_dirty: usize,

    /// Number of nodes with COLOR dirty flag
    pub nodes_paint_dirty: usize,

    /// Number of nodes with GEOMETRY dirty flag
    pub nodes_geometry_dirty: usize,

    /// Total number of nodes in the tree
    pub total_nodes: usize,

    /// Number of layout computations skipped due to clean flags
    pub layout_skips: usize,

    /// Number of text shaping operations skipped (cache hits)
    pub text_cache_hits: usize,

    /// Number of text shaping operations performed (cache misses)
    pub text_cache_misses: usize,

    /// Number of glyph atlas evictions
    pub atlas_evictions: usize,

    /// Number of instance buffer updates
    pub instance_updates: usize,
}

impl UiMetrics {
    /// Create new empty metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate cache hit rate for text shaping (0.0 to 1.0).
    pub fn text_cache_hit_rate(&self) -> f32 {
        let total = self.text_cache_hits + self.text_cache_misses;
        if total == 0 {
            0.0
        } else {
            self.text_cache_hits as f32 / total as f32
        }
    }

    /// Calculate percentage of nodes that needed layout recomputation.
    pub fn layout_dirty_percentage(&self) -> f32 {
        if self.total_nodes == 0 {
            0.0
        } else {
            (self.nodes_layout_dirty as f32 / self.total_nodes as f32) * 100.0
        }
    }

    /// Calculate percentage of nodes that only needed paint updates.
    pub fn paint_only_percentage(&self) -> f32 {
        if self.total_nodes == 0 {
            0.0
        } else {
            (self.nodes_paint_dirty as f32 / self.total_nodes as f32) * 100.0
        }
    }

    /// Returns true if this was a paint-only frame (no layout work).
    pub fn is_paint_only_frame(&self) -> bool {
        self.nodes_layout_dirty == 0 && self.nodes_text_dirty == 0
    }

    /// Format metrics as a human-readable string.
    pub fn format_summary(&self) -> String {
        format!(
            "UI Frame: {:.2}ms | Layout: {:.2}ms ({} nodes) | Text: {:.2}ms ({} cache hits) | Paint: {} nodes",
            self.total_time.as_secs_f64() * 1000.0,
            self.layout_time.as_secs_f64() * 1000.0,
            self.nodes_layout_dirty,
            self.text_shape_time.as_secs_f64() * 1000.0,
            self.text_cache_hits,
            self.nodes_paint_dirty,
        )
    }

    /// Format detailed metrics for debugging.
    pub fn format_detailed(&self) -> String {
        format!(
            r#"UI Performance Metrics:
  Total Time: {:.3}ms
  Layout Time: {:.3}ms ({}% of total)
  Text Shaping: {:.3}ms ({}% of total)
  Batch Building: {:.3}ms ({}% of total)
  GPU Upload: {:.3}ms ({}% of total)

  Dirty Nodes:
    Layout: {} / {} ({:.1}%)
    Text: {}
    Paint Only: {}
    Geometry: {}

  Text Cache:
    Hits: {}
    Misses: {}
    Hit Rate: {:.1}%

  Optimizations:
    Layout Skips: {}
    Instance Updates: {}
    Atlas Evictions: {}
"#,
            self.total_time.as_secs_f64() * 1000.0,
            self.layout_time.as_secs_f64() * 1000.0,
            self.percentage_of_total(self.layout_time),
            self.text_shape_time.as_secs_f64() * 1000.0,
            self.percentage_of_total(self.text_shape_time),
            self.build_batches_time.as_secs_f64() * 1000.0,
            self.percentage_of_total(self.build_batches_time),
            self.gpu_upload_time.as_secs_f64() * 1000.0,
            self.percentage_of_total(self.gpu_upload_time),
            self.nodes_layout_dirty,
            self.total_nodes,
            self.layout_dirty_percentage(),
            self.nodes_text_dirty,
            self.nodes_paint_dirty,
            self.nodes_geometry_dirty,
            self.text_cache_hits,
            self.text_cache_misses,
            self.text_cache_hit_rate() * 100.0,
            self.layout_skips,
            self.instance_updates,
            self.atlas_evictions,
        )
    }

    fn percentage_of_total(&self, duration: Duration) -> u32 {
        if self.total_time.as_nanos() == 0 {
            0
        } else {
            ((duration.as_nanos() as f64 / self.total_time.as_nanos() as f64) * 100.0) as u32
        }
    }
}

/// Helper for timing sections of UI code.
pub struct MetricsTimer {
    start: Instant,
}

impl MetricsTimer {
    /// Start a new timer.
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Stop the timer and return elapsed duration.
    pub fn stop(self) -> Duration {
        self.start.elapsed()
    }
}

/// Accumulator for collecting dirty flag statistics.
#[derive(Debug, Clone, Default)]
pub struct DirtyStats {
    pub layout_count: usize,
    pub text_count: usize,
    pub paint_count: usize,
    pub geometry_count: usize,
}

impl DirtyStats {
    /// Create new empty stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node's dirty flags to the statistics.
    pub fn add_node(&mut self, flags: DirtyFlags) {
        if flags.needs_layout() {
            self.layout_count += 1;
        }
        if flags.needs_text_shaping() {
            self.text_count += 1;
        }
        if flags.is_paint_only() {
            self.paint_count += 1;
        }
        if flags.needs_geometry_rebuild() {
            self.geometry_count += 1;
        }
    }
}

/// Per-node performance information for debugging.
#[derive(Debug, Clone)]
pub struct NodeMetrics {
    pub node_id: NodeId,
    pub dirty_flags: DirtyFlags,
    pub layout_time: Duration,
    pub text_time: Duration,
    pub paint_time: Duration,
}

impl NodeMetrics {
    /// Create new node metrics.
    pub fn new(node_id: NodeId, dirty_flags: DirtyFlags) -> Self {
        Self {
            node_id,
            dirty_flags,
            layout_time: Duration::ZERO,
            text_time: Duration::ZERO,
            paint_time: Duration::ZERO,
        }
    }

    /// Total time spent on this node.
    pub fn total_time(&self) -> Duration {
        self.layout_time + self.text_time + self.paint_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_default() {
        let metrics = UiMetrics::new();
        assert_eq!(metrics.total_nodes, 0);
        assert_eq!(metrics.nodes_layout_dirty, 0);
        assert_eq!(metrics.text_cache_hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_hit_rate() {
        let mut metrics = UiMetrics::new();
        metrics.text_cache_hits = 8;
        metrics.text_cache_misses = 2;
        assert_eq!(metrics.text_cache_hit_rate(), 0.8);
    }

    #[test]
    fn test_paint_only_frame() {
        let mut metrics = UiMetrics::new();
        metrics.nodes_paint_dirty = 5;
        assert!(metrics.is_paint_only_frame());

        metrics.nodes_layout_dirty = 1;
        assert!(!metrics.is_paint_only_frame());
    }

    #[test]
    fn test_dirty_stats() {
        let mut stats = DirtyStats::new();
        stats.add_node(DirtyFlags::LAYOUT);
        stats.add_node(DirtyFlags::COLOR);
        stats.add_node(DirtyFlags::TEXT_SHAPING);

        assert_eq!(stats.layout_count, 2); // LAYOUT + TEXT_SHAPING
        assert_eq!(stats.text_count, 1);
        assert_eq!(stats.paint_count, 1);
    }

    #[test]
    fn test_timer() {
        let timer = MetricsTimer::start();
        std::thread::sleep(Duration::from_millis(10));
        let elapsed = timer.stop();
        assert!(elapsed >= Duration::from_millis(10));
    }
}
