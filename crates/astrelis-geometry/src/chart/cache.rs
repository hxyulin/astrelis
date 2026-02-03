//! Chart caching for improved rendering performance.
//!
//! This module provides coordinate caching and spatial indexing to optimize
//! chart rendering, especially for charts with large data sets.

use super::rect::Rect;
use super::types::{AxisId, Chart, DataPoint};
use glam::Vec2;

bitflags::bitflags! {
    /// Dirty flags for tracking what needs to be updated in a chart.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ChartDirtyFlags: u16 {
        /// Data has changed completely (requires full rebuild).
        const DATA_CHANGED = 0b0000_0001;
        /// Data was appended (can use partial update).
        const DATA_APPENDED = 0b0000_0010;
        /// View changed (pan/zoom) - need to recalculate pixel coordinates.
        const VIEW_CHANGED = 0b0000_0100;
        /// Style changed (colors, line width, etc.).
        const STYLE_CHANGED = 0b0000_1000;
        /// Axes changed (range, ticks, labels).
        const AXES_CHANGED = 0b0001_0000;
        /// Bounds changed (widget resized).
        const BOUNDS_CHANGED = 0b0010_0000;
    }
}

impl ChartDirtyFlags {
    /// Check if the cache needs to be rebuilt.
    pub fn needs_cache_rebuild(&self) -> bool {
        self.intersects(
            Self::DATA_CHANGED
                | Self::VIEW_CHANGED
                | Self::AXES_CHANGED
                | Self::BOUNDS_CHANGED,
        )
    }

    /// Check if only data was appended (can use partial update).
    pub fn is_append_only(&self) -> bool {
        self.contains(Self::DATA_APPENDED) && !self.contains(Self::DATA_CHANGED)
    }

    /// Check if only style changed (no geometry update needed).
    pub fn is_style_only(&self) -> bool {
        *self == Self::STYLE_CHANGED
    }
}

/// Cached pixel coordinates for a single data series.
#[derive(Debug, Clone)]
pub struct SeriesPixelCache {
    /// Pixel positions for each data point.
    pub positions: Vec<Vec2>,
    /// X axis ID used for this cache.
    pub x_axis: AxisId,
    /// Y axis ID used for this cache.
    pub y_axis: AxisId,
    /// Data range that was used to compute these positions.
    pub x_range: (f64, f64),
    /// Y data range that was used to compute these positions.
    pub y_range: (f64, f64),
    /// Number of data points when cache was built.
    pub data_count: usize,
}

impl SeriesPixelCache {
    /// Create an empty cache.
    pub fn new(x_axis: AxisId, y_axis: AxisId) -> Self {
        Self {
            positions: Vec::new(),
            x_axis,
            y_axis,
            x_range: (0.0, 1.0),
            y_range: (0.0, 1.0),
            data_count: 0,
        }
    }

    /// Check if the cache is valid for the given parameters.
    pub fn is_valid(
        &self,
        x_range: (f64, f64),
        y_range: (f64, f64),
        data_count: usize,
    ) -> bool {
        self.x_range == x_range && self.y_range == y_range && self.data_count == data_count
    }
}

/// Chart coordinate and geometry cache.
///
/// This cache stores pre-computed pixel coordinates and spatial index
/// for fast rendering and hit testing.
#[derive(Debug, Clone)]
pub struct ChartCache {
    /// Per-series pixel coordinate caches.
    series_caches: Vec<SeriesPixelCache>,
    /// Spatial index for fast hit testing.
    spatial_index: Option<SpatialIndex>,
    /// Data version counter (increments when data changes).
    data_version: u64,
    /// View version counter (increments when view changes).
    view_version: u64,
    /// Bounds used for last cache build.
    last_bounds: Option<Rect>,
    /// Dirty flags tracking what needs updating.
    dirty_flags: ChartDirtyFlags,
}

