//! Typed GPU resource wrappers.
//!
//! This module provides lightweight wrappers around wgpu types that add
//! type safety and metadata tracking without fully encapsulating wgpu.
//!
//! # Types
//!
//! - [`TypedBuffer`]: A buffer that tracks its element type, length, and usage
//! - [`GpuTexture`]: A texture with cached view and metadata
//! - [`StorageTexture`]: A texture for compute shader read/write access
//!
//! # Example
//!
//! ```ignore
//! use astrelis_render::{GraphicsContext, TypedBuffer, GraphicsContextExt};
//!
//! let ctx = GraphicsContext::new_owned_sync_or_panic();
//!
//! // Create a typed buffer of f32 values
//! let data = [1.0f32, 2.0, 3.0, 4.0];
//! let buffer = TypedBuffer::new(
//!     ctx.device(),
//!     Some("My Buffer"),
//!     &data,
//!     wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
//! );
//!
//! // Later, update the buffer
//! buffer.write(ctx.queue(), &[5.0, 6.0, 7.0, 8.0]);
//! ```

use std::marker::PhantomData;

use crate::extension::{AsWgpu, IntoWgpu};

// =============================================================================
// TypedBuffer
// =============================================================================

/// A GPU buffer with type-safe element tracking.
///
/// This wrapper adds:
/// - Type safety via generics
/// - Automatic length tracking
/// - Convenient write operations
///
/// The underlying buffer is directly accessible via the `AsWgpu` trait.
pub struct TypedBuffer<T: bytemuck::Pod> {
    buffer: wgpu::Buffer,
    len: u32,
    usage: wgpu::BufferUsages,
    _marker: PhantomData<T>,
}

impl<T: bytemuck::Pod> TypedBuffer<T> {
    /// Create a new typed buffer with initial data.
    ///
    /// # Arguments
    ///
    /// * `device` - The wgpu device to create the buffer on
    /// * `label` - Optional debug label for the buffer
    /// * `data` - Initial data to populate the buffer with
    /// * `usage` - Buffer usage flags
    pub fn new(
        device: &wgpu::Device,
        label: Option<&str>,
        data: &[T],
        usage: wgpu::BufferUsages,
    ) -> Self {
        use wgpu::util::DeviceExt;

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label,
            contents: bytemuck::cast_slice(data),
            usage,
        });

        Self {
            buffer,
            len: data.len() as u32,
            usage,
            _marker: PhantomData,
        }
    }

    /// Create an empty typed buffer with a given capacity.
    ///
    /// # Arguments
    ///
    /// * `device` - The wgpu device to create the buffer on
    /// * `label` - Optional debug label for the buffer
    /// * `capacity` - Number of elements the buffer can hold
    /// * `usage` - Buffer usage flags
    pub fn with_capacity(
        device: &wgpu::Device,
        label: Option<&str>,
        capacity: u32,
        usage: wgpu::BufferUsages,
    ) -> Self {
        let size = (capacity as usize * std::mem::size_of::<T>()) as u64;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label,
            size,
            usage,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            len: 0,
            usage,
            _marker: PhantomData,
        }
    }

    /// Get the number of elements in the buffer.
    #[inline]
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Check if the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the size of the buffer in bytes.
    #[inline]
    pub fn size(&self) -> u64 {
        self.len as u64 * std::mem::size_of::<T>() as u64
    }

    /// Get the capacity of the buffer in bytes.
    #[inline]
    pub fn capacity_bytes(&self) -> u64 {
        self.buffer.size()
    }

    /// Get the capacity in number of elements.
    #[inline]
    pub fn capacity(&self) -> u32 {
        (self.buffer.size() / std::mem::size_of::<T>() as u64) as u32
    }

    /// Get the buffer usage flags.
    #[inline]
    pub fn usage(&self) -> wgpu::BufferUsages {
        self.usage
    }

    /// Get a slice of the entire buffer.
    #[inline]
    pub fn slice(&self) -> wgpu::BufferSlice<'_> {
        self.buffer.slice(..)
    }

    /// Get a slice of a portion of the buffer.
    #[inline]
    pub fn slice_range(&self, range: std::ops::Range<u32>) -> wgpu::BufferSlice<'_> {
        let start = range.start as u64 * std::mem::size_of::<T>() as u64;
        let end = range.end as u64 * std::mem::size_of::<T>() as u64;
        self.buffer.slice(start..end)
    }

    /// Write data to the buffer.
    ///
    /// The buffer must have been created with `COPY_DST` usage.
    pub fn write(&self, queue: &wgpu::Queue, data: &[T]) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
    }

    /// Write data to the buffer at an offset.
    ///
    /// The buffer must have been created with `COPY_DST` usage.
    pub fn write_at(&self, queue: &wgpu::Queue, offset: u32, data: &[T]) {
        let byte_offset = offset as u64 * std::mem::size_of::<T>() as u64;
        queue.write_buffer(&self.buffer, byte_offset, bytemuck::cast_slice(data));
    }

    /// Get the buffer as a binding resource (for bind groups).
    #[inline]
    pub fn as_binding(&self) -> wgpu::BindingResource<'_> {
        self.buffer.as_entire_binding()
    }

    /// Get a reference to the underlying buffer.
    #[inline]
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}

