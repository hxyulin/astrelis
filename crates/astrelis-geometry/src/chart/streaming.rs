//! Streaming chart utilities for live/real-time data visualization.
//!
//! This module provides helpers for efficiently updating charts with
//! streaming data, such as sensor readings or real-time metrics.
//!
//! # Features
//!
//! - **RingBuffer integration**: Efficient O(1) push for streaming data
//! - **Auto-scrolling**: Automatically adjust view window for live data
//! - **LTTB downsampling**: Preserve visual features with reduced point count
//! - **GPU acceleration**: Optional GPU-instanced line rendering for large datasets
//!
//! # Example
//!
//! ```ignore
//! let mut streaming = StreamingChart::new(chart);
//!
//! // In update loop:
//! streaming.push_time_point(0, timestamp, temperature);
//! streaming.auto_scroll(AxisId::X_PRIMARY, 60.0); // Show last 60 seconds
//!
//! // Before rendering:
//! let result = streaming.prepare_render(&bounds);
//! let display_data = streaming.get_display_data(0, pixel_width);
//! ```
//!
//! # GPU-Accelerated Streaming
//!
//! For high-performance streaming with large datasets:
//!
//! ```ignore
//! use astrelis_render::GraphicsContext;
//!
//! let context: Arc<GraphicsContext> = /* ... */;
//! let mut gpu_streaming = GpuStreamingChart::new(chart, context, surface_format);
//!
//! // Push data as usual
//! gpu_streaming.push_time_point(0, timestamp, value);
//!
//! // Prepare GPU buffers (only rebuilds if data changed)
//! gpu_streaming.prepare_render(&bounds);
//!
//! // In render pass:
//! gpu_streaming.render(pass, viewport, &bounds);
//! ```

use super::cache::{ChartCache, ChartDirtyFlags};
use super::data::downsample_data;
use super::rect::Rect;
use super::renderers::{
    GpuChartAreaRenderer, GpuChartBarRenderer, GpuChartLineRenderer, GpuChartScatterRenderer,
    GPU_RENDER_THRESHOLD,
};
use super::types::{AxisId, Chart, ChartType, DataPoint};
use astrelis_render::{wgpu, GraphicsContext, Viewport};
use std::sync::Arc;

#[cfg(feature = "chart-text")]
use super::text::ChartTextRenderer;

#[cfg(feature = "chart-text")]
use astrelis_text::FontSystem;

/// Per-series dirty tracking for efficient partial updates.
#[derive(Debug, Clone, Default)]
pub struct SeriesDirtyState {
    /// Number of points when last rendered.
    pub last_rendered_count: usize,
    /// Total points ever pushed (for ring buffer tracking).
    pub total_pushed: u64,
    /// Whether the series needs full rebuild.
    pub needs_full_rebuild: bool,
    /// Range of indices that need updating.
    pub dirty_range: Option<(usize, usize)>,
}

/// Result of prepare_render operation.
#[derive(Debug, Clone)]
pub struct PrepareResult {
    /// Whether any series was updated.
    pub updated: bool,
    /// Per-series update info.
    pub series_updates: Vec<SeriesUpdateInfo>,
}

/// Information about a series update.
#[derive(Debug, Clone)]
pub struct SeriesUpdateInfo {
    /// Series index.
    pub index: usize,
    /// Whether it was a full rebuild.
    pub full_rebuild: bool,
    /// Number of new points (for append).
    pub new_points: usize,
}

/// A wrapper around Chart that automatically manages caching for live updates.
///
/// `StreamingChart` tracks dirty state and provides efficient methods for
/// updating chart data in real-time scenarios.
///
/// # Example
///
/// ```ignore
/// use astrelis_geometry::chart::*;
///
/// // Create a streaming chart for sensor data
/// let chart = ChartBuilder::line()
///     .add_series("Temperature", &[])
///     .interactive(true)
///     .build();
///
/// let mut streaming = StreamingChart::new(chart);
///
/// // In your update loop:
/// streaming.push_point(0, DataPoint::new(timestamp, temperature), Some(1000));
///
/// // Before rendering:
/// streaming.prepare_render(&bounds);
///
/// // Render using cached data
/// chart_renderer.draw(streaming.chart(), bounds);
/// ```
#[derive(Debug)]
pub struct StreamingChart {
    chart: Chart,
    cache: ChartCache,
    /// Per-series dirty tracking for efficient updates.
    series_dirty: Vec<SeriesDirtyState>,
    /// Auto-scroll configuration per axis.
    auto_scroll_config: Vec<(AxisId, f64)>,
}

