//! Unclipped raw references still execute the built-in clipping shader and bind
//! its complete interface. Keep allocation outside measured recording/upload work.
use astrelis::{GraphicsContext, wgpu};
use wgpu::util::DeviceExt as _;

pub struct DisabledClips {
    pub layout: wgpu::BindGroupLayout,
    pub group: wgpu::BindGroup,
}

impl DisabledClips {
    pub fn new(g: &GraphicsContext) -> Self {
        let layout = g.rounded_clip_layout();
        // Public WGSL ABI: vec4 header (origin, count, reserved), then capacity
        // records of three vec4s. A zero count disables the intersection loop.
        let buffer = (g.rounded_clip_capacity() > 0).then(|| {
            g.device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("raw disabled rounded clip stack"),
                    contents: &vec![0; 16 + g.rounded_clip_capacity() * 48],
                    usage: wgpu::BufferUsages::UNIFORM,
                })
        });
        let entry = buffer.as_ref().map(|buffer| wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        });
        let group = g.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raw disabled rounded clips"),
            layout: &layout,
            entries: entry.as_slice(),
        });
        Self { layout, group }
    }
}

pub fn shader(g: &GraphicsContext, source: &str) -> String {
    format!(
        "{}\n{}\n{source}",
        include_str!("../../src/clip.wgsl"),
        g.rounded_clip_shader()
    )
}
