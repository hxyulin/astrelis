//! Efficient dirty range tracking for partial buffer updates.
//!
//! This module implements Phase 5 infrastructure for tracking which ranges
//! of instance buffers need GPU updates. Uses sorted, non-overlapping ranges
//! with automatic merging for optimal upload efficiency.

use std::ops::Range;

/// Tracks dirty ranges in a buffer for partial updates.
///
/// Maintains a sorted list of non-overlapping ranges that need updating.
/// Automatically merges adjacent and overlapping ranges for efficiency.
#[derive(Debug, Clone)]
pub struct DirtyRanges {
    /// Sorted, non-overlapping ranges (start, end) - end is exclusive
    ranges: Vec<Range<usize>>,
}

impl DirtyRanges {
    /// Create a new empty dirty range tracker.
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    /// Mark a range as dirty.
    ///
    /// The range will be merged with existing ranges if they overlap or are adjacent.
    pub fn mark_dirty(&mut self, start: usize, end: usize) {
        if start >= end {
            return; // Invalid range
        }

        let new_range = start..end;

        // Find insertion point and ranges to merge
        let mut insert_idx = self.ranges.len();
        let mut merge_start_idx = None;
        let mut merge_end_idx = None;

        for (i, range) in self.ranges.iter().enumerate() {
            // Check if ranges overlap or are adjacent
            if ranges_overlap_or_adjacent(&new_range, range) {
                if merge_start_idx.is_none() {
                    merge_start_idx = Some(i);
                }
                merge_end_idx = Some(i);
            } else if range.start > end {
                // Found first range beyond our new range
                if merge_start_idx.is_none() {
                    insert_idx = i;
                }
                break;
            }
        }

        match (merge_start_idx, merge_end_idx) {
            (Some(start_idx), Some(end_idx)) => {
                // Merge with existing ranges
                let merged_start = self.ranges[start_idx].start.min(start);
                let merged_end = self.ranges[end_idx].end.max(end);

                // Remove merged ranges
                self.ranges.drain(start_idx..=end_idx);

                // Insert merged range
                self.ranges.insert(start_idx, merged_start..merged_end);
            }
            (None, None) => {
                // No overlap, insert at correct position
                self.ranges.insert(insert_idx, new_range);
            }
            _ => unreachable!("merge_start_idx and merge_end_idx should both be Some or None"),
        }
    }

    /// Mark multiple ranges as dirty at once.
    pub fn mark_dirty_batch(&mut self, ranges: impl IntoIterator<Item = Range<usize>>) {
        for range in ranges {
            self.mark_dirty(range.start, range.end);
        }
    }

    /// Clear all dirty ranges.
    pub fn clear(&mut self) {
        self.ranges.clear();
    }

    /// Check if any ranges are dirty.
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Get the number of dirty ranges.
    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    /// Iterate over dirty ranges.
    pub fn iter(&self) -> impl Iterator<Item = &Range<usize>> {
        self.ranges.iter()
    }

    /// Get total number of dirty elements across all ranges.
    pub fn total_dirty_count(&self) -> usize {
        self.ranges.iter().map(|r| r.end - r.start).sum()
    }

    /// Check if a specific index is dirty.
    pub fn contains(&self, index: usize) -> bool {
        self.ranges
            .iter()
            .any(|range| index >= range.start && index < range.end)
    }

    /// Get the ranges as a slice.
    pub fn as_slice(&self) -> &[Range<usize>] {
        &self.ranges
    }

    /// Merge adjacent ranges (already done automatically in mark_dirty, but exposed for testing).
    pub fn merge_overlapping(&mut self) {
        if self.ranges.len() <= 1 {
            return;
        }

        let mut merged = Vec::with_capacity(self.ranges.len());
        let mut current = self.ranges[0].clone();

        for range in &self.ranges[1..] {
            if ranges_overlap_or_adjacent(&current, range) {
                // Extend current range
                current.end = current.end.max(range.end);
            } else {
                // Push completed range and start new one
                merged.push(current);
                current = range.clone();
            }
        }

        merged.push(current);
        self.ranges = merged;
    }

    /// Get statistics about the dirty ranges.
    pub fn stats(&self) -> DirtyRangeStats {
        let total_elements = self.total_dirty_count();
        let num_ranges = self.ranges.len();
        let avg_range_size = if num_ranges > 0 {
            total_elements as f32 / num_ranges as f32
        } else {
            0.0
        };

        DirtyRangeStats {
            num_ranges,
            total_elements,
            avg_range_size,
        }
    }
}

