//! GPU backend entry point.

use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use astrelis_window::Window;

use crate::command::CommandEncoder;
use crate::convert::types as conv;
use crate::device::{AdapterInfo, DeviceType, GpuBackendType, GpuDevice};
use crate::error::GpuError;
use crate::profiling::tier;
use crate::profiling::GpuProfilingCapabilities;
use crate::profiling::GpuProfilingTier;
use crate::surface::Surface;
use crate::types::PowerPreference;

// ============================================================================
// Calibration constants
// ============================================================================

/// Default interval between GPU clock re-calibrations (nanoseconds).
const DEFAULT_CALIBRATION_NS: u64 = 5_000_000_000; // 5 s
/// Minimum interval — adaptive mode will never go below this.
const MIN_CALIBRATION_NS: u64 = 500_000_000; // 500 ms
/// Maximum interval — adaptive mode will never exceed this.
const MAX_CALIBRATION_NS: u64 = 60_000_000_000; // 60 s
/// Drift above this triggers interval halving (100 µs).
const HIGH_DRIFT_NS: i64 = 100_000;
/// Drift below this counts toward interval doubling (10 µs).
const LOW_DRIFT_NS: i64 = 10_000;
/// Number of consecutive low-drift calibrations before doubling.
const STABLE_COUNT_THRESHOLD: u32 = 3;

/// Tracks GPU clock calibration state with adaptive interval logic.
struct CalibrationState {
    /// Profiler-epoch nanosecond of the last calibration.
    last_calibration_ns: AtomicU64,
    /// Current adaptive interval in nanoseconds.
    interval_ns: AtomicU64,
    /// Last observed offset from calibration, for drift detection.
    last_offset: AtomicI64,
    /// Count of consecutive low-drift calibrations.
    stable_count: AtomicU32,
    /// User override: `Some(ns)` for fixed interval, `None` for adaptive.
    override_ns: Mutex<Option<u64>>,
}

impl CalibrationState {
    fn new(initial_ns: u64) -> Self {
        Self {
            last_calibration_ns: AtomicU64::new(initial_ns),
            interval_ns: AtomicU64::new(DEFAULT_CALIBRATION_NS),
            last_offset: AtomicI64::new(0),
            stable_count: AtomicU32::new(0),
            override_ns: Mutex::new(None),
        }
    }

    /// Returns the current effective interval in nanoseconds.
    fn effective_interval_ns(&self) -> u64 {
        if let Some(fixed) = *self.override_ns.lock().unwrap() {
            return fixed;
        }
        self.interval_ns.load(Ordering::Relaxed)
    }

    /// Updates the adaptive interval based on observed drift.
    fn update_after_calibration(&self, new_offset: i64) {
        // Skip adaptive adjustment if user has a fixed override.
        if self.override_ns.lock().unwrap().is_some() {
            self.last_offset.store(new_offset, Ordering::Relaxed);
            return;
        }

        let prev_offset = self.last_offset.swap(new_offset, Ordering::Relaxed);
        let drift = (new_offset - prev_offset).abs();

        if drift > HIGH_DRIFT_NS {
            // High drift: halve the interval (floor at minimum).
            let cur = self.interval_ns.load(Ordering::Relaxed);
            let next = (cur / 2).max(MIN_CALIBRATION_NS);
            self.interval_ns.store(next, Ordering::Relaxed);
            self.stable_count.store(0, Ordering::Relaxed);
        } else if drift < LOW_DRIFT_NS {
            // Low drift: count toward interval doubling.
            let count = self.stable_count.fetch_add(1, Ordering::Relaxed) + 1;
            if count >= STABLE_COUNT_THRESHOLD {
                let cur = self.interval_ns.load(Ordering::Relaxed);
                let next = (cur * 2).min(MAX_CALIBRATION_NS);
                self.interval_ns.store(next, Ordering::Relaxed);
                self.stable_count.store(0, Ordering::Relaxed);
            }
        } else {
            // Moderate drift: reset stability counter but keep interval.
            self.stable_count.store(0, Ordering::Relaxed);
        }
    }
}

/// Configuration for GPU backend initialization.
#[derive(Clone, Debug, Default)]
pub struct GpuConfig {
    /// Power preference for adapter selection.
    pub power_preference: PowerPreference,
    /// Whether to enable validation/debug layers.
    ///
    /// `None` defaults to `cfg!(debug_assertions)`.
    pub validation: Option<bool>,
    /// Optional debug label for the device.
    pub device_label: Option<String>,
    /// GPU clock calibration strategy.
    ///
    /// `None` (default) uses adaptive mode: the interval shortens
    /// when observed drift is high and lengthens when the clock is
    /// stable. `Some(duration)` uses a fixed interval.
    pub calibration_interval: Option<Duration>,
}