impl<T: bytemuck::Pod> AsWgpu for TypedBuffer<T> {
    type WgpuType = wgpu::Buffer;

    fn as_wgpu(&self) -> &Self::WgpuType {
        &self.buffer
    }
}

impl<T: bytemuck::Pod> IntoWgpu for TypedBuffer<T> {
    type WgpuType = wgpu::Buffer;

    fn into_wgpu(self) -> Self::WgpuType {
        self.buffer
    }
}

// =============================================================================
// GpuTexture
// =============================================================================

/// A GPU texture with cached view and metadata.
///
/// This wrapper provides:
/// - Automatic view creation and caching
/// - Size and format metadata
/// - Convenient accessors
pub struct GpuTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: wgpu::Extent3d,
    format: wgpu::TextureFormat,
    sample_count: u32,
}

impl GpuTexture {
    /// Create a new GPU texture.
    pub fn new(device: &wgpu::Device, descriptor: &wgpu::TextureDescriptor) -> Self {
        let texture = device.create_texture(descriptor);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            size: descriptor.size,
            format: descriptor.format,
            sample_count: descriptor.sample_count,
        }
    }

    /// Create a simple 2D texture.
    pub fn new_2d(
        device: &wgpu::Device,
        label: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> Self {
        Self::new(
            device,
            &wgpu::TextureDescriptor {
                label,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            },
        )
    }

    /// Create a texture from raw data.
    pub fn from_data(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        data: &[u8],
    ) -> Self {
        let texture = Self::new_2d(
            device,
            label,
            width,
            height,
            format,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * format.block_copy_size(None).unwrap_or(4)),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        texture
    }

    /// Get the texture view.
    #[inline]
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get the texture size.
    #[inline]
    pub fn size(&self) -> wgpu::Extent3d {
        self.size
    }

    /// Get the texture width.
    #[inline]
    pub fn width(&self) -> u32 {
        self.size.width
    }

    /// Get the texture height.
    #[inline]
    pub fn height(&self) -> u32 {
        self.size.height
    }

    /// Get the texture format.
    #[inline]
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Get the sample count.
    #[inline]
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    /// Get the texture as a binding resource.
    #[inline]
    pub fn as_binding(&self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::TextureView(&self.view)
    }

    /// Create a custom view with different parameters.
    pub fn create_view(&self, descriptor: &wgpu::TextureViewDescriptor) -> wgpu::TextureView {
        self.texture.create_view(descriptor)
    }
}

impl AsWgpu for GpuTexture {
    type WgpuType = wgpu::Texture;

    fn as_wgpu(&self) -> &Self::WgpuType {
        &self.texture
    }
}

impl IntoWgpu for GpuTexture {
    type WgpuType = wgpu::Texture;

