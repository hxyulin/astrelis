use std::sync::{Arc, Mutex};

pub(crate) type UploadPool = Arc<Mutex<Vec<(wgpu::Buffer, Vec<u8>)>>>;

#[derive(Debug)]
struct Page {
    buffer: wgpu::Buffer,
    bytes: Vec<u8>,
    pool: UploadPool,
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
}
pub(crate) trait ResourceLease: std::fmt::Debug + Send + Sync {}
impl<T: std::fmt::Debug + Send + Sync> ResourceLease for T {}
impl DrawUploads {
    pub(crate) fn retain(&mut self, resource: Arc<dyn ResourceLease>) {
        // Repeated draws of the same prepared data need only one recording lease.
        if !self.leases.iter().any(|r| Arc::ptr_eq(r, &resource)) {
            self.leases.push(resource);
        }
    }
    pub(crate) fn retain_until_complete(&mut self, queue: &wgpu::Queue) {
        if !self.leases.is_empty() {
            let leases = std::mem::take(&mut self.leases);
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
            let (buffer, mut storage) = pool
                .iter()
                .position(|(b, _)| b.size() >= length)
                .map(|i| pool.swap_remove(i))
                .unwrap_or_else(|| {
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
        for page in &self.pages {
            queue.write_buffer(&page.buffer, 0, &page.bytes);
        }
    }
}
