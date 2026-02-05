//! GPU profiling via `wgpu-profiler` with puffin visualization.
//!
//! When the `gpu-profiling` feature is enabled, this module provides a
//! [`GpuFrameProfiler`] that wraps `wgpu_profiler::GpuProfiler` and
//! automatically reports GPU timing data to puffin.
//!
//! When the feature is disabled, all types and methods become zero-cost no-ops.
//!
//! # Automatic Integration
//!
//! The recommended usage is to attach the profiler to a [`RenderableWindow`](crate::RenderableWindow):
//!
//! ```ignore
//! // At init:
//! let ctx = GraphicsContext::new_owned_with_descriptor(
//!     GraphicsContextDescriptor::new()
//!         .request_capability::<GpuFrameProfiler>()
//! ).await?;
//! let profiler = Arc::new(GpuFrameProfiler::new(&ctx)?);
//! window.set_gpu_profiler(profiler);
//!
//! // Each frame — GPU profiling is fully automatic:
//! let mut frame = window.begin_drawing();
//! frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
//!     // GPU scope "main_pass" is automatically active
//! });
//! frame.finish(); // auto: resolve_queries -> submit -> end_frame
//! ```
//!
//! # Manual Scoping
//!
//! For custom GPU scopes outside of render passes:
//!
//! ```ignore
//! frame.with_gpu_scope("upload_data", |encoder| {
//!     encoder.copy_buffer_to_buffer(&src, 0, &dst, 0, size);
//! });
//! ```

use crate::capability::{GpuRequirements, RenderCapability};
use crate::features::GpuFeatures;

// ============================================================================
// RenderCapability — works in both enabled and disabled configurations
// ============================================================================

impl RenderCapability for GpuFrameProfiler {
    fn requirements() -> GpuRequirements {
        // All three timestamp features are requested (best-effort), not required.
        // wgpu-profiler gracefully degrades if any are unavailable:
        // - TIMESTAMP_QUERY: base feature, allows timestamp writes on pass definition
        // - TIMESTAMP_QUERY_INSIDE_ENCODERS: allows scopes on command encoders
        // - TIMESTAMP_QUERY_INSIDE_PASSES: allows scopes on render/compute passes
        GpuRequirements::new().request_features(
            GpuFeatures::TIMESTAMP_QUERY
                | GpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS
                | GpuFeatures::TIMESTAMP_QUERY_INSIDE_PASSES,
        )
    }

    fn name() -> &'static str {
        "GpuFrameProfiler"
    }
}

// ============================================================================
// Feature: gpu-profiling ENABLED
// ============================================================================
#[cfg(feature = "gpu-profiling")]
mod enabled {
    use std::sync::{Arc, Mutex};

    use crate::context::GraphicsContext;
    use crate::features::GpuFeatures;

    /// GPU frame profiler wrapping `wgpu_profiler::GpuProfiler`.
    ///
    /// All methods take `&self` using interior mutability (`Mutex`), making it
    /// easy to share the profiler between `RenderableWindow` and `FrameContext`
    /// via `Arc<GpuFrameProfiler>`.
    ///
    /// Create one per application. The profiler is automatically driven each frame
    /// when attached to a `RenderableWindow` via [`set_gpu_profiler`]:
    /// - GPU scopes are created around render passes in `with_pass()` / `clear_and_render()`
    /// - Queries are resolved and the frame is ended in `FrameContext::Drop`
    ///
    /// For manual use:
    /// 1. Open GPU scopes with [`scope`](Self::scope) on command encoders or render passes.
    /// 2. Call [`resolve_queries`](Self::resolve_queries) before submitting the encoder.
    /// 3. Call [`end_frame`](Self::end_frame) after queue submit.
    ///
    /// Results are automatically forwarded to puffin via
    /// `wgpu_profiler::puffin::output_frame_to_puffin`.
    ///
    /// # Timestamp Queries
    ///
    /// If the device was created with `TIMESTAMP_QUERY` enabled (via
    /// `request_capability::<GpuFrameProfiler>()`), scopes produce actual GPU
    /// timing data. Otherwise, wgpu-profiler falls back to debug groups only.
    pub struct GpuFrameProfiler {
        profiler: Mutex<wgpu_profiler::GpuProfiler>,
        timestamp_period: f32,
        has_timestamps: bool,
    }