    fn into_wgpu(self) -> Self::WgpuType {
        self.texture
    }
}

// =============================================================================
// StorageTexture
// =============================================================================

/// Access mode for storage textures in compute shaders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageTextureAccess {
    /// Read-only access in shaders.
    ReadOnly,
    /// Write-only access in shaders.
    WriteOnly,
    /// Read-write access in shaders.
    ReadWrite,
}

impl StorageTextureAccess {
    /// Convert to wgpu storage texture access.
    pub fn to_wgpu(self) -> wgpu::StorageTextureAccess {
        match self {
            Self::ReadOnly => wgpu::StorageTextureAccess::ReadOnly,
            Self::WriteOnly => wgpu::StorageTextureAccess::WriteOnly,
            Self::ReadWrite => wgpu::StorageTextureAccess::ReadWrite,
        }
    }
}

/// A texture for compute shader read/write access.
///
/// Storage textures are used when compute shaders need to write
/// to or read from textures directly.
pub struct StorageTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: wgpu::Extent3d,
    format: wgpu::TextureFormat,
    access: StorageTextureAccess,
}

impl StorageTexture {
    /// Create a new storage texture.
    pub fn new(
        device: &wgpu::Device,
        label: Option<&str>,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        access: StorageTextureAccess,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            size,
            format,
            access,
        }
    }

    /// Get the texture view.
    #[inline]
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get the texture size.
    #[inline]
    pub fn size(&self) -> wgpu::Extent3d {
        self.size
    }

    /// Get the texture width.
    #[inline]
    pub fn width(&self) -> u32 {
        self.size.width
    }

    /// Get the texture height.
    #[inline]
    pub fn height(&self) -> u32 {
        self.size.height
    }

    /// Get the texture format.
    #[inline]
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Get the access mode.
    #[inline]
    pub fn access(&self) -> StorageTextureAccess {
        self.access
    }

    /// Get the texture as a binding resource.
    #[inline]
    pub fn as_binding(&self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::TextureView(&self.view)
    }
}

impl AsWgpu for StorageTexture {
    type WgpuType = wgpu::Texture;

    fn as_wgpu(&self) -> &Self::WgpuType {
        &self.texture
    }
}

impl IntoWgpu for StorageTexture {
    type WgpuType = wgpu::Texture;

    fn into_wgpu(self) -> Self::WgpuType {
        self.texture
    }
}

// =============================================================================
// UniformBuffer (Convenience type)
// =============================================================================

/// A uniform buffer for shader uniforms.
///
/// This is a convenience type for buffers that contain uniform data.
pub type UniformBuffer<T> = TypedBuffer<T>;

impl<T: bytemuck::Pod> TypedBuffer<T> {
    /// Create a new uniform buffer.
    pub fn new_uniform(device: &wgpu::Device, label: Option<&str>, data: &T) -> Self {
        use wgpu::util::DeviceExt;

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label,
            contents: bytemuck::bytes_of(data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            buffer,
            len: 1,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            _marker: PhantomData,
        }
    }

    /// Write a single uniform value to the buffer.
    pub fn write_uniform(&self, queue: &wgpu::Queue, data: &T) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(data));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    struct TestData {
        x: f32,
        y: f32,
        z: f32,
        w: f32,
    }

    #[test]
    fn test_typed_buffer_size() {
        // Test that size calculations are correct
        assert_eq!(std::mem::size_of::<TestData>(), 16);
        assert_eq!(std::mem::size_of::<f32>(), 4);
    }

    #[test]
    fn test_storage_texture_access_conversion() {
        assert_eq!(
            StorageTextureAccess::ReadOnly.to_wgpu(),
            wgpu::StorageTextureAccess::ReadOnly
        );
        assert_eq!(
            StorageTextureAccess::WriteOnly.to_wgpu(),
            wgpu::StorageTextureAccess::WriteOnly
        );
        assert_eq!(
            StorageTextureAccess::ReadWrite.to_wgpu(),
            wgpu::StorageTextureAccess::ReadWrite
        );
    }
}
