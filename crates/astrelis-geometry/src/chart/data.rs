//! Data storage backends for chart series.
//!
//! This module provides efficient data storage options:
//! - `Vec<DataPoint>` - Standard vector for static data
//! - `RingBuffer<T>` - Fixed-capacity circular buffer for streaming data
//! - `TimeSeries` - Specialized ring buffer for time-series data with OHLC support

use super::types::DataPoint;

/// Efficient ring buffer for streaming data.
///
/// A fixed-capacity circular buffer that overwrites the oldest elements
/// when full. Provides O(1) push operations and maintains temporal ordering.
///
/// # Example
///
/// ```ignore
/// let mut buffer = RingBuffer::<f64>::new(100);
/// for i in 0..150 {
///     buffer.push(i as f64);
/// }
/// // Buffer now contains 50..150, oldest data was overwritten
/// assert_eq!(buffer.len(), 100);
/// assert_eq!(buffer.get(0), Some(&50.0)); // Oldest remaining
/// ```
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    /// Internal data storage
    data: Vec<T>,
    /// Maximum capacity
    capacity: usize,
    /// Current write position
    write_pos: usize,
    /// Current number of elements (capped at capacity)
    len: usize,
    /// Total number of elements ever written (for tracking)
    total_written: u64,
}

impl<T: Clone + Default> RingBuffer<T> {
    /// Create a new ring buffer with the specified capacity.
    ///
    /// # Panics
    ///
    /// Panics if capacity is 0.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "RingBuffer capacity must be > 0");
        Self {
            data: vec![T::default(); capacity],
            capacity,
            write_pos: 0,
            len: 0,
            total_written: 0,
        }
    }

    /// Create a new ring buffer with initial data.
    ///
    /// If the data exceeds capacity, only the last `capacity` elements are kept.
    pub fn from_vec(mut data: Vec<T>, capacity: usize) -> Self {
        assert!(capacity > 0, "RingBuffer capacity must be > 0");

        let total_written = data.len() as u64;

        if data.len() > capacity {
            // Keep only the last `capacity` elements
            let excess = data.len() - capacity;
            data.drain(..excess);
        }

        let len = data.len();
        let write_pos = len % capacity;

        // Pad to capacity
        data.resize_with(capacity, T::default);

        Self {
            data,
            capacity,
            write_pos,
            len,
            total_written,
        }
    }

    /// Push a single item to the buffer.
    ///
    /// O(1) operation. If the buffer is full, the oldest element is overwritten.
    #[inline]
    pub fn push(&mut self, item: T) {
        self.data[self.write_pos] = item;
        self.write_pos = (self.write_pos + 1) % self.capacity;
        self.len = (self.len + 1).min(self.capacity);
        self.total_written = self.total_written.wrapping_add(1);
    }

    /// Extend the buffer with multiple items.
    ///
    /// More efficient than calling `push` repeatedly.
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, items: I) {
        for item in items {
            self.push(item);
        }
    }

    /// Get an item by logical index (0 = oldest item).
    ///
    /// Returns `None` if the index is out of bounds.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }

        // Calculate actual index in the circular buffer
        let start = if self.len < self.capacity {
            0
        } else {
            self.write_pos
        };
        let actual_idx = (start + index) % self.capacity;
        Some(&self.data[actual_idx])
    }

    /// Get a mutable reference to an item by logical index.
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }

        let start = if self.len < self.capacity {
            0
        } else {
            self.write_pos
        };
        let actual_idx = (start + index) % self.capacity;
        Some(&mut self.data[actual_idx])
    }

    /// Get the most recent item (newest).
    #[inline]
    pub fn last(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Get the oldest item.
    #[inline]
    pub fn first(&self) -> Option<&T> {
        if self.len == 0 { None } else { self.get(0) }
    }

    /// Get the current number of elements.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the maximum capacity.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Check if the buffer has wrapped (oldest data was overwritten).
    #[inline]
    pub fn has_wrapped(&self) -> bool {
        self.total_written > self.capacity as u64
    }

    /// Get the total number of items ever written.
    #[inline]
    pub fn total_written(&self) -> u64 {
        self.total_written
    }

    /// Check if the buffer is full.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len == self.capacity
    }

    /// Clear all elements from the buffer.
    pub fn clear(&mut self) {
        self.len = 0;
        self.write_pos = 0;
        // Don't reset total_written - useful for tracking
    }

    /// Iterate over elements in order (oldest to newest).
    pub fn iter(&self) -> RingBufferIter<'_, T> {
        RingBufferIter {
            buffer: self,
            index: 0,
        }
    }

    /// Get the data as one or two contiguous slices.
    ///
    /// This is useful for efficient GPU uploads. If the buffer hasn't wrapped,
    /// returns a single slice. If wrapped, returns two slices that together
    /// contain all data in order (oldest to newest).
    pub fn as_slices(&self) -> (&[T], Option<&[T]>) {
        if self.len == 0 {
            return (&[], None);
        }

        if self.len < self.capacity || self.write_pos == 0 {
            // Haven't wrapped yet, or write position is at start
            (&self.data[..self.len], None)
        } else {
            // Wrapped: data from write_pos to end, then from 0 to write_pos
            let first = &self.data[self.write_pos..];
            let second = &self.data[..self.write_pos];
            (first, Some(second))
        }
    }

    /// Get a contiguous slice containing all data (may allocate).
    ///
    /// If the buffer hasn't wrapped, this returns a reference to the internal slice.
    /// Otherwise, it allocates and returns a new Vec.
    pub fn to_vec(&self) -> Vec<T> {
        let (first, second) = self.as_slices();
        if let Some(second) = second {
            let mut result = Vec::with_capacity(self.len);
            result.extend_from_slice(first);
            result.extend_from_slice(second);
            result
        } else {
            first.to_vec()
        }
    }
}