impl Default for ChartCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ChartCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            series_caches: Vec::new(),
            spatial_index: None,
            data_version: 0,
            view_version: 0,
            last_bounds: None,
            dirty_flags: ChartDirtyFlags::all(),
        }
    }

    /// Get the dirty flags.
    pub fn dirty_flags(&self) -> ChartDirtyFlags {
        self.dirty_flags
    }

    /// Mark the cache as needing a full rebuild.
    pub fn invalidate(&mut self) {
        self.dirty_flags = ChartDirtyFlags::all();
    }

    /// Mark data as changed.
    pub fn mark_data_changed(&mut self) {
        self.dirty_flags.insert(ChartDirtyFlags::DATA_CHANGED);
        self.data_version = self.data_version.wrapping_add(1);
    }

    /// Mark data as appended (partial update possible).
    pub fn mark_data_appended(&mut self, series_idx: usize, new_count: usize) {
        if series_idx < self.series_caches.len() {
            // Only mark as appended if current cache has fewer points
            let cache = &self.series_caches[series_idx];
            if cache.data_count < new_count {
                self.dirty_flags.insert(ChartDirtyFlags::DATA_APPENDED);
            }
        } else {
            // Series doesn't exist in cache yet, mark as full change
            self.dirty_flags.insert(ChartDirtyFlags::DATA_CHANGED);
        }
    }

    /// Mark view as changed (pan/zoom).
    pub fn mark_view_changed(&mut self) {
        self.dirty_flags.insert(ChartDirtyFlags::VIEW_CHANGED);
        self.view_version = self.view_version.wrapping_add(1);
    }

    /// Mark style as changed.
    pub fn mark_style_changed(&mut self) {
        self.dirty_flags.insert(ChartDirtyFlags::STYLE_CHANGED);
    }

    /// Mark axes as changed.
    pub fn mark_axes_changed(&mut self) {
        self.dirty_flags.insert(ChartDirtyFlags::AXES_CHANGED);
    }

    /// Mark bounds as changed.
    pub fn mark_bounds_changed(&mut self) {
        self.dirty_flags.insert(ChartDirtyFlags::BOUNDS_CHANGED);
    }

    /// Clear dirty flags after processing.
    pub fn clear_dirty(&mut self) {
        self.dirty_flags = ChartDirtyFlags::empty();
    }

    /// Check if cache needs to be rebuilt.
    pub fn needs_rebuild(&self) -> bool {
        self.dirty_flags.needs_cache_rebuild()
    }

    /// Rebuild the cache for a chart.
    pub fn rebuild(&mut self, chart: &Chart, bounds: &Rect) {
        let plot_area = bounds.inset(chart.padding);

        // Check if we can do a partial update
        if self.dirty_flags.is_append_only() && self.last_bounds == Some(*bounds) {
            self.partial_update(chart, &plot_area);
        } else {
            self.full_rebuild(chart, &plot_area);
        }

        self.last_bounds = Some(*bounds);
        self.clear_dirty();
    }

    /// Perform a full rebuild of the cache.
    fn full_rebuild(&mut self, chart: &Chart, plot_area: &Rect) {
        // Resize series caches to match chart
        self.series_caches.resize_with(chart.series.len(), || {
            SeriesPixelCache::new(AxisId::X_PRIMARY, AxisId::Y_PRIMARY)
        });

        // Rebuild each series cache
        for (series_idx, series) in chart.series.iter().enumerate() {
            let x_range = chart.axis_range(series.x_axis);
            let y_range = chart.axis_range(series.y_axis);

            let cache = &mut self.series_caches[series_idx];
            cache.x_axis = series.x_axis;
            cache.y_axis = series.y_axis;
            cache.x_range = x_range;
            cache.y_range = y_range;
            cache.data_count = series.data.len();

            // Compute pixel positions
            cache.positions.clear();
            cache.positions.reserve(series.data.len());

            for point in &series.data {
                let pixel = data_to_pixel(point, plot_area, x_range, y_range);
                cache.positions.push(pixel);
            }
        }

        // Rebuild spatial index
        self.rebuild_spatial_index(plot_area);
    }

    /// Perform a partial update (append only).
    fn partial_update(&mut self, chart: &Chart, plot_area: &Rect) {
        for (series_idx, series) in chart.series.iter().enumerate() {
            if series_idx >= self.series_caches.len() {
                // New series, need full rebuild for this one
                self.series_caches.push(SeriesPixelCache::new(
                    series.x_axis,
                    series.y_axis,
                ));
            }

            let cache = &mut self.series_caches[series_idx];
            let x_range = chart.axis_range(series.x_axis);
            let y_range = chart.axis_range(series.y_axis);

            // Check if ranges changed (would need full rebuild)
            if cache.x_range != x_range || cache.y_range != y_range {
                // Ranges changed, rebuild this series
                cache.x_range = x_range;
                cache.y_range = y_range;
                cache.positions.clear();
                cache.positions.reserve(series.data.len());
                for point in &series.data {
                    let pixel = data_to_pixel(point, plot_area, x_range, y_range);
                    cache.positions.push(pixel);
                }
            } else if series.data.len() > cache.data_count {
                // Append new points
                cache.positions.reserve(series.data.len() - cache.data_count);
                for point in &series.data[cache.data_count..] {
                    let pixel = data_to_pixel(point, plot_area, x_range, y_range);
                    cache.positions.push(pixel);
                }
            }

            cache.data_count = series.data.len();
        }

        // Rebuild spatial index with new data
        self.rebuild_spatial_index(plot_area);
    }

    /// Rebuild the spatial index from cached positions.
    fn rebuild_spatial_index(&mut self, plot_area: &Rect) {
        let mut index = SpatialIndex::new(*plot_area, 32, 32);

        for (series_idx, cache) in self.series_caches.iter().enumerate() {
            for (point_idx, &pos) in cache.positions.iter().enumerate() {
                index.insert(pos, series_idx, point_idx);
            }
        }

        self.spatial_index = Some(index);
    }

    /// Get cached pixel positions for a series.
    pub fn series_positions(&self, series_idx: usize) -> Option<&[Vec2]> {
        self.series_caches
            .get(series_idx)
            .map(|c| c.positions.as_slice())
    }

    /// Get the spatial index for hit testing.
    pub fn spatial_index(&self) -> Option<&SpatialIndex> {
        self.spatial_index.as_ref()
    }

    /// Perform a fast hit test using the spatial index.
    pub fn hit_test(
        &self,
        chart: &Chart,
        pixel: Vec2,
        max_distance: f32,
    ) -> Option<CacheHitResult> {
        let index = self.spatial_index.as_ref()?;

        let mut best: Option<CacheHitResult> = None;

        for (series_idx, point_idx) in index.query_near(pixel, max_distance) {
            if let Some(cache) = self.series_caches.get(series_idx)
                && let Some(&point_pixel) = cache.positions.get(point_idx) {
                    let dist = pixel.distance(point_pixel);
                    if dist <= max_distance
                        && best.as_ref().is_none_or(|b| dist < b.distance) {
                            let data_point = chart
                                .series
                                .get(series_idx)
                                .and_then(|s| s.data.get(point_idx))
                                .copied();

                            if let Some(data_point) = data_point {
                                best = Some(CacheHitResult {
                                    series_index: series_idx,
                                    point_index: point_idx,
                                    distance: dist,
                                    data_point,
                                    pixel_position: point_pixel,
                                });
                            }
                        }
                }
        }

        best
    }
}