impl StreamingChart {
    /// Create a new streaming chart wrapper.
    pub fn new(chart: Chart) -> Self {
        let series_count = chart.series.len();
        Self {
            chart,
            cache: ChartCache::new(),
            series_dirty: vec![SeriesDirtyState::default(); series_count],
            auto_scroll_config: Vec::new(),
        }
    }

    /// Get a reference to the underlying chart.
    pub fn chart(&self) -> &Chart {
        &self.chart
    }

    /// Get a mutable reference to the underlying chart.
    ///
    /// Note: Direct mutations bypass dirty tracking. Prefer using the
    /// streaming-specific methods when possible.
    pub fn chart_mut(&mut self) -> &mut Chart {
        self.cache.invalidate();
        &mut self.chart
    }

    /// Get a reference to the cache.
    pub fn cache(&self) -> &ChartCache {
        &self.cache
    }

    /// Get a mutable reference to the cache.
    pub fn cache_mut(&mut self) -> &mut ChartCache {
        &mut self.cache
    }

    /// Check if the cache needs to be rebuilt before rendering.
    pub fn needs_rebuild(&self) -> bool {
        self.cache.needs_rebuild()
    }

    /// Prepare the chart for rendering by rebuilding the cache if needed.
    pub fn prepare_render(&mut self, bounds: &Rect) {
        if self.cache.needs_rebuild() {
            self.cache.rebuild(&self.chart, bounds);
        }
    }

    /// Append data points to a series.
    ///
    /// This method tracks the change for efficient partial cache updates.
    pub fn append_data(&mut self, series_idx: usize, points: &[DataPoint]) {
        let old_len = self.chart.series_len(series_idx);
        self.chart.append_data(series_idx, points);
        let new_len = self.chart.series_len(series_idx);

        if new_len > old_len {
            self.cache.mark_data_appended(series_idx, new_len);
        }
    }

    /// Push a single point with optional sliding window.
    ///
    /// If `max_points` causes data to be removed, this marks as a full
    /// data change. Otherwise, it marks as an append for partial updates.
    pub fn push_point(&mut self, series_idx: usize, point: DataPoint, max_points: Option<usize>) {
        let old_len = self.chart.series_len(series_idx);
        self.chart.push_point(series_idx, point, max_points);
        let new_len = self.chart.series_len(series_idx);

        // If data was removed (sliding window), need full rebuild
        if new_len <= old_len && max_points.is_some() {
            self.cache.mark_data_changed();
        } else {
            self.cache.mark_data_appended(series_idx, new_len);
        }
    }

    /// Replace all data in a series.
    pub fn set_data(&mut self, series_idx: usize, data: Vec<DataPoint>) {
        self.chart.set_data(series_idx, data);
        self.cache.mark_data_changed();
    }

    /// Clear all data from a series.
    pub fn clear_data(&mut self, series_idx: usize) {
        self.chart.clear_data(series_idx);
        self.cache.mark_data_changed();
    }

    /// Notify that the view has changed (pan/zoom).
    ///
    /// Call this after modifying `chart.interactive.pan_offset` or
    /// `chart.interactive.zoom`.
    pub fn mark_view_changed(&mut self) {
        self.cache.mark_view_changed();
    }

    /// Notify that style has changed.
    pub fn mark_style_changed(&mut self) {
        self.cache.mark_style_changed();
    }

    /// Notify that axes have changed.
    pub fn mark_axes_changed(&mut self) {
        self.cache.mark_axes_changed();
    }

    /// Notify that bounds have changed.
    pub fn mark_bounds_changed(&mut self) {
        self.cache.mark_bounds_changed();
    }

    /// Get the current dirty flags.
    pub fn dirty_flags(&self) -> ChartDirtyFlags {
        self.cache.dirty_flags()
    }

    /// Manually clear dirty flags (normally done by prepare_render).
    pub fn clear_dirty(&mut self) {
        self.cache.clear_dirty();
    }

    // =========================================================================
    // Enhanced Streaming API
    // =========================================================================

    /// Push a time/value point to a series.
    ///
    /// Convenient method for time series data.
    pub fn push_time_point(&mut self, series_idx: usize, time: f64, value: f64) {
        self.push_point(series_idx, DataPoint::new(time, value), None);
    }

    /// Push a time/value point with sliding window.
    pub fn push_time_point_windowed(
        &mut self,
        series_idx: usize,
        time: f64,
        value: f64,
        max_points: usize,
    ) {
        self.push_point(series_idx, DataPoint::new(time, value), Some(max_points));
    }