impl<T> Default for RingBuffer<T>
where
    T: Clone + Default,
{
    fn default() -> Self {
        Self::new(1024)
    }
}

/// Iterator over ring buffer elements.
pub struct RingBufferIter<'a, T> {
    buffer: &'a RingBuffer<T>,
    index: usize,
}

impl<'a, T: Clone + Default> Iterator for RingBufferIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.buffer.len {
            None
        } else {
            let item = self.buffer.get(self.index);
            self.index += 1;
            item
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.buffer.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<T: Clone + Default> ExactSizeIterator for RingBufferIter<'_, T> {}

/// Time series point with optional OHLC (Open-High-Low-Close) data.
///
/// Used for downsampled or aggregated time series data where multiple
/// samples are combined into a single point.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TimeSeriesPoint {
    /// Timestamp (typically in seconds or milliseconds)
    pub time: f64,
    /// Primary value (close price, average, etc.)
    pub value: f64,
    /// Optional minimum value in the aggregation period
    pub min: Option<f64>,
    /// Optional maximum value in the aggregation period
    pub max: Option<f64>,
    /// Optional count of samples aggregated into this point
    pub count: Option<u32>,
}

impl TimeSeriesPoint {
    /// Create a simple time series point.
    pub fn new(time: f64, value: f64) -> Self {
        Self {
            time,
            value,
            min: None,
            max: None,
            count: None,
        }
    }

    /// Create a point with min/max range.
    pub fn with_range(time: f64, value: f64, min: f64, max: f64) -> Self {
        Self {
            time,
            value,
            min: Some(min),
            max: Some(max),
            count: None,
        }
    }

    /// Create a fully aggregated point.
    pub fn aggregated(time: f64, value: f64, min: f64, max: f64, count: u32) -> Self {
        Self {
            time,
            value,
            min: Some(min),
            max: Some(max),
            count: Some(count),
        }
    }

    /// Convert to a simple DataPoint (ignores OHLC data).
    pub fn to_data_point(&self) -> DataPoint {
        DataPoint::new(self.time, self.value)
    }
}

impl From<TimeSeriesPoint> for DataPoint {
    fn from(point: TimeSeriesPoint) -> Self {
        DataPoint::new(point.time, point.value)
    }
}

