//! Monitor type conversions from winit to astrelis.

use astrelis_core::geometry::{Physical, Point, Size};
use astrelis_window::monitor::{MonitorId, MonitorInfo, VideoMode};

/// Converts a winit MonitorHandle to astrelis MonitorInfo.
pub(crate) fn convert_monitor(handle: &winit::monitor::MonitorHandle, id: u64) -> MonitorInfo {
    let pos = handle.position();
    let size = handle.size();

    let video_modes = handle
        .video_modes()
        .map(|vm| {
            let size = vm.size();
            VideoMode {
                width: size.width,
                height: size.height,
                refresh_rate_millihertz: vm.refresh_rate_millihertz(),
                bit_depth: vm.bit_depth(),
            }
        })
        .collect();

    MonitorInfo {
        id: MonitorId::from_raw(id),
        name: handle.name(),
        position: Point::<Physical>::new(pos.x as f32, pos.y as f32),
        size: Size::<Physical>::new(size.width as f32, size.height as f32),
        scale_factor: handle.scale_factor() as f32,
        video_modes,
    }
}
