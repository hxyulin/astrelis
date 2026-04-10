//! wgpu compute pass implementation.

use std::sync::Arc;

use astrelis_gpu::command::ComputePass;
use astrelis_gpu::id::{BindGroupId, BufferId, ComputePipelineId};

use crate::device::WgpuDevice;

/// wgpu compute pass that executes commands immediately.
pub struct WgpuComputePass<'a> {
    pass: Option<wgpu::ComputePass<'a>>,
    device: &'a Arc<WgpuDevice>,
}

impl<'a> WgpuComputePass<'a> {
    pub(crate) fn new(
        encoder: &'a mut wgpu::CommandEncoder,
        device: &'a Arc<WgpuDevice>,
        label: Option<&str>,
    ) -> Self {
        astrelis_profiling::profile_function!();
        let pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label,
            ..Default::default()
        });

        Self {
            pass: Some(pass),
            device,
        }
    }

    fn pass_mut(&mut self) -> &mut wgpu::ComputePass<'a> {
        self.pass.as_mut().expect("compute pass already ended")
    }
}

impl ComputePass for WgpuComputePass<'_> {
    fn set_pipeline(&mut self, pipeline: ComputePipelineId) {
        let pipelines = self.device.compute_pipelines.read_guard();
        let p = pipelines
            .get(&pipeline.raw())
            .expect("invalid compute pipeline handle");
        self.pass_mut().set_pipeline(p);
    }

    fn set_bind_group(&mut self, index: u32, bind_group: BindGroupId, offsets: &[u32]) {
        let groups = self.device.bind_groups.read_guard();
        let bg = groups
            .get(&bind_group.raw())
            .expect("invalid bind group handle");
        self.pass_mut().set_bind_group(index, Some(bg), offsets);
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        astrelis_profiling::profile_function!();
        self.pass_mut().dispatch_workgroups(x, y, z);
    }

    fn dispatch_indirect(&mut self, buffer: BufferId, offset: u64) {
        let buffers = self.device.buffers.read_guard();
        let buf = buffers
            .get(&buffer.raw())
            .expect("invalid indirect buffer handle");
        self.pass_mut().dispatch_workgroups_indirect(buf, offset);
    }

    fn set_push_constants(&mut self, _offset: u32, _data: &[u8]) {
        // Push constants are not supported in wgpu 29+.
        unimplemented!("push constants are not supported in the wgpu 29 backend");
    }
}