    impl GpuFrameProfiler {
        /// Create a new GPU frame profiler.
        ///
        /// The profiler inspects the device features to determine whether
        /// `TIMESTAMP_QUERY` is available. If not, it still works but only
        /// records debug group labels (no timing data).
        pub fn new(context: &Arc<GraphicsContext>) -> Result<Self, wgpu_profiler::CreationError> {
            let has_timestamps = context.has_feature(GpuFeatures::TIMESTAMP_QUERY);
            let has_encoder_timestamps =
                context.has_feature(GpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS);
            let has_pass_timestamps =
                context.has_feature(GpuFeatures::TIMESTAMP_QUERY_INSIDE_PASSES);

            if has_timestamps {
                tracing::info!(
                    "GPU profiler: TIMESTAMP_QUERY=yes, INSIDE_ENCODERS={}, INSIDE_PASSES={}",
                    if has_encoder_timestamps { "yes" } else { "no" },
                    if has_pass_timestamps { "yes" } else { "no" },
                );
                if !has_encoder_timestamps {
                    tracing::warn!(
                        "GPU profiler: TIMESTAMP_QUERY_INSIDE_ENCODERS not available — \
                         scopes on command encoders will not produce timing data"
                    );
                }
                if !has_pass_timestamps {
                    tracing::warn!(
                        "GPU profiler: TIMESTAMP_QUERY_INSIDE_PASSES not available — \
                         scopes on render/compute passes will not produce timing data"
                    );
                }
            } else {
                tracing::warn!(
                    "GPU profiler: TIMESTAMP_QUERY not enabled — debug groups only, no timing data. \
                     Use GraphicsContextDescriptor::request_capability::<GpuFrameProfiler>() to request it."
                );
            }

            let profiler = wgpu_profiler::GpuProfiler::new(
                context.device(),
                wgpu_profiler::GpuProfilerSettings::default(),
            )?;
            let timestamp_period = context.queue().get_timestamp_period();

            Ok(Self {
                profiler: Mutex::new(profiler),
                timestamp_period,
                has_timestamps,
            })
        }

        /// Whether this profiler has actual GPU timestamp query support.
        ///
        /// If `false`, scopes still appear in the profiler as debug groups
        /// but without timing data.
        pub fn has_timestamp_queries(&self) -> bool {
            self.has_timestamps
        }

        /// Open a profiling scope on a command encoder or render/compute pass.
        ///
        /// The scope is automatically closed when the returned guard is dropped.
        ///
        /// Returns a [`GpuProfileScope`] that wraps the underlying `wgpu_profiler::Scope`
        /// and holds the `Mutex` guard. Access the recorder via `Deref`/`DerefMut`.
        ///
        /// # Panics
        ///
        /// Panics if the internal profiler lock is poisoned.
        pub fn scope<'a, Recorder: wgpu_profiler::ProfilerCommandRecorder>(
            &'a self,
            label: impl Into<String>,
            encoder_or_pass: &'a mut Recorder,
        ) -> GpuProfileScope<'a, Recorder> {
            let profiler = self.profiler.lock().unwrap();
            // SAFETY: We extend the MutexGuard's lifetime to match &self ('a).
            // This is sound because:
            // 1. The GpuProfiler lives as long as self (lifetime 'a)
            // 2. GpuProfiler::scope() only needs &self (immutable borrow)
            // 3. The caller must drop the scope before calling resolve_queries/end_frame
            //    (which is guaranteed by the frame lifecycle: scopes live within render passes,
            //    resolve/end happen in FrameContext::Drop after all passes are done)
            let profiler_ptr = &*profiler as *const wgpu_profiler::GpuProfiler;
            let profiler_ref: &'a wgpu_profiler::GpuProfiler = unsafe { &*profiler_ptr };
            let scope = profiler_ref.scope(label, encoder_or_pass);
            GpuProfileScope {
                scope,
                _borrow: profiler,
            }
        }

        /// Resolve all pending queries. Call this before submitting the encoder.
        pub fn resolve_queries(&self, encoder: &mut wgpu::CommandEncoder) {
            self.profiler.lock().unwrap().resolve_queries(encoder);
        }

        /// End the current profiling frame. Call this after queue submit.
        ///
        /// Processes finished frames and reports results to puffin.
        pub fn end_frame(&self) -> Result<(), wgpu_profiler::EndFrameError> {
            let mut profiler = self.profiler.lock().unwrap();
            profiler.end_frame()?;

            // Process any finished frames and report to puffin
            if let Some(results) = profiler.process_finished_frame(self.timestamp_period) {
                wgpu_profiler::puffin::output_frame_to_puffin(
                    &mut puffin::GlobalProfiler::lock(),
                    &results,
                );
            }

            Ok(())
        }

        /// Get a reference to the inner `Mutex<wgpu_profiler::GpuProfiler>` for advanced use.
        pub fn inner(&self) -> &Mutex<wgpu_profiler::GpuProfiler> {
            &self.profiler
        }
    }

    /// A GPU profiling scope that wraps `wgpu_profiler::Scope` and holds
    /// the `Mutex` guard.
    ///
    /// This type implements `Deref`/`DerefMut` to the underlying recorder
    /// (command encoder or render/compute pass), so you can use it as a
    /// drop-in replacement for the recorder.
    ///
    /// The scope is automatically closed (GPU timestamp written) when dropped.
    pub struct GpuProfileScope<'a, Recorder: wgpu_profiler::ProfilerCommandRecorder> {
        scope: wgpu_profiler::Scope<'a, Recorder>,
        _borrow: std::sync::MutexGuard<'a, wgpu_profiler::GpuProfiler>,
    }

    impl<Recorder: wgpu_profiler::ProfilerCommandRecorder> std::ops::Deref
        for GpuProfileScope<'_, Recorder>
    {
        type Target = Recorder;

        fn deref(&self) -> &Self::Target {
            &self.scope
        }
    }

    impl<Recorder: wgpu_profiler::ProfilerCommandRecorder> std::ops::DerefMut
        for GpuProfileScope<'_, Recorder>
    {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.scope
        }
    }
}

