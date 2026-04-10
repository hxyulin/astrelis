//! wgpu queue implementation.

use std::sync::Arc;

use astrelis_gpu::device::GpuDevice;
use astrelis_gpu::queue::GpuQueue;

use crate::device::WgpuDevice;
use crate::encoder::WgpuCommandEncoder;

/// wgpu-backed command queue.
pub struct WgpuQueue {
    device: Arc<WgpuDevice>,
}

impl WgpuQueue {
    pub(crate) fn new(device: Arc<WgpuDevice>) -> Self {
        Self { device }
    }
}

impl GpuQueue for WgpuQueue {
    type Device = WgpuDevice;

    fn submit(
        &self,
        encoders: impl IntoIterator<Item = <Self::Device as GpuDevice>::Encoder>,
    ) {
        astrelis_profiling::profile_function!();
        let command_buffers: Vec<wgpu::CommandBuffer> = encoders
            .into_iter()
            .map(|e: WgpuCommandEncoder| e.finish())
            .collect();
        self.device.queue.submit(command_buffers);

        // Signal end of frame to the GPU profiler so it can initiate readback.
        let mut profiler = self.device.gpu_profiler.lock().unwrap();
        profiler.end_frame();
    }
}