    /// Configure auto-scrolling for an axis.
    ///
    /// When enabled, the axis range will automatically shift to show
    /// the most recent `window_size` units of data.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Show the last 60 seconds of data
    /// streaming.auto_scroll(AxisId::X_PRIMARY, 60.0);
    /// ```
    pub fn auto_scroll(&mut self, axis_id: AxisId, window_size: f64) {
        // Remove existing config for this axis
        self.auto_scroll_config.retain(|(id, _)| *id != axis_id);
        self.auto_scroll_config.push((axis_id, window_size));
    }

    /// Disable auto-scrolling for an axis.
    pub fn disable_auto_scroll(&mut self, axis_id: AxisId) {
        self.auto_scroll_config.retain(|(id, _)| *id != axis_id);
    }

    /// Apply auto-scroll to update axis ranges.
    ///
    /// Call this after pushing data to adjust the view window.
    pub fn apply_auto_scroll(&mut self) {
        for (axis_id, window_size) in &self.auto_scroll_config {
            // Find the maximum data value for this axis
            let max_value = self.find_max_for_axis(*axis_id);

            if let Some(max) = max_value {
                // Update the axis range to show a window ending at max
                if let Some(axis) = self.chart.get_axis_mut(*axis_id) {
                    axis.min = Some(max - window_size);
                    axis.max = Some(max);
                }
                self.cache.mark_view_changed();
            }
        }
    }

    /// Find the maximum data value for an axis across all series.
    fn find_max_for_axis(&self, axis_id: AxisId) -> Option<f64> {
        let mut max = None;

        for series in &self.chart.series {
            // Check if this series uses this axis
            let uses_axis = series.x_axis == axis_id || series.y_axis == axis_id;
            if !uses_axis {
                continue;
            }

            if let Some(last_point) = series.data.last() {
                let value = if series.x_axis == axis_id {
                    last_point.x
                } else {
                    last_point.y
                };

                max = Some(max.map_or(value, |m: f64| m.max(value)));
            }
        }

        max
    }

    /// Prepare for rendering and return detailed update information.
    pub fn prepare_render_with_result(&mut self, bounds: &Rect) -> PrepareResult {
        // Ensure series_dirty has correct size
        while self.series_dirty.len() < self.chart.series.len() {
            self.series_dirty.push(SeriesDirtyState::default());
        }

        // Apply auto-scroll first
        self.apply_auto_scroll();

        let updated = self.cache.needs_rebuild();

        // Collect update info
        let series_updates: Vec<SeriesUpdateInfo> = self
            .chart
            .series
            .iter()
            .enumerate()
            .filter_map(|(idx, series)| {
                let dirty_state = &self.series_dirty[idx];
                if series.data.len() != dirty_state.last_rendered_count {
                    Some(SeriesUpdateInfo {
                        index: idx,
                        full_rebuild: dirty_state.needs_full_rebuild,
                        new_points: series.data.len().saturating_sub(dirty_state.last_rendered_count),
                    })
                } else {
                    None
                }
            })
            .collect();

        // Rebuild cache if needed
        if self.cache.needs_rebuild() {
            self.cache.rebuild(&self.chart, bounds);
        }

        // Update dirty state
        for (idx, series) in self.chart.series.iter().enumerate() {
            if idx < self.series_dirty.len() {
                self.series_dirty[idx].last_rendered_count = series.data.len();
                self.series_dirty[idx].needs_full_rebuild = false;
                self.series_dirty[idx].dirty_range = None;
            }
        }

        PrepareResult {
            updated,
            series_updates,
        }
    }

    /// Get display data for a series, downsampled if necessary.
    ///
    /// Uses LTTB (Largest Triangle Three Buckets) algorithm to preserve
    /// visual features while reducing point count for rendering.
    ///
    /// # Arguments
    ///
    /// * `series_idx` - Index of the series
    /// * `pixel_width` - Width of the display area in pixels
    ///
    /// # Returns
    ///
    /// Downsampled data points optimized for the given pixel width.
    /// If the data has fewer points than pixels, returns all points.
    pub fn get_display_data(&self, series_idx: usize, pixel_width: f32) -> Vec<DataPoint> {
        let Some(series) = self.chart.series.get(series_idx) else {
            return Vec::new();
        };

        let target_points = (pixel_width * 2.0) as usize; // 2 points per pixel max

        if series.data.len() <= target_points {
            series.data.clone()
        } else {
            downsample_data(&series.data, target_points)
        }
    }

    /// Get downsampled data for all series.
    pub fn get_all_display_data(&self, pixel_width: f32) -> Vec<Vec<DataPoint>> {
        (0..self.chart.series.len())
            .map(|idx| self.get_display_data(idx, pixel_width))
            .collect()
    }

