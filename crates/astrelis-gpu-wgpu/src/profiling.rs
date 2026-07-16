//! Wgpu timestamp collection for the Astrelis profiler.

use astrelis_gpu::{CommandEncoder, Device, Features, GpuError, GraphicsApi, PollMode, Queue};
use astrelis_profiling::{
    Profiler,
    data::GpuLaneId,
    gpu::{GpuBackend, GpuFrame, GpuScope},
};
use std::time::{Duration, Instant};

use crate::{WgpuCommandEncoder, WgpuDeviceExt, WgpuQueueExt};

/// Per-queue wgpu timestamp collector.
///
/// Metal is rejected while wgpu issue #9414 remains unresolved. On macOS,
/// select Vulkan and install MoltenVK when GPU profiling is required.
pub struct WgpuGpuProfiler {
    inner: wgpu_profiler::GpuProfiler,
    lane: GpuLaneId,
    last_calibration: Instant,
}

impl WgpuGpuProfiler {
    /// Creates and calibrates a timestamp collector for a device queue.
    pub fn new(device: &Device, queue: &Queue, label: Option<&str>) -> Result<Self, GpuError> {
        let capabilities = device.capabilities();
        let required = Features::TIMESTAMP_QUERY | Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if !capabilities.features.contains(required) {
            return Err(GpuError::new(
                "GPU profiling requires timestamp queries inside command encoders",
            ));
        }
        if !capabilities.reliable_timestamps {
            return Err(GpuError::new(
                "timestamp queries are unreliable on this backend; use Vulkan/MoltenVK on macOS",
            ));
        }
        let raw_device = device
            .as_wgpu()
            .ok_or_else(|| GpuError::new("device does not use the wgpu backend"))?;
        let raw_queue = queue
            .as_wgpu()
            .ok_or_else(|| GpuError::new("queue does not use the wgpu backend"))?;
        let lane =
            astrelis_profiling::gpu::register_gpu_lane(convert_backend(capabilities.api), label);
        let offset = calibrate(raw_device, raw_queue)?;
        astrelis_profiling::gpu::set_gpu_lane_offset_ns(lane, offset);
        let inner = wgpu_profiler::GpuProfiler::new(
            raw_device,
            wgpu_profiler::GpuProfilerSettings::default(),
        )
        .map_err(|error| GpuError::new(error.to_string()))?;
        Ok(Self {
            inner,
            lane,
            last_calibration: Instant::now(),
        })
    }

    /// Returns the profiler timeline lane assigned to this queue.
    pub fn lane(&self) -> GpuLaneId {
        self.lane
    }

    /// Records a nested encoder timestamp scope.
    pub fn scope<R>(
        &mut self,
        label: impl Into<String>,
        encoder: &mut CommandEncoder,
        record: impl FnOnce(&mut Self, &mut CommandEncoder) -> R,
    ) -> Result<R, GpuError> {
        let query = {
            let raw = raw_encoder(encoder)?;
            self.inner.begin_query(label, raw)
        };
        let result = record(self, encoder);
        {
            let raw = raw_encoder(encoder)?;
            self.inner.end_query(raw, query);
        }
        Ok(result)
    }

    /// Appends query resolve commands before the encoder is finished.
    pub fn resolve_frame(&mut self, encoder: &mut CommandEncoder) -> Result<(), GpuError> {
        self.inner.resolve_queries(raw_encoder(encoder)?);
        Ok(())
    }

    /// Marks the frame submitted. Call this immediately after queue submission.
    pub fn end_frame(&mut self) -> Result<(), GpuError> {
        self.inner
            .end_frame()
            .map_err(|error| GpuError::new(error.to_string()))
    }

