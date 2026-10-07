use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

pub(crate) type UploadPool = Arc<Mutex<IdlePages>>;

/// Recordings an idle page may go unused before it is released. A spike frame's
/// extra pages are freed after this many later recordings; pages that steady
/// frames keep using are never idle that long.
pub(crate) const IDLE_RECORDINGS: u64 = 120;

/// Completed upload pages with their CPU staging, most recently returned last.
#[derive(Debug, Default)]
pub(crate) struct IdlePages {
    pages: Vec<(wgpu::Buffer, Vec<u8>)>,
    // Recording count when each page was returned, parallel to `pages` and
    // nondecreasing because pages are appended as they return.
    returned: Vec<u64>,
    recordings: u64,
}
impl std::ops::Deref for IdlePages {
    type Target = [(wgpu::Buffer, Vec<u8>)];
    fn deref(&self) -> &Self::Target {
        &self.pages
    }
}
impl IdlePages {
    fn push(&mut self, page: (wgpu::Buffer, Vec<u8>)) {
        self.pages.push(page);
        self.returned.push(self.recordings);
    }
    // Prefers the most recently returned page so surplus pages stay idle and age out.
    fn take(&mut self, length: u64) -> Option<(wgpu::Buffer, Vec<u8>)> {
        let index = self.pages.iter().rposition(|(b, _)| b.size() >= length)?;
        self.returned.remove(index);
        Some(self.pages.remove(index))
    }
    fn finish_recording(&mut self) {
        self.recordings += 1;
        let oldest = self.recordings.saturating_sub(IDLE_RECORDINGS);
        if self.returned.first().is_some_and(|&r| r < oldest) {
            let mut index = 0;
            self.pages.retain(|_| {
                index += 1;
                self.returned[index - 1] >= oldest
            });
            self.returned.retain(|&r| r >= oldest);
        }
    }
}