    /// Get statistics about the streaming data.
    pub fn statistics(&self) -> StreamingStatistics {
        let total_points: usize = self.chart.series.iter().map(|s| s.data.len()).sum();
        let series_counts: Vec<usize> = self.chart.series.iter().map(|s| s.data.len()).collect();

        StreamingStatistics {
            total_points,
            series_counts,
            cache_dirty: self.cache.needs_rebuild(),
            auto_scroll_active: !self.auto_scroll_config.is_empty(),
        }
    }
}

/// Statistics about streaming chart data.
#[derive(Debug, Clone)]
pub struct StreamingStatistics {
    /// Total data points across all series.
    pub total_points: usize,
    /// Data points per series.
    pub series_counts: Vec<usize>,
    /// Whether cache needs rebuild.
    pub cache_dirty: bool,
    /// Whether auto-scroll is active.
    pub auto_scroll_active: bool,
}

impl From<Chart> for StreamingChart {
    fn from(chart: Chart) -> Self {
        Self::new(chart)
    }
}

impl std::ops::Deref for StreamingChart {
    type Target = Chart;

    fn deref(&self) -> &Self::Target {
        &self.chart
    }
}

/// Configuration for a sliding window chart.
#[derive(Debug, Clone, Copy)]
pub struct SlidingWindowConfig {
    /// Maximum number of points to keep per series.
    pub max_points: usize,
    /// Whether to auto-scale axes based on visible data.
    pub auto_scale: bool,
}

impl Default for SlidingWindowConfig {
    fn default() -> Self {
        Self {
            max_points: 1000,
            auto_scale: true,
        }
    }
}

impl SlidingWindowConfig {
    /// Create a new config with the specified max points.
    pub fn new(max_points: usize) -> Self {
        Self {
            max_points,
            auto_scale: true,
        }
    }

    /// Set whether to auto-scale axes.
    pub fn with_auto_scale(mut self, auto_scale: bool) -> Self {
        self.auto_scale = auto_scale;
        self
    }
}

// =============================================================================
// GPU-Accelerated Streaming Chart
// =============================================================================

/// GPU-accelerated streaming chart for high-performance live data visualization.
///
/// Combines `StreamingChart` functionality with GPU-accelerated renderers for
/// efficient rendering of large datasets. The GPU path is automatically used
/// when series exceed `GPU_RENDER_THRESHOLD` points.
///
/// Supports all chart types:
/// - **Line**: GPU-instanced line segments via `GpuChartLineRenderer`
/// - **Scatter**: GPU-instanced points via `GpuChartScatterRenderer`
/// - **Bar**: GPU-instanced quads via `GpuChartBarRenderer`
/// - **Area**: GPU-instanced fill + lines via `GpuChartAreaRenderer`
///
/// # Performance Characteristics
///
/// | Operation | CPU Path | GPU Path |
/// |-----------|----------|----------|
/// | Data append | O(n) tessellation | O(n) buffer upload (once) |
/// | Pan/zoom | O(n) tessellation | O(1) uniform update |
/// | Render | O(n) triangles | O(n) instances |
///
/// # Text Rendering (with `chart-text` feature)
///
/// When the `chart-text` feature is enabled, use `with_text()` to add text rendering:
///
/// ```ignore
/// let font_system = FontSystem::with_system_fonts();
/// let gpu_chart = GpuStreamingChart::new(chart, context.clone(), surface_format)
///     .with_text(context.clone(), font_system);
/// ```
///
/// # Example
///
/// ```ignore
/// use astrelis_geometry::chart::*;
/// use astrelis_render::GraphicsContext;
/// use std::sync::Arc;
///
/// let context: Arc<GraphicsContext> = /* ... */;
/// let surface_format = window.surface_format();
/// let chart = ChartBuilder::line()
///     .add_series("Sensor", &[])
///     .interactive(true)
///     .build();
///
/// let mut gpu_chart = GpuStreamingChart::new(chart, context, surface_format);
///
/// // In update loop:
/// gpu_chart.push_time_point(0, time, value);
/// gpu_chart.auto_scroll(AxisId::X_PRIMARY, 60.0);
///
/// // Before rendering:
/// let bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
/// gpu_chart.prepare_render(&bounds);
///
/// // In render pass:
/// gpu_chart.render(&mut pass, viewport, &bounds);
/// ```
pub struct GpuStreamingChart {
    /// Underlying streaming chart.
    streaming: StreamingChart,
    /// GPU line renderer for accelerated line chart rendering.
    line_renderer: GpuChartLineRenderer,
    /// GPU scatter renderer for accelerated scatter chart rendering.
    scatter_renderer: GpuChartScatterRenderer,
    /// GPU bar renderer for accelerated bar chart rendering.
    bar_renderer: GpuChartBarRenderer,
    /// GPU area renderer for accelerated area chart rendering.
    area_renderer: GpuChartAreaRenderer,
    /// Whether GPU rendering is enabled (auto-detected based on data size).
    gpu_enabled: bool,
    /// Force GPU rendering regardless of data size.
    force_gpu: bool,
    /// Optional text renderer for titles, labels, and legends.
    #[cfg(feature = "chart-text")]
    text_renderer: Option<ChartTextRenderer>,
}