    /// Processes all completed frames and forwards them to `astrelis-profiling`.
    pub fn process_finished_frames(
        &mut self,
        device: &Device,
        queue: &Queue,
    ) -> Result<usize, GpuError> {
        if self.last_calibration.elapsed() >= Duration::from_secs(5) {
            let raw_device = device
                .as_wgpu()
                .ok_or_else(|| GpuError::new("device does not use the wgpu backend"))?;
            let raw_queue = queue
                .as_wgpu()
                .ok_or_else(|| GpuError::new("queue does not use the wgpu backend"))?;
            let offset = calibrate(raw_device, raw_queue)?;
            astrelis_profiling::gpu::set_gpu_lane_offset_ns(self.lane, offset);
            self.last_calibration = Instant::now();
        }
        device.poll(PollMode::Poll)?;
        let mut processed = 0;
        while let Some(results) = self.inner.process_finished_frame(queue.timestamp_period()) {
            let scopes = results.into_iter().filter_map(convert_scope).collect();
            astrelis_profiling::gpu::report_gpu_frame(self.lane, GpuFrame { scopes });
            processed += 1;
        }
        Ok(processed)
    }
}

fn raw_encoder(encoder: &mut CommandEncoder) -> Result<&mut wgpu::CommandEncoder, GpuError> {
    encoder
        .backend_mut()?
        .as_any_mut()
        .downcast_mut::<WgpuCommandEncoder>()
        .and_then(|encoder| encoder.raw.as_mut())
        .ok_or_else(|| GpuError::new("command encoder does not use the wgpu backend"))
}

fn convert_scope(result: wgpu_profiler::GpuTimerQueryResult) -> Option<GpuScope> {
    let time = result.time?;
    Some(GpuScope {
        label: result.label,
        start_ns: (time.start * 1_000_000_000.0) as i64,
        end_ns: (time.end * 1_000_000_000.0) as i64,
        nested: result
            .nested_queries
            .into_iter()
            .filter_map(convert_scope)
            .collect(),
    })
}

fn convert_backend(api: GraphicsApi) -> GpuBackend {
    match api {
        GraphicsApi::Vulkan => GpuBackend::Vulkan,
        GraphicsApi::Metal => GpuBackend::Metal,
        GraphicsApi::Dx12 => GpuBackend::Dx12,
        GraphicsApi::Gl => GpuBackend::Gl,
        GraphicsApi::WebGpu => GpuBackend::WebGpu,
        _ => GpuBackend::Unknown,
    }
}

fn calibrate(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<i64, GpuError> {
    let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("Astrelis GPU clock calibration"),
        ty: wgpu::QueryType::Timestamp,
        count: 2,
    });
    let resolve = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Astrelis GPU clock calibration resolve"),
        size: 16,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Astrelis GPU clock calibration readback"),
        size: 16,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Astrelis GPU clock calibration"),
    });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Astrelis GPU clock calibration pass"),
            color_attachments: &[],
            depth_stencil_attachment: None,
            timestamp_writes: Some(wgpu::RenderPassTimestampWrites {
                query_set: &queries,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            }),
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    encoder.resolve_query_set(&queries, 0..2, &resolve, 0);
    encoder.copy_buffer_to_buffer(&resolve, 0, &readback, 0, 16);
    let cpu_start = Profiler::get().clock.now_ns();
    queue.submit([encoder.finish()]);
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| GpuError::new(error.to_string()))?;
    let cpu_end = Profiler::get().clock.now_ns();

    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| GpuError::new(error.to_string()))?;
    receiver
        .recv()
        .map_err(|_| GpuError::new("calibration mapping callback was dropped"))?
        .map_err(|error| GpuError::new(error.to_string()))?;
    let bytes = readback.slice(..).get_mapped_range();
    let start = u64::from_le_bytes(bytes[0..8].try_into().expect("timestamp byte count"));
    let end = u64::from_le_bytes(bytes[8..16].try_into().expect("timestamp byte count"));
    drop(bytes);
    readback.unmap();
    if start == 0 || end < start {
        return Err(GpuError::new(
            "backend returned invalid timestamp calibration samples",
        ));
    }
    let gpu_mid_ns =
        (((start as u128 + end as u128) / 2) as f64 * queue.get_timestamp_period() as f64) as i128;
    let cpu_mid_ns = ((cpu_start as u128 + cpu_end as u128) / 2) as i128;
    Ok((cpu_mid_ns - gpu_mid_ns).clamp(i64::MIN as i128, i64::MAX as i128) as i64)
}