/// Result of a hit test using the cache.
#[derive(Debug, Clone)]
pub struct CacheHitResult {
    /// Series index.
    pub series_index: usize,
    /// Point index within the series.
    pub point_index: usize,
    /// Distance from the test point to the data point (in pixels).
    pub distance: f32,
    /// The data point.
    pub data_point: DataPoint,
    /// The pixel position of the data point.
    pub pixel_position: Vec2,
}

/// Spatial index for O(1) hit testing.
///
/// Uses a uniform grid to partition the chart area, allowing fast
/// lookup of points near a given position.
#[derive(Debug, Clone)]
pub struct SpatialIndex {
    /// Grid cells, each containing a list of (series_idx, point_idx).
    cells: Vec<Vec<(usize, usize)>>,
    /// Number of columns in the grid.
    cols: usize,
    /// Number of rows in the grid.
    rows: usize,
    /// Bounds of the indexed area.
    bounds: Rect,
    /// Width of each cell.
    cell_width: f32,
    /// Height of each cell.
    cell_height: f32,
}

impl SpatialIndex {
    /// Create a new spatial index for the given bounds.
    pub fn new(bounds: Rect, cols: usize, rows: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);

        Self {
            cells: vec![Vec::new(); cols * rows],
            cols,
            rows,
            bounds,
            cell_width: bounds.width / cols as f32,
            cell_height: bounds.height / rows as f32,
        }
    }

    /// Insert a point into the index.
    pub fn insert(&mut self, pos: Vec2, series_idx: usize, point_idx: usize) {
        if let Some(cell_idx) = self.cell_index(pos) {
            self.cells[cell_idx].push((series_idx, point_idx));
        }
    }

    /// Clear all entries from the index.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.clear();
        }
    }

    /// Get the cell index for a position.
    fn cell_index(&self, pos: Vec2) -> Option<usize> {
        if !self.bounds.contains(pos) {
            return None;
        }

        let col = ((pos.x - self.bounds.x) / self.cell_width) as usize;
        let row = ((pos.y - self.bounds.y) / self.cell_height) as usize;

        let col = col.min(self.cols - 1);
        let row = row.min(self.rows - 1);

        Some(row * self.cols + col)
    }

    /// Get the cell coordinates for a position.
    fn cell_coords(&self, pos: Vec2) -> Option<(usize, usize)> {
        if !self.bounds.contains(pos) {
            // Clamp to bounds for edge cases
            let x = pos.x.clamp(self.bounds.x, self.bounds.right());
            let y = pos.y.clamp(self.bounds.y, self.bounds.bottom());

            let col = ((x - self.bounds.x) / self.cell_width) as usize;
            let row = ((y - self.bounds.y) / self.cell_height) as usize;

            return Some((col.min(self.cols - 1), row.min(self.rows - 1)));
        }

        let col = ((pos.x - self.bounds.x) / self.cell_width) as usize;
        let row = ((pos.y - self.bounds.y) / self.cell_height) as usize;

        Some((col.min(self.cols - 1), row.min(self.rows - 1)))
    }

    /// Query points near a position within a radius.
    ///
    /// Returns an iterator over (series_idx, point_idx) pairs.
    pub fn query_near(&self, pos: Vec2, radius: f32) -> impl Iterator<Item = (usize, usize)> + '_ {
        // Calculate cell range to check
        let (center_col, center_row) = self.cell_coords(pos).unwrap_or((0, 0));

        let cells_x = (radius / self.cell_width).ceil() as usize + 1;
        let cells_y = (radius / self.cell_height).ceil() as usize + 1;

        let min_col = center_col.saturating_sub(cells_x);
        let max_col = (center_col + cells_x).min(self.cols - 1);
        let min_row = center_row.saturating_sub(cells_y);
        let max_row = (center_row + cells_y).min(self.rows - 1);

        // Collect all candidates from nearby cells
        SpatialQueryIter {
            index: self,
            min_col,
            max_col,
            min_row,
            max_row,
            current_col: min_col,
            current_row: min_row,
            current_entry: 0,
        }
    }

    /// Get the number of points in the index.
    pub fn len(&self) -> usize {
        self.cells.iter().map(|c| c.len()).sum()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(|c| c.is_empty())
    }
}