impl std::fmt::Debug for GpuStreamingChart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("GpuStreamingChart");
        s.field("streaming", &self.streaming)
            .field("line_renderer", &self.line_renderer)
            .field("scatter_renderer", &self.scatter_renderer)
            .field("bar_renderer", &self.bar_renderer)
            .field("area_renderer", &self.area_renderer)
            .field("gpu_enabled", &self.gpu_enabled)
            .field("force_gpu", &self.force_gpu);
        #[cfg(feature = "chart-text")]
        s.field("has_text_renderer", &self.text_renderer.is_some());
        s.finish()
    }
}

impl GpuStreamingChart {
    /// Create a new GPU-accelerated streaming chart.
    ///
    /// The `target_format` must match the render target this chart will draw into.
    pub fn new(
        chart: Chart,
        context: Arc<GraphicsContext>,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            streaming: StreamingChart::new(chart),
            line_renderer: GpuChartLineRenderer::new(context.clone(), target_format),
            scatter_renderer: GpuChartScatterRenderer::new(context.clone(), target_format),
            bar_renderer: GpuChartBarRenderer::new(context.clone(), target_format),
            area_renderer: GpuChartAreaRenderer::new(context, target_format),
            gpu_enabled: false,
            force_gpu: false,
            #[cfg(feature = "chart-text")]
            text_renderer: None,
        }
    }

    /// Create from an existing streaming chart.
    ///
    /// The `target_format` must match the render target this chart will draw into.
    pub fn from_streaming(
        streaming: StreamingChart,
        context: Arc<GraphicsContext>,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            streaming,
            line_renderer: GpuChartLineRenderer::new(context.clone(), target_format),
            scatter_renderer: GpuChartScatterRenderer::new(context.clone(), target_format),
            bar_renderer: GpuChartBarRenderer::new(context.clone(), target_format),
            area_renderer: GpuChartAreaRenderer::new(context, target_format),
            gpu_enabled: false,
            force_gpu: false,
            #[cfg(feature = "chart-text")]
            text_renderer: None,
        }
    }

    /// Enable text rendering for titles, labels, and legends.
    ///
    /// Requires the `chart-text` feature.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::FontSystem;
    ///
    /// let font_system = FontSystem::with_system_fonts();
    /// let gpu_chart = GpuStreamingChart::new(chart, context.clone(), surface_format)
    ///     .with_text(context.clone(), font_system);
    /// ```
    #[cfg(feature = "chart-text")]
    pub fn with_text(mut self, context: Arc<GraphicsContext>, font_system: FontSystem) -> Self {
        self.text_renderer = Some(ChartTextRenderer::new(context, font_system));
        self
    }

    /// Get a reference to the text renderer if enabled.
    #[cfg(feature = "chart-text")]
    pub fn text_renderer(&self) -> Option<&ChartTextRenderer> {
        self.text_renderer.as_ref()
    }

    /// Get a mutable reference to the text renderer if enabled.
    #[cfg(feature = "chart-text")]
    pub fn text_renderer_mut(&mut self) -> Option<&mut ChartTextRenderer> {
        self.text_renderer.as_mut()
    }

    /// Force GPU rendering regardless of data size.
    ///
    /// Useful for testing or when you know data will grow large.
    pub fn force_gpu_rendering(mut self, force: bool) -> Self {
        self.force_gpu = force;
        self
    }

    /// Get a reference to the underlying chart.
    pub fn chart(&self) -> &Chart {
        self.streaming.chart()
    }

    /// Get a mutable reference to the underlying chart.
    pub fn chart_mut(&mut self) -> &mut Chart {
        self.streaming.chart_mut()
    }

    /// Get a reference to the underlying streaming chart.
    pub fn streaming(&self) -> &StreamingChart {
        &self.streaming
    }

    /// Get a mutable reference to the underlying streaming chart.
    pub fn streaming_mut(&mut self) -> &mut StreamingChart {
        &mut self.streaming
    }

    /// Get a reference to the GPU line renderer.
    pub fn line_renderer(&self) -> &GpuChartLineRenderer {
        &self.line_renderer
    }

    /// Get a mutable reference to the GPU line renderer.
    pub fn line_renderer_mut(&mut self) -> &mut GpuChartLineRenderer {
        &mut self.line_renderer
    }

    /// Get a reference to the GPU scatter renderer.
    pub fn scatter_renderer(&self) -> &GpuChartScatterRenderer {
        &self.scatter_renderer
    }

    /// Get a reference to the GPU bar renderer.
    pub fn bar_renderer(&self) -> &GpuChartBarRenderer {
        &self.bar_renderer
    }

    /// Get a reference to the GPU area renderer.
    pub fn area_renderer(&self) -> &GpuChartAreaRenderer {
        &self.area_renderer
    }

    /// Check if GPU rendering is currently enabled.
    pub fn is_gpu_enabled(&self) -> bool {
        self.gpu_enabled
    }

    // =========================================================================
    // Data Operations (delegated to StreamingChart)
    // =========================================================================

    /// Append data points to a series.
    pub fn append_data(&mut self, series_idx: usize, points: &[DataPoint]) {
        self.streaming.append_data(series_idx, points);
        self.mark_all_renderers_data_changed();
    }

    /// Push a single point with optional sliding window.
    pub fn push_point(&mut self, series_idx: usize, point: DataPoint, max_points: Option<usize>) {
        self.streaming.push_point(series_idx, point, max_points);
        self.mark_all_renderers_data_changed();
    }

    /// Replace all data in a series.
    pub fn set_data(&mut self, series_idx: usize, data: Vec<DataPoint>) {
        self.streaming.set_data(series_idx, data);
        self.mark_all_renderers_data_changed();
    }

    /// Clear all data from a series.
    pub fn clear_data(&mut self, series_idx: usize) {
        self.streaming.clear_data(series_idx);
        self.mark_all_renderers_data_changed();
    }

    /// Push a time/value point to a series.
    pub fn push_time_point(&mut self, series_idx: usize, time: f64, value: f64) {
        self.streaming.push_time_point(series_idx, time, value);
        self.mark_all_renderers_data_changed();
    }

    /// Push a time/value point with sliding window.
    pub fn push_time_point_windowed(
        &mut self,
        series_idx: usize,
        time: f64,
        value: f64,
        max_points: usize,
    ) {
        self.streaming
            .push_time_point_windowed(series_idx, time, value, max_points);
        self.mark_all_renderers_data_changed();
    }

    /// Mark all GPU renderers as needing data update.
    fn mark_all_renderers_data_changed(&mut self) {
        self.line_renderer.mark_data_changed();
        self.scatter_renderer.mark_data_changed();
        self.bar_renderer.mark_data_changed();
        self.area_renderer.mark_data_changed();
    }

    /// Configure auto-scrolling for an axis.
    pub fn auto_scroll(&mut self, axis_id: AxisId, window_size: f64) {
        self.streaming.auto_scroll(axis_id, window_size);
    }

    /// Disable auto-scrolling for an axis.
    pub fn disable_auto_scroll(&mut self, axis_id: AxisId) {
        self.streaming.disable_auto_scroll(axis_id);
    }

    /// Notify that the view has changed (pan/zoom).
    pub fn mark_view_changed(&mut self) {
        self.streaming.mark_view_changed();
    }

    /// Notify that style has changed.
    pub fn mark_style_changed(&mut self) {
        self.streaming.mark_style_changed();
    }

    /// Notify that bounds have changed (widget resized).
    pub fn mark_bounds_changed(&mut self) {
        self.streaming.mark_bounds_changed();
    }

    /// Get the current dirty flags.
    pub fn dirty_flags(&self) -> ChartDirtyFlags {
        self.streaming.dirty_flags()
    }

    // =========================================================================
    // Rendering
    // =========================================================================

    /// Check if GPU rendering should be used based on current data size.
    fn should_use_gpu(&self) -> bool {
        if self.force_gpu {
            return true;
        }

        // Check if any series exceeds threshold
        self.streaming
            .chart()
            .series
            .iter()
            .any(|s| s.data.len() > GPU_RENDER_THRESHOLD)
    }

    /// Prepare the chart for rendering.
    ///
    /// This prepares both the coordinate cache and GPU buffers as needed.
    /// Call this once per frame before rendering.
    pub fn prepare_render(&mut self, bounds: &Rect) {
        // Prepare coordinate cache
        self.streaming.prepare_render(bounds);

        // Check if we should enable GPU rendering
        self.gpu_enabled = self.should_use_gpu();

        // Prepare the appropriate GPU renderer based on chart type
        if self.gpu_enabled {
            self.prepare_gpu_renderer();
        }
    }

    /// Prepare with detailed result information.
    pub fn prepare_render_with_result(&mut self, bounds: &Rect) -> PrepareResult {
        let result = self.streaming.prepare_render_with_result(bounds);

        // Check if we should enable GPU rendering
        self.gpu_enabled = self.should_use_gpu();

        // Prepare the appropriate GPU renderer based on chart type
        if self.gpu_enabled {
            self.prepare_gpu_renderer();
        }

        result
    }

    /// Prepare the appropriate GPU renderer based on chart type.
    fn prepare_gpu_renderer(&mut self) {
        let chart = self.streaming.chart();
        match chart.chart_type {
            ChartType::Line => {
                self.line_renderer.prepare(chart);
            }
            ChartType::Scatter => {
                self.scatter_renderer.prepare(chart);
            }
            ChartType::Bar => {
                self.bar_renderer.prepare(chart);
            }
            ChartType::Area => {
                self.area_renderer.prepare(chart);
            }
        }
    }

    /// Render the chart to a render pass.
    ///
    /// This uses GPU-accelerated rendering for large datasets and
    /// falls back to tessellation for small datasets.
    ///
    /// # Arguments
    ///
    /// * `pass` - The render pass to draw into
    /// * `viewport` - The viewport for rendering
    /// * `geometry_renderer` - The geometry renderer for non-GPU elements
    /// * `bounds` - The chart bounds
    pub fn render(
        &self,
        pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        geometry_renderer: &mut crate::GeometryRenderer,
        bounds: &Rect,
    ) {
        use crate::chart::ChartRenderer;

        // Render non-GPU elements via geometry renderer
        let plot_area = bounds.inset(self.streaming.chart().padding);
        let chart = self.streaming.chart();

        {
            let mut chart_renderer = ChartRenderer::new(geometry_renderer);
            if self.gpu_enabled {
                // Draw everything except the data series (which will be GPU rendered)
                chart_renderer.draw_with_gpu_lines(chart, *bounds);
            } else {
                // Draw everything via tessellation
                chart_renderer.draw(chart, *bounds);
            }
            chart_renderer.render(pass, viewport);
        }

        // Render GPU data series if enabled
        if self.gpu_enabled {
            self.render_gpu_series(pass, viewport, &plot_area, chart);
        }
    }

    /// Render the chart with text labels to a render pass.
    ///
    /// This is an enhanced version of `render()` that also draws text elements
    /// when the `chart-text` feature is enabled and a text renderer is configured.
    ///
    /// # Arguments
    ///
    /// * `pass` - The render pass to draw into
    /// * `viewport` - The viewport for rendering
    /// * `geometry_renderer` - The geometry renderer for non-GPU elements
    /// * `bounds` - The chart bounds
    #[cfg(feature = "chart-text")]
    pub fn render_with_text(
        &mut self,
        pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        geometry_renderer: &mut crate::GeometryRenderer,
        bounds: &Rect,
    ) {
        use crate::chart::ChartRenderer;

        let chart = self.streaming.chart();
        let plot_area = bounds.inset(chart.padding);

        // Calculate text margins if text renderer is available
        let text_margins = self
            .text_renderer
            .as_ref()
            .map(|tr| tr.calculate_margins(chart))
            .unwrap_or_default();

        // Adjust plot area for text margins
        let adjusted_plot_area = Rect::new(
            plot_area.x + text_margins.left,
            plot_area.y + text_margins.top,
            (plot_area.width - text_margins.left - text_margins.right).max(1.0),
            (plot_area.height - text_margins.top - text_margins.bottom).max(1.0),
        );

        // Render chart geometry
        {
            let mut chart_renderer = ChartRenderer::new(geometry_renderer);
            if self.gpu_enabled {
                chart_renderer.draw_with_gpu_lines(chart, *bounds);
            } else {
                chart_renderer.draw(chart, *bounds);
            }
            chart_renderer.render(pass, viewport);
        }

        // Render GPU data series if enabled
        if self.gpu_enabled {
            self.render_gpu_series(pass, viewport, &adjusted_plot_area, chart);
        }

        // Render text elements
        if let Some(text_renderer) = &mut self.text_renderer {
            text_renderer.set_viewport(viewport);

            // Draw title
            text_renderer.draw_title(chart, bounds);

            // Draw tick labels
            text_renderer.draw_tick_labels(chart, &adjusted_plot_area);

            // Draw axis labels
            text_renderer.draw_axis_labels(chart, &adjusted_plot_area);

            // Draw legend
            text_renderer.draw_legend(chart, &adjusted_plot_area, geometry_renderer);

            // Render geometry for legend background
            geometry_renderer.render(pass, viewport);

            // Render all text
            text_renderer.render(pass);
        }
    }

    /// Render data series using the appropriate GPU renderer.
    fn render_gpu_series(
        &self,
        pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        plot_area: &Rect,
        chart: &Chart,
    ) {
        match chart.chart_type {
            ChartType::Line => {
                if self.line_renderer.segment_count() > 0 {
                    self.line_renderer.render(pass, viewport, plot_area, chart);
                }
            }
            ChartType::Scatter => {
                if self.scatter_renderer.point_count() > 0 {
                    self.scatter_renderer.render(pass, viewport, plot_area, chart);
                }
            }
            ChartType::Bar => {
                if self.bar_renderer.quad_count() > 0 {
                    self.bar_renderer.render(pass, viewport, plot_area, chart);
                }
            }
            ChartType::Area => {
                if self.area_renderer.quad_count() > 0 || self.area_renderer.segment_count() > 0 {
                    self.area_renderer.render(pass, viewport, plot_area, chart);
                }
            }
        }
    }

    /// Get statistics about the streaming data.
    pub fn statistics(&self) -> GpuStreamingStatistics {
        let base = self.streaming.statistics();
        let chart = self.streaming.chart();

        // Get GPU element count based on chart type
        let gpu_element_count = match chart.chart_type {
            ChartType::Line => self.line_renderer.segment_count(),
            ChartType::Scatter => self.scatter_renderer.point_count(),
            ChartType::Bar => self.bar_renderer.quad_count(),
            ChartType::Area => {
                self.area_renderer.quad_count() + self.area_renderer.segment_count()
            }
        };

        GpuStreamingStatistics {
            total_points: base.total_points,
            series_counts: base.series_counts,
            cache_dirty: base.cache_dirty,
            auto_scroll_active: base.auto_scroll_active,
            gpu_enabled: self.gpu_enabled,
            gpu_segment_count: gpu_element_count,
        }
    }

    /// Get downsampled data for display (for external use like tooltips).
    pub fn get_display_data(&self, series_idx: usize, pixel_width: f32) -> Vec<DataPoint> {
        self.streaming.get_display_data(series_idx, pixel_width)
    }
}

