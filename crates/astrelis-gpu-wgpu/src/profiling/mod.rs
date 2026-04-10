//! Custom GPU timestamp profiling system.
//!
//! Replaces `wgpu-profiler` with a lightweight, engine-owned implementation
//! that integrates directly with [`astrelis_profiling`] and supports
//! tier-aware timestamp insertion.

mod bridge;
pub(crate) mod query_pool;
pub(crate) mod tier;

use astrelis_gpu::profiling::GpuProfilingTier;

use self::query_pool::{QueryPool, TimestampPair};

/// Maximum number of frames that can be in-flight before we recycle pools.
const MAX_PENDING_FRAMES: usize = 4;

/// A GPU timestamp profiler that manages query pools and frame lifecycle.
///
/// Created once at device initialization, stored in `WgpuDevice`.
pub(crate) struct GpuTimestampProfiler {
    /// The detected profiling tier.
    tier: GpuProfilingTier,
    /// Timestamp period in nanoseconds per tick.
    timestamp_period_ns: f32,
    /// The active query pool for the current frame.
    active_pool: Option<QueryPool>,
    /// Scopes pending in the current frame (before resolve).
    active_scopes: Vec<PendingScope>,
    /// Frames that have been submitted but not yet read back.
    pending_frames: Vec<PendingFrame>,
    /// Recycled pools available for reuse.
    free_pools: Vec<QueryPool>,
    /// Reference to the wgpu device for creating new pools and polling.
    device: wgpu::Device,
}

/// A scope pending timestamp resolution.
#[derive(Clone, Debug)]
pub(crate) struct PendingScope {
    /// Human-readable label.
    pub(crate) label: String,
    /// Timestamp query pair (start + end indices in the query pool).
    pub(crate) pair: TimestampPair,
}

/// A frame's worth of query data waiting for GPU readback.
struct PendingFrame {
    pool: QueryPool,
    scopes: Vec<PendingScope>,
    /// Whether `map_readback()` has been called on this frame's pool.
    map_requested: bool,
}

impl GpuTimestampProfiler {
    /// Creates a new GPU timestamp profiler.
    ///
    /// `tier` determines what kind of timestamp queries will be inserted.
    /// `timestamp_period_ns` converts raw GPU ticks to nanoseconds.
    pub(crate) fn new(
        device: wgpu::Device,
        tier: GpuProfilingTier,
        timestamp_period_ns: f32,
    ) -> Self {
        Self {
            tier,
            timestamp_period_ns,
            active_pool: None,
            active_scopes: Vec::new(),
            pending_frames: Vec::new(),
            free_pools: Vec::new(),
            device,
        }
    }

    /// Returns the detected profiling tier.
    pub(crate) fn tier(&self) -> GpuProfilingTier {
        self.tier
    }

    /// Returns the timestamp period in nanoseconds per tick.
    pub(crate) fn timestamp_period_ns(&self) -> f32 {
        self.timestamp_period_ns
    }

    /// Allocates a timestamp pair and records a pending scope.
    ///
    /// Returns `None` if the tier does not support timestamps.
    pub(crate) fn begin_scope(&mut self, label: &str) -> Option<TimestampPair> {
        if self.tier == GpuProfilingTier::None {
            return None;
        }

        let pool = self.ensure_active_pool();
        let pair = pool.allocate_pair();

        if let Some(pair) = pair {
            self.active_scopes.push(PendingScope {
                label: label.to_owned(),
                pair,
            });
            Some(pair)
        } else {
            // Pool is full — allocate a new one.
            // For simplicity in V1, we just warn. A more sophisticated
            // implementation would chain multiple pools per frame.
            eprintln!(
                "GPU profiler: query pool exhausted, dropping scope '{label}'. \
                 Consider reducing the number of profiled passes per frame."
            );
            None
        }
    }

    /// Returns a reference to the active query pool's `QuerySet`.
    pub(crate) fn active_query_set(&self) -> Option<&wgpu::QuerySet> {
        self.active_pool.as_ref().map(|p| p.query_set())
    }

    /// Resolves all queries in the active pool and prepares for frame submission.
    ///
    /// Call this after all command encoders have been finished but before
    /// submitting the command buffer.
    pub(crate) fn resolve_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(pool) = self.active_pool.take() {
            if !pool.is_empty() {
                pool.resolve(encoder);
            }
            let scopes = std::mem::take(&mut self.active_scopes);
            self.pending_frames.push(PendingFrame {
                pool,
                scopes,
                map_requested: false,
            });
        }
    }

    /// Signals the end of a frame. Initiates async readback of pending frames
    /// that haven't been mapped yet.
    pub(crate) fn end_frame(&mut self) {
        for frame in &mut self.pending_frames {
            if !frame.map_requested && !frame.pool.is_empty() {
                frame.pool.map_readback();
                frame.map_requested = true;
            }
        }
    }

    /// Processes finished frames: reads back timestamps, converts to
    /// [`GpuScope`], and reports to the profiling backend.
    ///
    /// Call once per frame (e.g., before `astrelis_profiling::new_frame()`).
    pub(crate) fn process_finished_frames(&mut self) {
        // Poll the device to drive pending map_async callbacks to completion.
        let _ = self.device.poll(wgpu::PollType::Poll);

        // Process all frames whose readback mapping has completed.
        while !self.pending_frames.is_empty() {
            let frame = &self.pending_frames[0];

            // Empty pools or pools with completed mapping can be read.
            if frame.pool.is_empty() || frame.pool.is_mapping_ready() {
                let frame = self.pending_frames.remove(0);
                if !frame.pool.is_empty() {
                    let ts = frame.pool.read_and_unmap();
                    bridge::report_results(
                        &frame.scopes,
                        &ts,
                        self.timestamp_period_ns,
                    );
                }
                // Recycle the pool.
                let mut pool = frame.pool;
                pool.reset();
                self.free_pools.push(pool);
            } else {
                // Oldest frame not ready yet — stop processing.
                break;
            }
        }

        // Limit pending frames to prevent unbounded growth.
        while self.pending_frames.len() > MAX_PENDING_FRAMES {
            let frame = self.pending_frames.remove(0);
            eprintln!(
                "GPU profiler: dropping old frame ({} scopes) — too many pending frames",
                frame.scopes.len()
            );
            let mut pool = frame.pool;
            pool.reset();
            self.free_pools.push(pool);
        }
    }

    /// Ensures there is an active query pool, creating or recycling one if needed.
    fn ensure_active_pool(&mut self) -> &mut QueryPool {
        if self.active_pool.is_none() {
            let pool = self
                .free_pools
                .pop()
                .unwrap_or_else(|| QueryPool::new(&self.device));
            self.active_pool = Some(pool);
        }
        self.active_pool.as_mut().unwrap()
    }
}