/// Iterator for spatial queries.
struct SpatialQueryIter<'a> {
    index: &'a SpatialIndex,
    min_col: usize,
    max_col: usize,
    #[allow(dead_code)] // Used to initialize current_row
    min_row: usize,
    max_row: usize,
    current_col: usize,
    current_row: usize,
    current_entry: usize,
}

impl Iterator for SpatialQueryIter<'_> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current_row > self.max_row {
                return None;
            }

            let cell_idx = self.current_row * self.index.cols + self.current_col;
            let cell = &self.index.cells[cell_idx];

            if self.current_entry < cell.len() {
                let result = cell[self.current_entry];
                self.current_entry += 1;
                return Some(result);
            }

            // Move to next cell
            self.current_entry = 0;
            self.current_col += 1;

            if self.current_col > self.max_col {
                self.current_col = self.min_col;
                self.current_row += 1;
            }
        }
    }
}

/// Convert a data point to pixel coordinates.
fn data_to_pixel(
    point: &DataPoint,
    plot_area: &Rect,
    x_range: (f64, f64),
    y_range: (f64, f64),
) -> Vec2 {
    let (x_min, x_max) = x_range;
    let (y_min, y_max) = y_range;

    let px = plot_area.x + ((point.x - x_min) / (x_max - x_min)) as f32 * plot_area.width;
    // Y is inverted (0 at top in screen coords)
    let py = plot_area.y + plot_area.height
        - ((point.y - y_min) / (y_max - y_min)) as f32 * plot_area.height;

    Vec2::new(px, py)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirty_flags() {
        let mut flags = ChartDirtyFlags::empty();
        assert!(!flags.needs_cache_rebuild());

        flags.insert(ChartDirtyFlags::STYLE_CHANGED);
        assert!(!flags.needs_cache_rebuild());
        assert!(flags.is_style_only());

        flags.insert(ChartDirtyFlags::DATA_CHANGED);
        assert!(flags.needs_cache_rebuild());
        assert!(!flags.is_style_only());
    }

    #[test]
    fn test_spatial_index() {
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut index = SpatialIndex::new(bounds, 10, 10);

        // Insert some points
        index.insert(Vec2::new(15.0, 15.0), 0, 0);
        index.insert(Vec2::new(25.0, 25.0), 0, 1);
        index.insert(Vec2::new(85.0, 85.0), 1, 0);

        assert_eq!(index.len(), 3);

        // Query near first point
        let results: Vec<_> = index.query_near(Vec2::new(15.0, 15.0), 20.0).collect();
        assert!(results.contains(&(0, 0)));
        assert!(results.contains(&(0, 1)));
        assert!(!results.contains(&(1, 0)));
    }
}