/// Concrete GPU backend wrapping wgpu.
///
/// This is the top-level entry point for all GPU operations. It owns the
/// wgpu instance, adapter, device, and queue.
///
/// # Example
///
/// ```ignore
/// use astrelis_gpu::{Gpu, GpuConfig};
///
/// let gpu = Gpu::new(&GpuConfig::default())?;
/// println!("GPU: {}", gpu.device().adapter_info().name);
///
/// let surface = gpu.create_surface(window)?;
/// ```
pub struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: GpuDevice,
    queue: wgpu::Queue,
    profiling_tier: GpuProfilingTier,
    /// Adaptive GPU clock calibration state.
    calibration: CalibrationState,
    /// Whether the GPU requires separate-submission query resolution.
    ///
    /// Detected at init via a probe that tests whether
    /// `vkCmdCopyQueryPoolResults(WAIT)` returns stale data when
    /// resolved on the same encoder. When `true`, the submit path
    /// blocks until the GPU completes before resolving queries on a
    /// separate encoder.
    needs_separate_resolve: bool,
}

impl Gpu {
    /// Initializes the GPU backend, selecting an adapter and creating a device.
    pub fn new(config: &GpuConfig) -> Result<Self, GpuError> {
        astrelis_profiling::profile_function!();
        let flags = if config.validation.unwrap_or(cfg!(debug_assertions)) {
            wgpu::InstanceFlags::debugging()
        } else {
            wgpu::InstanceFlags::empty()
        };
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        // With the `molten-vk` feature, use Vulkan via MoltenVK on
        // macOS instead of Metal. macOS 26 (Metal 4) broke the legacy
        // MTLCounterSampleBuffer API — timestamp queries return all
        // zeros. Vulkan works correctly through MoltenVK.
        #[cfg(feature = "molten-vk")]
        {
            desc.backends = wgpu::Backends::VULKAN;
        }
        #[cfg(not(feature = "molten-vk"))]
        {
            desc.backends = wgpu::Backends::all();
        }
        desc.flags = flags;
        let instance = wgpu::Instance::new(desc);

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: conv::power_preference(config.power_preference),
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .map_err(|e| GpuError::NoAdapter(e.to_string()))?;

        let info = adapter.get_info();
        let adapter_info = AdapterInfo {
            name: info.name.clone(),
            backend: match info.backend {
                wgpu::Backend::Vulkan => GpuBackendType::Vulkan,
                wgpu::Backend::Metal => GpuBackendType::Metal,
                wgpu::Backend::Dx12 => GpuBackendType::Dx12,
                wgpu::Backend::Gl => GpuBackendType::Gl,
                wgpu::Backend::BrowserWebGpu => GpuBackendType::BrowserWebGpu,
                _ => GpuBackendType::Vulkan,
            },
            device_type: match info.device_type {
                wgpu::DeviceType::DiscreteGpu => DeviceType::DiscreteGpu,
                wgpu::DeviceType::IntegratedGpu => DeviceType::IntegratedGpu,
                wgpu::DeviceType::VirtualGpu => DeviceType::VirtualGpu,
                wgpu::DeviceType::Cpu => DeviceType::Cpu,
                _ => DeviceType::Other,
            },
        };

        // Detect GPU profiling capabilities.
        let profiling_tier = tier::detect_tier(&adapter);
        let timer_features = tier::required_features(profiling_tier);
        eprintln!("GPU profiling: tier={profiling_tier:?}, features={timer_features:?}");
        if profiling_tier == GpuProfilingTier::None {
            eprintln!(
                "GPU adapter does not support any timer query features; \
                 GPU profiling will produce no timing data"
            );
        } else {
            astrelis_profiling::gpu::init_gpu_context(map_backend(info.backend));
        }

        let required_features = timer_features;
        let device_desc = wgpu::DeviceDescriptor {
            label: config.device_label.as_deref(),
            memory_hints: wgpu::MemoryHints::Performance,
            required_features,
            ..Default::default()
        };
        let (wgpu_device, wgpu_queue) = pollster::block_on(adapter.request_device(&device_desc))
            .map_err(|e| GpuError::DeviceCreationFailed(e.to_string()))?;

        let queue_clone = wgpu_queue.clone();

        let gpu_profiler = wgpu_profiler::GpuProfiler::new(
            &wgpu_device,
            wgpu_profiler::GpuProfilerSettings::default(),
        )
        .map_err(|e| GpuError::DeviceCreationFailed(format!("GPU profiler creation failed: {e}")))?;

        let device = GpuDevice::new(wgpu_device, queue_clone, adapter_info, gpu_profiler);

        // Synchronous CPU↔GPU clock calibration. Submits a single
        // timestamp query, waits for the result, and installs the
        // offset so GPU spans land at the correct position on the
        // shared timeline from the very first frame.
        let p = astrelis_profiling::profiler::Profiler::get();
        let cal_ns = if profiling_tier != GpuProfilingTier::None {
            calibrate_gpu_clock(&device.device, &wgpu_queue);
            p.clock.now_ns()
        } else {
            0
        };

        let calibration = CalibrationState::new(cal_ns);
        // Apply user-configured fixed interval if provided.
        if let Some(interval) = config.calibration_interval {
            *calibration.override_ns.lock().unwrap() = Some(interval.as_nanos() as u64);
        }
        // Seed last_offset with the initial calibration offset.
        if profiling_tier != GpuProfilingTier::None {
            calibration
                .last_offset
                .store(p.clock.gpu_offset_ns(), Ordering::Relaxed);
        }

        // Probe whether the driver requires separate-submission
        // query resolution. This is specific to MoltenVK's
        // VK_QUERY_RESULT_WAIT_BIT bug — on native Metal (or other
        // backends), zero timestamps indicate fundamentally broken
        // timestamp support, not a resolve-timing issue. Only probe
        // when the molten-vk feature is active.
        #[cfg(feature = "molten-vk")]
        let needs_separate_resolve = if profiling_tier != GpuProfilingTier::None {
            probe_needs_separate_resolve(&device.device, &wgpu_queue)
        } else {
            false
        };
        #[cfg(not(feature = "molten-vk"))]
        let needs_separate_resolve = false;

        Ok(Self {
            instance,
            adapter,
            device,
            queue: wgpu_queue,
            profiling_tier,
            calibration,
            needs_separate_resolve,
        })
    }

