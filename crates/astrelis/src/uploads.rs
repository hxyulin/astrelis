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
}
impl DrawUploads {
    pub(crate) fn append(
        &mut self,
        graphics: &crate::GraphicsContext,
        bytes: &[u8],
    ) -> (wgpu::Buffer, std::ops::Range<u64>) {
        let length = bytes.len() as u64;
        if self
            .pages
            .last()
            .is_none_or(|p| p.bytes.len() as u64 + length > p.buffer.size())
        {
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
        let start = page.bytes.len() as u64;
        page.bytes.extend_from_slice(bytes);
        (page.buffer.clone(), start..start + length)
    }
    pub(crate) fn upload(&self, queue: &wgpu::Queue) {
        for page in &self.pages {
            queue.write_buffer(&page.buffer, 0, &page.bytes);
        }
    }
}