impl From<(f64, f64)> for TimeSeriesPoint {
    fn from((time, value): (f64, f64)) -> Self {
        Self::new(time, value)
    }
}

/// Data storage backend for chart series.
///
/// Provides multiple storage options optimized for different use cases:
/// - `Vec` - Standard vector for static or infrequently updated data
/// - `Ring` - Ring buffer for high-frequency streaming data
/// - `TimeSeries` - Ring buffer with OHLC support for time series
/// - `Downsampled` - Downsampled view of another data source
#[derive(Debug, Clone)]
pub enum SeriesData {
    /// Standard vector storage.
    Vec(Vec<DataPoint>),
    /// Ring buffer for streaming data.
    Ring(RingBuffer<DataPoint>),
    /// Time series with OHLC support.
    TimeSeries(RingBuffer<TimeSeriesPoint>),
    /// Downsampled view of source data.
    Downsampled {
        /// Source data (boxed to prevent infinite size)
        source: Box<SeriesData>,
        /// Downsampling factor
        factor: usize,
        /// Cached downsampled data
        cache: Vec<DataPoint>,
        /// Version of source data when cache was built
        source_version: u64,
    },
}

impl Default for SeriesData {
    fn default() -> Self {
        Self::Vec(Vec::new())
    }
}

impl SeriesData {
    /// Create from a vector of data points.
    pub fn from_vec(data: Vec<DataPoint>) -> Self {
        Self::Vec(data)
    }

    /// Create a ring buffer with the specified capacity.
    pub fn ring(capacity: usize) -> Self {
        Self::Ring(RingBuffer::new(capacity))
    }

    /// Create a time series ring buffer with the specified capacity.
    pub fn time_series(capacity: usize) -> Self {
        Self::TimeSeries(RingBuffer::new(capacity))
    }

    /// Get the number of data points.
    pub fn len(&self) -> usize {
        match self {
            Self::Vec(v) => v.len(),
            Self::Ring(r) => r.len(),
            Self::TimeSeries(r) => r.len(),
            Self::Downsampled { cache, .. } => cache.len(),
        }
    }

    /// Check if the data is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a data point by index.
    pub fn get(&self, index: usize) -> Option<DataPoint> {
        match self {
            Self::Vec(v) => v.get(index).copied(),
            Self::Ring(r) => r.get(index).copied(),
            Self::TimeSeries(r) => r.get(index).map(|p| p.to_data_point()),
            Self::Downsampled { cache, .. } => cache.get(index).copied(),
        }
    }

    /// Get the first (oldest) data point.
    pub fn first(&self) -> Option<DataPoint> {
        self.get(0)
    }

    /// Get the last (newest) data point.
    pub fn last(&self) -> Option<DataPoint> {
        if self.is_empty() {
            None
        } else {
            self.get(self.len() - 1)
        }
    }

    /// Push a data point (only works for Vec and Ring variants).
    pub fn push(&mut self, point: DataPoint) {
        match self {
            Self::Vec(v) => v.push(point),
            Self::Ring(r) => r.push(point),
            Self::TimeSeries(r) => r.push(TimeSeriesPoint::new(point.x, point.y)),
            Self::Downsampled { .. } => {
                // Cannot push to downsampled - push to source instead
            }
        }
    }

    /// Push a time series point (only for TimeSeries variant).
    pub fn push_time_point(&mut self, point: TimeSeriesPoint) {
        if let Self::TimeSeries(r) = self {
            r.push(point);
        }
    }

    /// Clear all data.
    pub fn clear(&mut self) {
        match self {
            Self::Vec(v) => v.clear(),
            Self::Ring(r) => r.clear(),
            Self::TimeSeries(r) => r.clear(),
            Self::Downsampled { cache, .. } => cache.clear(),
        }
    }