    /// Returns a reference to the GPU device.
    pub fn device(&self) -> &GpuDevice {
        &self.device
    }

    /// Creates a presentation surface for the given window.
    ///
    /// The window must outlive the surface. Call
    /// [`Surface::configure`] before rendering.
    pub fn create_surface(&self, window: &dyn Window) -> Result<Surface, GpuError> {
        astrelis_profiling::profile_function!();
        // SAFETY: The caller must ensure the window outlives the surface.
        let surface = unsafe {
            let raw_display = window
                .display_handle()
                .map_err(|e| GpuError::SurfaceError(e.to_string()))?;
            let raw_window = window
                .window_handle()
                .map_err(|e| GpuError::SurfaceError(e.to_string()))?;
            let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display.as_raw()),
                raw_window_handle: raw_window.as_raw(),
            };
            self.instance
                .create_surface_unsafe(target)
                .map_err(|e| GpuError::SurfaceError(e.to_string()))?
        };

        let capabilities = surface.get_capabilities(&self.adapter);

        Ok(Surface::new(
            surface,
            &self.device,
            capabilities,
        ))
    }

    /// Submits command encoders for execution on the GPU.
    ///
    /// The encoders are consumed and their commands are submitted in order.
    ///
    /// When `needs_separate_resolve` is set (detected at init via
    /// probe), submits user work first, blocks until the GPU
    /// completes, then resolves profiler queries on a separate
    /// encoder. This works around drivers (e.g. MoltenVK) where
    /// `vkCmdCopyQueryPoolResults(WAIT)` returns stale data when
    /// resolved on the same or next command buffer.
    pub fn submit<'a>(&self, encoders: impl IntoIterator<Item = CommandEncoder<'a>>) {
        astrelis_profiling::profile_function!();
        let command_buffers: Vec<wgpu::CommandBuffer> = encoders
            .into_iter()
            .map(|e: CommandEncoder<'_>| e.finish())
            .collect();

        let idx = if self.needs_separate_resolve {
            // Separate-resolve path: submit user work, block until
            // the GPU completes, then resolve profiler queries on a
            // fresh encoder. The blocking poll is typically cheap
            // (microseconds for simple passes).
            self.queue.submit(command_buffers);
            let _ = self.device.device.poll(wgpu::PollType::wait_indefinitely());

            let mut profiler = self.device.gpu_profiler.lock().unwrap();
            let mut resolve_encoder =
                self.device.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor {
                        label: Some("profiler_resolve"),
                    },
                );
            profiler.resolve_queries(&mut resolve_encoder);
            self.queue
                .submit(std::iter::once(resolve_encoder.finish()))
        } else {
            // Normal path: resolve in the same submission batch.
            let mut profiler = self.device.gpu_profiler.lock().unwrap();
            let mut resolve_encoder =
                self.device.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor {
                        label: Some("profiler_resolve"),
                    },
                );
            profiler.resolve_queries(&mut resolve_encoder);
            let mut all = command_buffers;
            all.push(resolve_encoder.finish());
            self.queue.submit(all)
        };

        *self.device.last_submission.lock().unwrap() = Some(idx);

        // Signal end of frame to the GPU profiler.
        let mut profiler = self.device.gpu_profiler.lock().unwrap();
        if let Err(e) = profiler.end_frame() {
            eprintln!("GPU profiler end_frame error: {e}");
        }
    }

    /// Returns the GPU profiling capabilities detected at initialization.
    ///
    /// Use this to determine what level of GPU timing data is available
    /// on the current platform. See [`GpuProfilingCapabilities`] for details.
    pub fn profiling_capabilities(&self) -> GpuProfilingCapabilities {
        GpuProfilingCapabilities {
            tier: self.profiling_tier,
            timestamp_period_ns: self.queue.get_timestamp_period(),
        }
    }

    /// Processes completed GPU profiling frames and submits the
    /// resulting spans to `astrelis-profiling`'s global timeline.
    ///
    /// Call once per frame, typically before
    /// `astrelis_profiling::new_frame()`. GPU timestamp results
    /// arrive 1-3 frames late due to GPU buffering; the profiler
    /// places them on the shared timeline using their absolute
    /// nanosecond timestamps, not the current frame index.
    pub fn process_profiling_frames(&self) {
        astrelis_profiling::profile_function!();
        self.device.process_gpu_profiling_frames();

        // Periodic re-calibration of the CPU↔GPU clock offset.
        // The interval is adaptive by default: shortens when drift
        // is high, lengthens when the clock is stable.
        if self.profiling_tier != GpuProfilingTier::None {
            let p = astrelis_profiling::profiler::Profiler::get();
            let now = p.clock.now_ns();
            let last = self.calibration.last_calibration_ns.load(Ordering::Relaxed);
            let elapsed = now.saturating_sub(last);
            if elapsed >= self.calibration.effective_interval_ns() {
                calibrate_gpu_clock(&self.device.device, &self.queue);
                let new_offset = p.clock.gpu_offset_ns();
                self.calibration.update_after_calibration(new_offset);
                self.calibration
                    .last_calibration_ns
                    .store(now, Ordering::Relaxed);
            }
        }
    }

    /// Returns a reference to the underlying [`wgpu::Device`].
    ///
    /// This is an escape hatch for advanced use cases (e.g., egui integration,
    /// custom compute passes) that need direct access to the raw wgpu device.
    pub fn raw_device(&self) -> &wgpu::Device {
        &self.device.device
    }

    /// Returns a reference to the underlying [`wgpu::Queue`].
    ///
    /// This is an escape hatch for advanced use cases that need direct access
    /// to the raw wgpu queue.
    pub fn raw_queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Sets the GPU clock calibration interval.
    ///
    /// Pass `None` to re-enable adaptive mode (the default). Pass
    /// `Some(duration)` to use a fixed interval. The duration is
    /// clamped to [500 ms, 60 s].
    pub fn set_calibration_interval(&self, interval: Option<Duration>) {
        *self.calibration.override_ns.lock().unwrap() = interval.map(|d| {
            (d.as_nanos() as u64)
                .clamp(MIN_CALIBRATION_NS, MAX_CALIBRATION_NS)
        });
    }

    /// Returns the current effective calibration interval.
    pub fn calibration_interval(&self) -> Duration {
        Duration::from_nanos(self.calibration.effective_interval_ns())
    }
}

