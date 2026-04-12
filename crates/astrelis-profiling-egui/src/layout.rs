//! Shared depth computation for flame-graph-style rendering.
//!
//! Both [`crate::LastFrameFlameGraph`] and
//! [`crate::ProfilerWindow`] need to assign a nesting depth to every
//! visible span so nested scopes stack vertically. For well-formed
//! synchronous RAII spans on a single thread / lane, end events
//! always occur in reverse begin order, so the depth of a span
//! equals the number of still-open spans whose `end_ns` is strictly
//! greater than its `start_ns` at the moment the span begins.
//!
//! [`compute_depth`] implements that as a small end-sorted stack and
//! is reused verbatim by both widgets. The same helper works for
//! CPU and GPU spans — the caller supplies a slice of `(start_ns,
//! end_ns)` pairs already sorted by `start_ns`.

/// Computes the nesting depth of each span in a start-sorted slice
/// of `(start_ns, end_ns)` tuples.
///
/// Returned vector has the same length as the input, with
/// `depths[i]` = nesting depth of `spans[i]` (0 for root-level
/// spans, 1 for children, and so on).
///
/// # Invariants
///
/// - The input **must** be sorted ascending by `start_ns`. Callers
///   reading from [`astrelis_profiling::timeline::ThreadStream`] or
///   `GpuStream` need to sort first, because those streams are
///   `end_ns`-sorted.
/// - Spans on the same lane are assumed to be well-nested. A pair
///   of overlapping-but-not-nested spans (e.g. from sloppy manual
///   begin/end calls) will produce a visually plausible but
///   technically incorrect depth — the stack trick assumes strict
///   LIFO termination.
pub fn compute_depth(spans: &[(u64, u64)]) -> Vec<u32> {
    let mut stack: Vec<u64> = Vec::new();
    let mut depths = Vec::with_capacity(spans.len());
    for &(start_ns, end_ns) in spans {
        while let Some(&top_end) = stack.last() {
            if top_end <= start_ns {
                stack.pop();
            } else {
                break;
            }
        }
        depths.push(stack.len() as u32);
        stack.push(end_ns);
    }
    depths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_spans_are_all_depth_zero() {
        let spans = [(0, 10), (20, 30), (40, 50)];
        assert_eq!(compute_depth(&spans), vec![0, 0, 0]);
    }

    #[test]
    fn three_levels_of_nesting() {
        // outer [0, 100] contains mid [10, 90] contains inner [20, 80].
        let spans = [(0, 100), (10, 90), (20, 80)];
        assert_eq!(compute_depth(&spans), vec![0, 1, 2]);
    }

    #[test]
    fn mixed_siblings_and_children() {
        // Two top-level spans, each with a child.
        //
        // timeline:
        //   0-----100            150--------300
        //     10-50                 160-290
        let spans = [(0, 100), (10, 50), (150, 300), (160, 290)];
        assert_eq!(compute_depth(&spans), vec![0, 1, 0, 1]);
    }

    #[test]
    fn sibling_children_return_to_same_depth() {
        // Parent with two sibling children — the second child should
        // be at the same depth as the first, not one deeper.
        //
        //   0------------200
        //     10-50   60-100
        let spans = [(0, 200), (10, 50), (60, 100)];
        assert_eq!(compute_depth(&spans), vec![0, 1, 1]);
    }

    #[test]
    fn touching_end_equals_next_start_pops_parent() {
        // A span ending at `t` and a new span starting at `t` are not
        // nested — the first pops off the stack before the second
        // pushes.
        let spans = [(0, 100), (100, 200)];
        assert_eq!(compute_depth(&spans), vec![0, 0]);
    }

    #[test]
    fn empty_input() {
        assert!(compute_depth(&[]).is_empty());
    }
}