    /// Iterate over data points.
    pub fn iter(&self) -> SeriesDataIter<'_> {
        SeriesDataIter {
            data: self,
            index: 0,
            len: self.len(),
        }
    }

    /// Get the X range (min, max) of the data.
    pub fn x_range(&self) -> Option<(f64, f64)> {
        if self.is_empty() {
            return None;
        }

        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;

        for point in self.iter() {
            min = min.min(point.x);
            max = max.max(point.x);
        }

        Some((min, max))
    }

    /// Get the Y range (min, max) of the data.
    pub fn y_range(&self) -> Option<(f64, f64)> {
        if self.is_empty() {
            return None;
        }

        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;

        for point in self.iter() {
            min = min.min(point.y);
            max = max.max(point.y);
        }

        Some((min, max))
    }

    /// Get the bounds (min_x, min_y, max_x, max_y) of the data.
    pub fn bounds(&self) -> Option<(DataPoint, DataPoint)> {
        if self.is_empty() {
            return None;
        }

        let mut min = DataPoint::new(f64::INFINITY, f64::INFINITY);
        let mut max = DataPoint::new(f64::NEG_INFINITY, f64::NEG_INFINITY);

        for point in self.iter() {
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
        }

        Some((min, max))
    }

    /// Check if this is a ring buffer variant.
    pub fn is_ring(&self) -> bool {
        matches!(self, Self::Ring(_) | Self::TimeSeries(_))
    }

    /// Check if the ring buffer has wrapped (data was overwritten).
    pub fn has_wrapped(&self) -> bool {
        match self {
            Self::Ring(r) => r.has_wrapped(),
            Self::TimeSeries(r) => r.has_wrapped(),
            _ => false,
        }
    }

    /// Get the capacity (for ring buffers) or current length (for Vec).
    pub fn capacity(&self) -> usize {
        match self {
            Self::Vec(v) => v.capacity(),
            Self::Ring(r) => r.capacity(),
            Self::TimeSeries(r) => r.capacity(),
            Self::Downsampled { source, .. } => source.capacity(),
        }
    }

    /// Convert to a Vec of DataPoints.
    pub fn to_vec(&self) -> Vec<DataPoint> {
        self.iter().collect()
    }
}

/// Iterator over SeriesData.
pub struct SeriesDataIter<'a> {
    data: &'a SeriesData,
    index: usize,
    len: usize,
}