/// Performs a synchronous GPU clock calibration round-trip.
///
/// Submits a single timestamp query via an empty compute pass, waits
/// for the result, and installs the CPU↔GPU offset on the profiler's
/// global clock. The offset is `cpu_mid - gpu_ns` where `cpu_mid` is
/// the midpoint of CPU timestamps taken before and after the blocking
/// poll.
fn calibrate_gpu_clock(device: &wgpu::Device, queue: &wgpu::Queue) {
    let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("gpu_clock_calibration"),
        count: 1,
        ty: wgpu::QueryType::Timestamp,
    });
    let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu_clock_cal_resolve"),
        size: 8,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu_clock_cal_readback"),
        size: 8,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let p = astrelis_profiling::profiler::Profiler::get();
    let cpu_before = p.clock.now_ns();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("gpu_clock_cal"),
    });
    {
        let _pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("gpu_clock_cal_pass"),
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set: &query_set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: None,
            }),
        });
    }
    encoder.resolve_query_set(&query_set, 0..1, &resolve_buf, 0);
    encoder.copy_buffer_to_buffer(&resolve_buf, 0, &readback_buf, 0, 8);
    queue.submit(std::iter::once(encoder.finish()));

    let slice = readback_buf.slice(..);
    let (tx, rx) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("calibration poll failed");
    rx.recv()
        .expect("calibration channel closed")
        .expect("calibration map failed");

    let cpu_after = p.clock.now_ns();

    let data = slice.get_mapped_range();
    let gpu_ticks = u64::from_le_bytes(data[..8].try_into().unwrap());
    drop(data);
    readback_buf.unmap();

    if gpu_ticks == 0 {
        eprintln!("GPU clock calibration: timestamp was zero, skipping");
        return;
    }

    let period = queue.get_timestamp_period();
    let gpu_ns = (gpu_ticks as f64 * period as f64) as i64;
    let cpu_mid = ((cpu_before as i64) + (cpu_after as i64)) / 2;
    let offset = cpu_mid - gpu_ns;

    p.clock.set_gpu_epoch_offset_ns(offset);
}

