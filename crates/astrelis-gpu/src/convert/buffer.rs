//! Buffer-related type conversions.

use crate::buffer::BufferUsages;

pub(crate) fn buffer_usages(u: BufferUsages) -> wgpu::BufferUsages {
    wgpu::BufferUsages::from_bits_truncate(u.bits())
}