impl std::ops::Deref for GpuStreamingChart {
    type Target = Chart;

    fn deref(&self) -> &Self::Target {
        self.streaming.chart()
    }
}

impl From<(Chart, Arc<GraphicsContext>, wgpu::TextureFormat)> for GpuStreamingChart {
    fn from((chart, context, target_format): (Chart, Arc<GraphicsContext>, wgpu::TextureFormat)) -> Self {
        Self::new(chart, context, target_format)
    }
}

/// Statistics about GPU streaming chart data.
#[derive(Debug, Clone)]
pub struct GpuStreamingStatistics {
    /// Total data points across all series.
    pub total_points: usize,
    /// Data points per series.
    pub series_counts: Vec<usize>,
    /// Whether cache needs rebuild.
    pub cache_dirty: bool,
    /// Whether auto-scroll is active.
    pub auto_scroll_active: bool,
    /// Whether GPU rendering is enabled.
    pub gpu_enabled: bool,
    /// Number of GPU line segments.
    pub gpu_segment_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::ChartBuilder;

    #[test]
    fn test_streaming_chart_append() {
        let chart = ChartBuilder::line()
            .add_series("Test", &[(0.0_f64, 1.0_f64)])
            .build();

        let mut streaming = StreamingChart::new(chart);

        assert_eq!(streaming.chart.series_len(0), 1);

        streaming.append_data(0, &[DataPoint::new(1.0, 2.0), DataPoint::new(2.0, 3.0)]);

        assert_eq!(streaming.chart.series_len(0), 3);
        assert!(streaming.cache.dirty_flags().contains(ChartDirtyFlags::DATA_APPENDED));
    }

    #[test]
    fn test_streaming_chart_sliding_window() {
        let chart = ChartBuilder::line()
            .add_series("Test", &[] as &[(f64, f64)])
            .build();

        let mut streaming = StreamingChart::new(chart);

        // Add points up to the limit
        for i in 0..5 {
            streaming.push_point(0, DataPoint::new(i as f64, i as f64), Some(3));
        }

        // Should only have last 3 points
        assert_eq!(streaming.chart.series_len(0), 3);
        // Sliding window should trigger full data change
        assert!(streaming.cache.dirty_flags().contains(ChartDirtyFlags::DATA_CHANGED));
    }
}