#[cfg(feature = "molten-vk")]
/// Probes whether the GPU requires separate-submission query
/// resolution by submitting a timestamp query and resolving it on
/// the same encoder. If the result is zero, the driver has a
/// `VK_QUERY_RESULT_WAIT_BIT` bug (e.g. MoltenVK) and we must use
/// pipelined resolve instead.
///
/// Returns `true` if separate resolve is needed.
fn probe_needs_separate_resolve(device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
    let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("resolve_probe"),
        count: 1,
        ty: wgpu::QueryType::Timestamp,
    });
    let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("resolve_probe_resolve"),
        size: 8,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("resolve_probe_readback"),
        size: 8,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Submit timestamp + resolve on the SAME encoder (the "normal" path).
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("resolve_probe"),
    });
    {
        let _pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("resolve_probe_pass"),
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set: &query_set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: None,
            }),
        });
    }
    encoder.resolve_query_set(&query_set, 0..1, &resolve_buf, 0);
    encoder.copy_buffer_to_buffer(&resolve_buf, 0, &readback_buf, 0, 8);
    queue.submit(std::iter::once(encoder.finish()));

    let slice = readback_buf.slice(..);
    let (tx, rx) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("resolve probe poll failed");
    rx.recv()
        .expect("resolve probe channel closed")
        .expect("resolve probe map failed");

    let data = slice.get_mapped_range();
    let ticks = u64::from_le_bytes(data[..8].try_into().unwrap());
    drop(data);
    readback_buf.unmap();

    // If the resolved timestamp is zero, the driver returned stale
    // data — same-encoder resolve doesn't work.
    let needs_separate = ticks == 0;
    if needs_separate {
        eprintln!(
            "GPU resolve probe: same-encoder resolve returned zero — \
             enabling pipelined resolve workaround"
        );
    } else {
        eprintln!("GPU resolve probe: same-encoder resolve works correctly");
    }
    needs_separate
}

/// Maps a `wgpu::Backend` to the backend-agnostic profiling enum.
fn map_backend(backend: wgpu::Backend) -> astrelis_profiling::gpu::GpuBackend {
    use astrelis_profiling::gpu::GpuBackend;
    match backend {
        wgpu::Backend::Vulkan => GpuBackend::Vulkan,
        wgpu::Backend::Metal => GpuBackend::Metal,
        wgpu::Backend::Dx12 => GpuBackend::Dx12,
        wgpu::Backend::Gl => GpuBackend::Gl,
        wgpu::Backend::BrowserWebGpu => GpuBackend::WebGpu,
        _ => GpuBackend::Unknown,
    }
}
