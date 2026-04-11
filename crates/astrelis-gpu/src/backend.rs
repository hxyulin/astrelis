//! GPU backend entry point.

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
        desc.backends = wgpu::Backends::all();
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

        // Construct `wgpu_profiler::GpuProfiler` directly rather than
        // going through the `new_with_tracy_client` helper, which
        // calls `device.poll(wait_indefinitely)` during calibration
        // and hangs on macOS Metal. The profiler's GPU lane is
        // registered lazily inside `astrelis-profiling` on the first
        // reported frame, which also sidesteps the calibration
        // ordering problem Metal has with standalone timestamp
        // queries.
        let gpu_profiler = wgpu_profiler::GpuProfiler::new(
            &wgpu_device,
            wgpu_profiler::GpuProfilerSettings::default(),
        )
        .map_err(|e| GpuError::DeviceCreationFailed(format!("GPU profiler creation failed: {e}")))?;

        let device = GpuDevice::new(wgpu_device, queue_clone, adapter_info, gpu_profiler);

        Ok(Self {
            instance,
            adapter,
            device,
            queue: wgpu_queue,
            profiling_tier,
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
    pub fn submit<'a>(&self, encoders: impl IntoIterator<Item = CommandEncoder<'a>>) {
        astrelis_profiling::profile_function!();
        let command_buffers: Vec<wgpu::CommandBuffer> = encoders
            .into_iter()
            .map(|e: CommandEncoder<'_>| e.finish())
            .collect();
        self.queue.submit(command_buffers);

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