impl Iterator for SeriesDataIter<'_> {
    type Item = DataPoint;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            None
        } else {
            let item = self.data.get(self.index);
            self.index += 1;
            item
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for SeriesDataIter<'_> {}

/// LTTB (Largest Triangle Three Buckets) downsampling algorithm.
///
/// Preserves visual features while reducing data density for rendering.
/// Returns indices of points to keep.
pub fn lttb_downsample(data: &[DataPoint], target_count: usize) -> Vec<usize> {
    let n = data.len();

    if n <= target_count {
        return (0..n).collect();
    }

    if target_count < 3 {
        if target_count == 0 {
            return vec![];
        } else if target_count == 1 {
            return vec![0];
        } else {
            return vec![0, n - 1];
        }
    }

    let mut result = Vec::with_capacity(target_count);

    // Always include first point
    result.push(0);

    // Bucket size (excluding first and last points)
    let bucket_size = (n - 2) as f64 / (target_count - 2) as f64;

    let mut a = 0usize; // Previous selected point

    for i in 0..(target_count - 2) {
        // Calculate bucket boundaries
        let bucket_start = ((i as f64 * bucket_size) as usize + 1).min(n - 1);
        let bucket_end = (((i + 1) as f64 * bucket_size) as usize + 1).min(n - 1);

        // Calculate average point in next bucket (for triangle area calculation)
        let next_bucket_start = bucket_end;
        let next_bucket_end = (((i + 2) as f64 * bucket_size) as usize + 1).min(n);

        let mut avg_x = 0.0;
        let mut avg_y = 0.0;
        let count = next_bucket_end - next_bucket_start;

        if count > 0 {
            for j in next_bucket_start..next_bucket_end {
                avg_x += data[j].x;
                avg_y += data[j].y;
            }
            avg_x /= count as f64;
            avg_y /= count as f64;
        }

        // Find point in current bucket that creates largest triangle
        let mut max_area = 0.0;
        let mut max_area_idx = bucket_start;

        let point_a = &data[a];

        for j in bucket_start..bucket_end {
            let point = &data[j];

            // Calculate triangle area using cross product
            let area = ((point_a.x - avg_x) * (point.y - point_a.y)
                - (point_a.x - point.x) * (avg_y - point_a.y))
                .abs();

            if area > max_area {
                max_area = area;
                max_area_idx = j;
            }
        }

        result.push(max_area_idx);
        a = max_area_idx;
    }

    // Always include last point
    result.push(n - 1);

    result
}

/// Downsample data using LTTB algorithm.
pub fn downsample_data(data: &[DataPoint], target_count: usize) -> Vec<DataPoint> {
    let indices = lttb_downsample(data, target_count);
    indices.into_iter().map(|i| data[i]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let mut buffer = RingBuffer::<i32>::new(5);

        // Push fewer than capacity
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        assert_eq!(buffer.len(), 3);
        assert!(!buffer.has_wrapped());
        assert_eq!(buffer.get(0), Some(&1));
        assert_eq!(buffer.get(2), Some(&3));
    }

    #[test]
    fn test_ring_buffer_wrap() {
        let mut buffer = RingBuffer::<i32>::new(3);

        // Push more than capacity
        for i in 0..5 {
            buffer.push(i);
        }

        assert_eq!(buffer.len(), 3);
        assert!(buffer.has_wrapped());
        assert_eq!(buffer.get(0), Some(&2)); // Oldest remaining
        assert_eq!(buffer.get(1), Some(&3));
        assert_eq!(buffer.get(2), Some(&4)); // Newest
    }

    #[test]
    fn test_ring_buffer_iter() {
        let mut buffer = RingBuffer::<i32>::new(4);

        for i in 0..6 {
            buffer.push(i);
        }

        let items: Vec<_> = buffer.iter().copied().collect();
        assert_eq!(items, vec![2, 3, 4, 5]);
    }

    #[test]
    fn test_ring_buffer_slices() {
        let mut buffer = RingBuffer::<i32>::new(4);

        // Before wrap
        buffer.push(1);
        buffer.push(2);
        let (first, second) = buffer.as_slices();
        assert_eq!(first, &[1, 2]);
        assert!(second.is_none());

        // After wrap
        buffer.push(3);
        buffer.push(4);
        buffer.push(5);
        buffer.push(6);
        let (first, second) = buffer.as_slices();
        // Should contain 3, 4, 5, 6 in order
        assert_eq!(first.len() + second.map_or(0, |s| s.len()), 4);
    }

    #[test]
    fn test_series_data_vec() {
        let mut data =
            SeriesData::from_vec(vec![DataPoint::new(0.0, 1.0), DataPoint::new(1.0, 2.0)]);

        assert_eq!(data.len(), 2);
        data.push(DataPoint::new(2.0, 3.0));
        assert_eq!(data.len(), 3);
        assert_eq!(data.get(2), Some(DataPoint::new(2.0, 3.0)));
    }

    #[test]
    fn test_series_data_ring() {
        let mut data = SeriesData::ring(3);

        for i in 0..5 {
            data.push(DataPoint::new(i as f64, i as f64 * 2.0));
        }

        assert_eq!(data.len(), 3);
        assert!(data.has_wrapped());
        assert_eq!(data.first(), Some(DataPoint::new(2.0, 4.0)));
    }

    #[test]
    fn test_lttb_downsample() {
        // Create a simple dataset
        let data: Vec<DataPoint> = (0..100)
            .map(|i| DataPoint::new(i as f64, (i as f64 * 0.1).sin()))
            .collect();

        let downsampled = downsample_data(&data, 20);

        assert_eq!(downsampled.len(), 20);
        // First and last points should be preserved
        assert_eq!(downsampled[0], data[0]);
        assert_eq!(downsampled[19], data[99]);
    }
}
