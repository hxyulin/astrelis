//! GPU backend entry point trait.

use astrelis_window::Window;

use crate::device::GpuDevice;
use crate::error::GpuError;
use crate::queue::GpuQueue;
use crate::surface::GpuSurface;
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

/// Top-level GPU backend entry point.
///
/// Each backend crate (e.g., `astrelis-gpu-wgpu`) provides one concrete
/// implementation of this trait. This follows the same pattern as
/// [`WindowBackend`](astrelis_window::WindowBackend).
///
/// # Example
///
/// ```ignore
/// use astrelis_gpu::{GpuBackend, GpuConfig};
/// use astrelis_gpu_wgpu::WgpuBackend;
///
/// let gpu = WgpuBackend::new(&GpuConfig::default())?;
/// let surface = gpu.create_surface(window)?;
/// ```
pub trait GpuBackend: Sized {
    /// The device type for this backend.
    type Device: GpuDevice;
    /// The queue type for this backend.
    type Queue: GpuQueue<Device = Self::Device>;
    /// The surface type for this backend.
    type Surface: GpuSurface;

    /// Initializes the GPU backend, selecting an adapter and creating a device.
    fn new(config: &GpuConfig) -> Result<Self, GpuError>;

    /// Returns a reference to the GPU device.
    fn device(&self) -> &Self::Device;

    /// Returns a reference to the GPU queue.
    fn queue(&self) -> &Self::Queue;

    /// Creates a presentation surface for the given window.
    ///
    /// The window must outlive the surface. Call
    /// [`GpuSurface::configure`] before rendering.
    fn create_surface(&self, window: &dyn Window) -> Result<Self::Surface, GpuError>;
}
