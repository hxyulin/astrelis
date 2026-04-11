//! Monotonic nanosecond clock and CPU↔GPU timestamp alignment.
//!
//! All timestamps in the global [`Timeline`](crate::timeline::Timeline)
//! are nanoseconds since the profiler epoch — the `Instant` captured
//! at profiler initialization. Using a single engine-wide epoch keeps
//! CPU and GPU spans on the same axis.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Instant;

/// Monotonic clock rooted at the profiler's construction `Instant`.
///
/// `now_ns()` returns nanoseconds since that epoch; the raw `Instant`
/// is exposed for callers that want to compute wall-clock relationships
/// with other parts of the engine.
pub struct Clock {
    epoch: Instant,
    /// Offset from GPU ticks to profiler nanoseconds. Set the first
    /// time the GPU subsystem calibrates by calling
    /// [`Clock::set_gpu_epoch_offset_ns`].
    ///
    /// Stored as `i64` because GPU ticks can be larger or smaller
    /// than the CPU clock at calibration time (subtraction can be
    /// negative).
    gpu_offset_ns: AtomicI64,
    /// `true` once the GPU epoch has been calibrated. Until then,
    /// GPU timestamp conversion falls back to using the raw GPU tick
    /// as if it were already profiler-relative.
    gpu_calibrated: AtomicU64,
}

impl Clock {
    /// Creates a new clock whose epoch is "now".
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
            gpu_offset_ns: AtomicI64::new(0),
            gpu_calibrated: AtomicU64::new(0),
        }
    }

    /// Returns the profiler epoch `Instant`.
    pub fn epoch(&self) -> Instant {
        self.epoch
    }

    /// Returns nanoseconds elapsed since the profiler epoch.
    ///
    /// Clamped to `u64::MAX` in the unlikely event of an overflow
    /// (would require the engine to run for ~584 years).
    #[inline]
    pub fn now_ns(&self) -> u64 {
        let elapsed = self.epoch.elapsed();
        elapsed
            .as_secs()
            .saturating_mul(1_000_000_000)
            .saturating_add(elapsed.subsec_nanos() as u64)
    }

    /// Records the CPU↔GPU clock offset learned from a calibration
    /// reading.
    ///
    /// The initial offset is installed at device creation; periodic
    /// re-calibration keeps it fresh as the GPU clock drifts.
    pub fn set_gpu_epoch_offset_ns(&self, offset_ns: i64) {
        self.gpu_offset_ns.store(offset_ns, Ordering::Relaxed);
        self.gpu_calibrated.store(1, Ordering::Release);
    }

    /// Returns `true` if a GPU clock offset has been installed.
    #[inline]
    pub fn gpu_calibrated(&self) -> bool {
        self.gpu_calibrated.load(Ordering::Acquire) != 0
    }

    /// Converts a raw GPU nanosecond timestamp to the profiler epoch.
    ///
    /// Before calibration, returns the input unchanged.
    #[inline]
    pub fn gpu_to_profiler_ns(&self, gpu_ns: u64) -> u64 {
        let offset = self.gpu_offset_ns.load(Ordering::Relaxed);
        if offset >= 0 {
            gpu_ns.saturating_add(offset as u64)
        } else {
            gpu_ns.saturating_sub((-offset) as u64)
        }
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_ns_is_monotonic() {
        let c = Clock::new();
        let a = c.now_ns();
        let b = c.now_ns();
        assert!(b >= a);
    }

    #[test]
    fn gpu_offset_roundtrip() {
        let c = Clock::new();
        c.set_gpu_epoch_offset_ns(1_000_000);
        assert_eq!(c.gpu_to_profiler_ns(500), 1_000_500);
    }

    #[test]
    fn gpu_offset_negative_roundtrip() {
        let c = Clock::new();
        c.set_gpu_epoch_offset_ns(-500);
        assert_eq!(c.gpu_to_profiler_ns(1_000), 500);
    }
}