#[cfg(feature = "gpu-profiling")]
pub use enabled::*;

// ============================================================================
// Feature: gpu-profiling DISABLED (zero-cost no-ops)
// ============================================================================
#[cfg(not(feature = "gpu-profiling"))]
mod disabled {
    use std::sync::Arc;

    use crate::context::GraphicsContext;

    /// No-op GPU frame profiler (gpu-profiling feature disabled).
    ///
    /// All methods are no-ops that compile to nothing. The `&self` signatures
    /// match the enabled version for API compatibility.
    pub struct GpuFrameProfiler;

    impl GpuFrameProfiler {
        /// No-op: create a new GPU frame profiler.
        pub fn new(_context: &Arc<GraphicsContext>) -> Result<Self, GpuFrameProfilerError> {
            Ok(Self)
        }

        /// No-op: always returns false.
        pub fn has_timestamp_queries(&self) -> bool {
            false
        }

        /// No-op: resolve queries.
        pub fn resolve_queries(&self, _encoder: &mut wgpu::CommandEncoder) {}

        /// No-op: end frame.
        pub fn end_frame(&self) -> Result<(), GpuFrameProfilerError> {
            Ok(())
        }
    }

    /// Placeholder error type when gpu-profiling is disabled.
    #[derive(Debug)]
    pub struct GpuFrameProfilerError;

    impl std::fmt::Display for GpuFrameProfilerError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "GPU profiling is disabled")
        }
    }

    impl std::error::Error for GpuFrameProfilerError {}
}

#[cfg(not(feature = "gpu-profiling"))]
pub use disabled::*;

// ============================================================================
// Convenience Macro
// ============================================================================

/// Execute a block of code within a GPU profiling scope on a `Frame`.
///
/// When the `gpu-profiling` feature is enabled and a GPU profiler is attached
/// to the frame, this creates a GPU timing scope around the block.
/// When disabled or no profiler is attached, the block is executed directly.
///
/// # Usage
///
/// ```ignore
/// use astrelis_render::gpu_profile_scope;
///
/// gpu_profile_scope!(frame, "upload_textures", |encoder| {
///     encoder.copy_buffer_to_buffer(&src, 0, &dst, 0, size);
/// });
/// ```
#[macro_export]
macro_rules! gpu_profile_scope {
    ($frame:expr, $label:expr, $body:expr) => {
        $frame.with_gpu_scope($label, $body)
    };
}
