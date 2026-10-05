//! Reuse completed geometry allocations without mutating any retained text.

use crate::GraphicsContext;
use std::sync::{Arc, Mutex, Weak};

// This bounds recycled storage, including buffers waiting for GPU completion.
// Live prepared texts keep their own allocations.
const MAX_IDLE_BYTES: u64 = 1024 * 1024;
const MAX_IDLE_BUFFERS: usize = 64;
const MAX_POOLED_BUFFER: u64 = 64 * 1024;

#[derive(Debug, Default)]
pub(super) struct GeometryPool {
    buffers: Vec<(wgpu::Buffer, Weak<()>)>,
    bytes: u64,
}

#[derive(Debug)]
pub(super) struct Geometry {
    pub buffer: wgpu::Buffer,
    pub lease: Arc<()>,
    pool: Weak<Mutex<GeometryPool>>,
}

impl GeometryPool {
    pub fn upload(
        pool: &Arc<Mutex<Self>>,
        graphics: &GraphicsContext,
        contents: &[u8],
    ) -> (Geometry, bool) {
        let size = contents.len() as u64;
        let reusable = {
            let mut idle = pool.lock().unwrap_or_else(|e| e.into_inner());
            let best = idle
                .buffers
                .iter()
                .enumerate()
                .filter(|(_, (b, lease))| b.size() >= size && lease.strong_count() == 0)
                .min_by_key(|(_, (b, _))| b.size())
                .map(|(i, _)| i);
            best.map(|i| {
                let (buffer, _) = idle.buffers.swap_remove(i);
                idle.bytes -= buffer.size();
                buffer
            })
        };
        let reused = reusable.is_some();
        let buffer = if let Some(buffer) = reusable {
            graphics.queue().write_buffer(&buffer, 0, contents);
            buffer
        } else {
            // Keep large paragraphs exact-sized. Small labels can grow within a bucket.
            let capacity = if size <= MAX_POOLED_BUFFER {
                size.next_power_of_two()
            } else {
                size
            }
            .min(graphics.device().limits().max_buffer_size & !3);
            let buffer = graphics.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("Astrelis prepared glyphs"),
                size: capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: true,
            });
            buffer
                .slice(..)
                .get_mapped_range_mut()
                .unwrap()
                .slice(..contents.len())
                .copy_from_slice(contents);
            buffer.unmap();
            buffer
        };
        (
            Geometry {
                buffer,
                lease: Arc::new(()),
                pool: Arc::downgrade(pool),
            },
            reused,
        )
    }
}

impl Drop for Geometry {
    fn drop(&mut self) {
        let size = self.buffer.size();
        if size > MAX_POOLED_BUFFER {
            return;
        }
        if let Some(pool) = self.pool.upgrade() {
            let mut idle = pool.lock().unwrap_or_else(|e| e.into_inner());
            if idle.buffers.len() < MAX_IDLE_BUFFERS && idle.bytes + size <= MAX_IDLE_BYTES {
                idle.bytes += size;
                idle.buffers
                    .push((self.buffer.clone(), Arc::downgrade(&self.lease)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_geometry_storage_is_bounded_and_large_buffers_are_released() {
        pollster::block_on(async {
            let graphics = GraphicsContext::headless().await.unwrap();
            let pool = Arc::default();
            let bytes = vec![0; 16 * 1024];
            let live: Vec<_> = (0..70)
                .map(|_| GeometryPool::upload(&pool, &graphics, &bytes).0)
                .collect();
            assert_eq!(pool.lock().unwrap().bytes, 0);
            drop(live);
            assert_eq!(pool.lock().unwrap().buffers.len(), MAX_IDLE_BUFFERS);
            assert_eq!(pool.lock().unwrap().bytes, MAX_IDLE_BYTES);
            let (large, reused) = GeometryPool::upload(&pool, &graphics, &vec![0; 128 * 1024]);
            assert!(!reused);
            drop(large);
            assert_eq!(pool.lock().unwrap().bytes, MAX_IDLE_BYTES);
            let (small, reused) = GeometryPool::upload(&pool, &graphics, &[0; 1024]);
            assert!(reused);
            assert_eq!(pool.lock().unwrap().bytes, MAX_IDLE_BYTES - 16 * 1024);
            // Pool ownership is weak: a retained allocation cannot keep idle buffers alive.
            let weak = Arc::downgrade(&pool);
            drop(pool);
            assert!(weak.upgrade().is_none());
            drop(small);
        });
    }
}