impl Default for DirtyRanges {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about dirty ranges.
#[derive(Debug, Clone, Copy)]
pub struct DirtyRangeStats {
    pub num_ranges: usize,
    pub total_elements: usize,
    pub avg_range_size: f32,
}

/// Check if two ranges overlap or are adjacent.
fn ranges_overlap_or_adjacent(a: &Range<usize>, b: &Range<usize>) -> bool {
    // Ranges overlap if: a.start < b.end && b.start < a.end
    // Ranges are adjacent if: a.end == b.start || b.end == a.start
    (a.start < b.end && b.start < a.end) || a.end == b.start || b.end == a.start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_range() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(10, 20);

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges.as_slice()[0], 10..20);
        assert_eq!(ranges.total_dirty_count(), 10);
    }

    #[test]
    fn test_non_overlapping_ranges() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(10, 20);
        ranges.mark_dirty(30, 40);

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges.as_slice()[0], 10..20);
        assert_eq!(ranges.as_slice()[1], 30..40);
    }

    #[test]
    fn test_overlapping_ranges() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(10, 20);
        ranges.mark_dirty(15, 25);

        assert_eq!(ranges.len(), 1, "Overlapping ranges should merge");
        assert_eq!(ranges.as_slice()[0], 10..25);
    }

    #[test]
    fn test_adjacent_ranges() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(10, 20);
        ranges.mark_dirty(20, 30);

        assert_eq!(ranges.len(), 1, "Adjacent ranges should merge");
        assert_eq!(ranges.as_slice()[0], 10..30);
    }

    #[test]
    fn test_contained_range() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(10, 30);
        ranges.mark_dirty(15, 20);

        assert_eq!(ranges.len(), 1, "Contained range should not split");
        assert_eq!(ranges.as_slice()[0], 10..30);
    }

    #[test]
    fn test_containing_range() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(15, 20);
        ranges.mark_dirty(10, 30);

        assert_eq!(ranges.len(), 1, "Containing range should absorb smaller");
        assert_eq!(ranges.as_slice()[0], 10..30);
    }

    #[test]
    fn test_multiple_merge() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(10, 20);
        ranges.mark_dirty(30, 40);
        ranges.mark_dirty(50, 60);
        ranges.mark_dirty(15, 55); // Should merge all three

        assert_eq!(ranges.len(), 1, "Should merge all overlapping ranges");
        assert_eq!(ranges.as_slice()[0], 10..60);
    }

    #[test]
    fn test_insertion_order() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(30, 40);
        ranges.mark_dirty(10, 20);
        ranges.mark_dirty(50, 60);

        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges.as_slice()[0], 10..20);
        assert_eq!(ranges.as_slice()[1], 30..40);
        assert_eq!(ranges.as_slice()[2], 50..60);
    }

    #[test]
    fn test_clear() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(10, 20);
        ranges.mark_dirty(30, 40);

        ranges.clear();

        assert_eq!(ranges.len(), 0);
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_contains() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(10, 20);
        ranges.mark_dirty(30, 40);

        assert!(ranges.contains(10));
        assert!(ranges.contains(15));
        assert!(ranges.contains(19));
        assert!(!ranges.contains(20));
        assert!(!ranges.contains(25));
        assert!(ranges.contains(35));
    }

    #[test]
    fn test_total_dirty_count() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(10, 20); // 10 elements
        ranges.mark_dirty(30, 35); // 5 elements

        assert_eq!(ranges.total_dirty_count(), 15);
    }

    #[test]
    fn test_invalid_range() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(20, 10); // end < start

        assert_eq!(ranges.len(), 0, "Invalid range should be ignored");
    }

    #[test]
    fn test_zero_length_range() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(10, 10); // zero length

        assert_eq!(ranges.len(), 0, "Zero-length range should be ignored");
    }

    #[test]
    fn test_batch_marking() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty_batch(vec![10..20, 30..40, 50..60]);

        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges.total_dirty_count(), 30);
    }

    #[test]
    fn test_stats() {
        let mut ranges = DirtyRanges::new();
        ranges.mark_dirty(10, 20);
        ranges.mark_dirty(30, 40);

        let stats = ranges.stats();
        assert_eq!(stats.num_ranges, 2);
        assert_eq!(stats.total_elements, 20);
        assert_eq!(stats.avg_range_size, 10.0);
    }

    #[test]
    fn test_ranges_overlap_or_adjacent_fn() {
        assert!(ranges_overlap_or_adjacent(&(10..20), &(15..25)));
        assert!(ranges_overlap_or_adjacent(&(10..20), &(20..30)));
        assert!(ranges_overlap_or_adjacent(&(20..30), &(10..20)));
        assert!(!ranges_overlap_or_adjacent(&(10..20), &(21..30)));
        assert!(ranges_overlap_or_adjacent(&(10..30), &(15..20)));
    }

    #[test]
    fn test_complex_merge_scenario() {
        let mut ranges = DirtyRanges::new();

        // Add ranges in random order
        ranges.mark_dirty(100, 110);
        ranges.mark_dirty(50, 60);
        ranges.mark_dirty(80, 90);
        ranges.mark_dirty(55, 65); // Extends 50-60 to 50-65
        ranges.mark_dirty(75, 85); // Extends 80-90 to 75-90
        ranges.mark_dirty(65, 75); // Merges 50-65 and 75-90 to 50-90

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges.as_slice()[0], 50..90);
        assert_eq!(ranges.as_slice()[1], 100..110);
    }
}
