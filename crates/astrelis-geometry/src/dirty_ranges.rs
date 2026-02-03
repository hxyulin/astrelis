//! Efficient dirty range tracking for partial buffer updates.
//!
//! Tracks which ranges of instance buffers need GPU updates.

use std::ops::Range;

/// Tracks dirty ranges in a buffer for partial updates.
///
/// Maintains a sorted list of non-overlapping ranges that need updating.
/// Automatically merges adjacent and overlapping ranges for efficiency.
#[derive(Debug, Clone, Default)]
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
    pub fn mark_dirty(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }

        let new_range = start..end;
        let mut insert_idx = self.ranges.len();
        let mut merge_start_idx = None;
        let mut merge_end_idx = None;

        for (i, range) in self.ranges.iter().enumerate() {
            if ranges_overlap_or_adjacent(&new_range, range) {
                if merge_start_idx.is_none() {
                    merge_start_idx = Some(i);
                }
                merge_end_idx = Some(i);
            } else if range.start > end {
                if merge_start_idx.is_none() {
                    insert_idx = i;
                }
                break;
            }
        }

        match (merge_start_idx, merge_end_idx) {
            (Some(start_idx), Some(end_idx)) => {
                let merged_start = self.ranges[start_idx].start.min(start);
                let merged_end = self.ranges[end_idx].end.max(end);
                self.ranges.drain(start_idx..=end_idx);
                self.ranges.insert(start_idx, merged_start..merged_end);
            }
            (None, None) => {
                self.ranges.insert(insert_idx, new_range);
            }
            _ => unreachable!(),
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

    /// Get total number of dirty elements.
    pub fn total_dirty_count(&self) -> usize {
        self.ranges.iter().map(|r| r.end - r.start).sum()
    }
}

fn ranges_overlap_or_adjacent(a: &Range<usize>, b: &Range<usize>) -> bool {
    (a.start < b.end && b.start < a.end) || a.end == b.start || b.end == a.start
}