#[derive(Debug)]
struct Page {
    buffer: wgpu::Buffer,
    bytes: Vec<u8>,
    pool: UploadPool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_leases_deduplicate_small_and_large_collections() {
        let resources: Vec<_> = (0..600).map(Arc::new).collect();
        let weak = Arc::downgrade(&resources[599]);
        let mut uploads = DrawUploads::default();
        for r in &resources {
            uploads.retain(r.clone());
            uploads.retain(r.clone());
        }
        for r in resources.iter().rev() {
            uploads.retain(r.clone());
        }
        assert_eq!(uploads.leases.len(), resources.len());
        drop(resources);
        assert!(weak.upgrade().is_some());
        drop(uploads);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn spike_pages_are_released_after_idle_recordings_and_steady_pages_kept() {
        use crate::{FramebufferOptions, GraphicsContext, Rect, ShapeDraw, ShapeRenderer};
        pollster::block_on(async {
            let g = GraphicsContext::headless().await.unwrap();
            let mut target = g
                .create_framebuffer(FramebufferOptions::new(16, 16))
                .unwrap();
            let mut shapes = ShapeRenderer::new(&g);
            let mut frame = |count: usize| {
                let draws = vec![ShapeDraw::rect(Rect::new(0., 0., 4., 4.), [1.; 4]); count];
                let mut frame = target.begin_frame().unwrap();
                shapes
                    .draw_many(&mut frame.render_pass().begin().unwrap(), &draws)
                    .unwrap();
                frame.finish().unwrap();
            };
            // 5,000 records of 96 bytes need eight 64 KiB pages.
            frame(5000);
            assert_eq!(g.upload_pool.lock().unwrap().len(), 8);
            frame(10);
            let steady = g.upload_pool.lock().unwrap().last().unwrap().0.clone();
            for _ in 0..IDLE_RECORDINGS {
                frame(10);
            }
            let pool = g.upload_pool.lock().unwrap();
            assert_eq!(pool.len(), 1);
            assert_eq!(pool[0].0, steady);
        });
    }
}
impl Drop for Page {
    fn drop(&mut self) {
        self.pool
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((self.buffer.clone(), std::mem::take(&mut self.bytes)));
    }
}
/// Pages are leased exclusively until the recording submits or is abandoned.
/// Submitted pages can be reused: subsequent queue writes are ordered after the
/// earlier submission. Never upload a page before its owning recording finishes.
#[derive(Debug, Default)]
pub(crate) struct DrawUploads {
    pages: Vec<Page>,
    leases: Vec<Arc<dyn ResourceLease>>,
    lease_ids: Option<HashSet<usize>>,
    // Counts this recording toward idle page expiry when it ends.
    pool: Option<UploadPool>,
}
impl Drop for DrawUploads {
    fn drop(&mut self) {
        self.pages.clear();
        if let Some(pool) = &self.pool {
            pool.lock()
                .unwrap_or_else(|e| e.into_inner())
                .finish_recording();
        }
    }
}
pub(crate) trait ResourceLease: std::fmt::Debug + Send + Sync {}
impl<T: std::fmt::Debug + Send + Sync> ResourceLease for T {}
impl DrawUploads {
    pub(crate) fn new(pool: &UploadPool) -> Self {
        let mut uploads = Self::default();
        uploads.pool = Some(pool.clone());
        uploads
    }
    /// Retains `resource` unless it was the last one retained, without touching
    /// its reference count in that case.
    pub(crate) fn retain_ref<T: ResourceLease + 'static>(&mut self, resource: &Arc<T>) {
        if self
            .leases
            .last()
            .is_some_and(|r| std::ptr::addr_eq(Arc::as_ptr(r), Arc::as_ptr(resource)))
        {
            return;
        }
        self.retain(resource.clone());
    }
    pub(crate) fn retain(&mut self, resource: Arc<dyn ResourceLease>) {
        // Small passes need no hash allocation. Larger collections must avoid a
        // quadratic scan when each label retains independent geometry.
        if self
            .leases
            .last()
            .is_some_and(|r| Arc::ptr_eq(r, &resource))
        {
            return;
        }
        if self.lease_ids.is_none() && self.leases.len() >= 8 {
            self.lease_ids = Some(
                self.leases
                    .iter()
                    .map(|r| Arc::as_ptr(r) as *const () as usize)
                    .collect(),
            );
        }
        let fresh = if let Some(ids) = &mut self.lease_ids {
            ids.insert(Arc::as_ptr(&resource) as *const () as usize)
        } else {
            !self.leases.iter().any(|r| Arc::ptr_eq(r, &resource))
        };
        if fresh {
            self.leases.push(resource);
        }
    }
    pub(crate) fn retain_until_complete(&mut self, queue: &wgpu::Queue) {
        if !self.leases.is_empty() {
            let leases = std::mem::take(&mut self.leases);
            self.lease_ids = None;
            queue.on_submitted_work_done(move || drop(leases));
        }
    }
    pub(crate) fn append(
        &mut self,
        graphics: &crate::GraphicsContext,
        bytes: &[u8],
        alignment: u64,
    ) -> (wgpu::Buffer, std::ops::Range<u64>) {
        let length = bytes.len() as u64;
        if self.pages.last().is_none_or(|p| {
            (p.bytes.len() as u64).div_ceil(alignment) * alignment + length > p.buffer.size()
        }) {
            let mut pool = graphics
                .upload_pool
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let (buffer, mut storage) = pool.take(length).unwrap_or_else(|| {
                let buffer = graphics.device().create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Astrelis draw upload page"),
                    size: length
                        .max(64 * 1024)
                        .min(graphics.device().limits().max_buffer_size),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let bytes = Vec::with_capacity(buffer.size() as usize);
                (buffer, bytes)
            });
            storage.clear();
            self.pages.push(Page {
                bytes: storage,
                buffer,
                pool: graphics.upload_pool.clone(),
            });
        }
        let page = self.pages.last_mut().unwrap();
        // Renderers with different instance strides can share a page. Callers
        // using first_instance offsets must align starts to their record stride.
        let start = (page.bytes.len() as u64).div_ceil(alignment) * alignment;
        page.bytes.resize(start as usize, 0);
        page.bytes.extend_from_slice(bytes);
        (page.buffer.clone(), start..start + length)
    }
    pub(crate) fn upload(&self, queue: &wgpu::Queue) {
        profiling::scope!("astrelis::upload_instances");
        for page in &self.pages {
            queue.write_buffer(&page.buffer, 0, &page.bytes);
        }
    }
}
