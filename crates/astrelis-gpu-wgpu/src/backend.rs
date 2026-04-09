//! wgpu backend implementation.

use std::sync::Arc;

use astrelis_gpu::backend::{GpuBackend, GpuConfig};
use astrelis_gpu::device::{AdapterInfo, DeviceType, GpuBackendType};
use astrelis_gpu::error::GpuError;
use astrelis_window::Window;

use crate::convert::types as conv;
use crate::device::WgpuDevice;
use crate::queue::WgpuQueue;
use crate::surface::WgpuSurface;

/// wgpu-based GPU backend for the Astrelis engine.
///
/// # Example
///
/// ```ignore
/// use astrelis_gpu::{GpuBackend, GpuConfig};
/// use astrelis_gpu_wgpu::WgpuBackend;
///
/// let gpu = WgpuBackend::new(&GpuConfig::default())?;
/// println!("GPU: {}", gpu.device().adapter_info().name);
/// ```
pub struct WgpuBackend {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: Arc<WgpuDevice>,
    queue: WgpuQueue,
}

impl GpuBackend for WgpuBackend {
    type Device = WgpuDevice;
    type Queue = WgpuQueue;
    type Surface = WgpuSurface;

    fn new(config: &GpuConfig) -> Result<Self, GpuError> {
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

        let device_desc = wgpu::DeviceDescriptor {
            label: config.device_label.as_deref(),
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        };
        let (wgpu_device, wgpu_queue) = pollster::block_on(adapter.request_device(&device_desc))
            .map_err(|e| GpuError::DeviceCreationFailed(e.to_string()))?;

        let device = WgpuDevice::new(wgpu_device, wgpu_queue, adapter_info);
        let queue = WgpuQueue::new(Arc::clone(&device));

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    fn device(&self) -> &Self::Device {
        &self.device
    }

    fn queue(&self) -> &Self::Queue {
        &self.queue
    }

    fn create_surface(&self, window: &dyn Window) -> Result<Self::Surface, GpuError> {
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

        Ok(WgpuSurface::new(
            surface,
            Arc::clone(&self.device),
            capabilities,
        ))
    }
}
