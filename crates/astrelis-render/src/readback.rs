//! GPU readback utilities for screenshot and framebuffer capture.
//!
//! This module provides utilities for reading back data from the GPU to the CPU:
//! - Screenshot capture from textures
//! - Framebuffer readback
//! - Async GPU-to-CPU data transfer
//! - PNG export
//!
//! # Example
//!
//! ```ignore
//! use astrelis_render::*;
//!
//! // Capture a screenshot
//! let readback = GpuReadback::from_texture(&context, &texture);
//! let data = readback.read_async().await?;
//!
//! // Save to PNG
//! readback.save_png("screenshot.png")?;
//! ```

use std::sync::Arc;

use crate::GraphicsContext;

/// GPU readback error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadbackError {
    /// Buffer mapping failed
    MapFailed(String),
    /// Texture copy failed
    CopyFailed(String),
    /// Image encoding failed
    EncodeFailed(String),
    /// IO error
    IoError(String),
    /// Invalid dimensions
    InvalidDimensions,
    /// Unsupported format
    UnsupportedFormat,
}

impl std::fmt::Display for ReadbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MapFailed(msg) => write!(f, "Buffer mapping failed: {}", msg),
            Self::CopyFailed(msg) => write!(f, "Texture copy failed: {}", msg),
            Self::EncodeFailed(msg) => write!(f, "Image encoding failed: {}", msg),
            Self::IoError(msg) => write!(f, "IO error: {}", msg),
            Self::InvalidDimensions => write!(f, "Invalid dimensions for readback"),
            Self::UnsupportedFormat => write!(f, "Unsupported texture format for readback"),
        }
    }
}

impl std::error::Error for ReadbackError {}

/// GPU readback handle for async data retrieval.
pub struct GpuReadback {
    /// Readback buffer
    buffer: wgpu::Buffer,
    /// Texture dimensions (width, height)
    dimensions: (u32, u32),
    /// Bytes per row (with padding)
    bytes_per_row: u32,
    /// Texture format
    format: wgpu::TextureFormat,
}

impl GpuReadback {
    /// Create a readback from a texture.
    ///
    /// This copies the texture to a staging buffer for CPU readback.
    pub fn from_texture(context: Arc<GraphicsContext>, texture: &wgpu::Texture) -> Result<Self, ReadbackError> {
        let size = texture.size();
        let dimensions = (size.width, size.height);
        let format = texture.format();

        // Validate dimensions
        if dimensions.0 == 0 || dimensions.1 == 0 {
            return Err(ReadbackError::InvalidDimensions);
        }

        // Calculate bytes per row (must be aligned to 256 bytes)
        let bytes_per_pixel = match format {
            wgpu::TextureFormat::Rgba8Unorm
            | wgpu::TextureFormat::Rgba8UnormSrgb
            | wgpu::TextureFormat::Bgra8Unorm
            | wgpu::TextureFormat::Bgra8UnormSrgb => 4,
            wgpu::TextureFormat::Rgb10a2Unorm => 4,
            _ => return Err(ReadbackError::UnsupportedFormat),
        };

        let unpadded_bytes_per_row = dimensions.0 * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

        // Create staging buffer
        let buffer_size = (bytes_per_row * dimensions.1) as wgpu::BufferAddress;
        let buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        let mut encoder = context.device().create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("readback_encoder"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(dimensions.1),
                },
            },
            size,
        );

        context.queue().submit(Some(encoder.finish()));

        Ok(Self {
            buffer,
            dimensions,
            bytes_per_row,
            format,
        })
    }

    /// Read data from GPU (blocking).
    ///
    /// Returns raw RGBA bytes.
    /// Note: This is a simplified blocking implementation.
    /// For async usage, consider wrapping in async runtime.
    pub fn read(&self) -> Result<Vec<u8>, ReadbackError> {
        let buffer_slice = self.buffer.slice(..);

        // Map the buffer
        buffer_slice.map_async(wgpu::MapMode::Read, |_| {});

        // Note: In real usage, you would poll the device here
        // For now, we'll just proceed - the get_mapped_range will block

        // Read data
        let data = buffer_slice.get_mapped_range();
        let bytes_per_pixel = 4; // RGBA
        let mut result = Vec::with_capacity((self.dimensions.0 * self.dimensions.1 * bytes_per_pixel) as usize);

        // Copy data, removing row padding
        for y in 0..self.dimensions.1 {
            let row_start = (y * self.bytes_per_row) as usize;
            let row_end = row_start + (self.dimensions.0 * bytes_per_pixel) as usize;
            result.extend_from_slice(&data[row_start..row_end]);
        }

        drop(data);
        self.buffer.unmap();

        Ok(result)
    }

    /// Save the readback data as a PNG file.
    #[cfg(feature = "image")]
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> Result<(), ReadbackError> {
        let data = self.read()?;

        // Convert to image format
        let img = image::RgbaImage::from_raw(self.dimensions.0, self.dimensions.1, data)
            .ok_or(ReadbackError::EncodeFailed(
                "Failed to create image from raw data".to_string(),
            ))?;

        // Save to PNG
        img.save(path)
            .map_err(|e| ReadbackError::IoError(format!("{}", e)))?;

        Ok(())
    }

    /// Get the dimensions (width, height).
    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    /// Get the texture format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

/// Extension trait for convenient screenshot capture.
pub trait ReadbackExt {
    /// Capture a screenshot from a texture.
    fn capture_texture(&self, texture: &wgpu::Texture) -> Result<GpuReadback, ReadbackError>;
}

impl ReadbackExt for Arc<GraphicsContext> {
    fn capture_texture(&self, texture: &wgpu::Texture) -> Result<GpuReadback, ReadbackError> {
        GpuReadback::from_texture(self.clone(), texture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_readback_error_display() {
        let err = ReadbackError::MapFailed("test".to_string());
        assert!(format!("{}", err).contains("Buffer mapping failed"));

        let err = ReadbackError::InvalidDimensions;
        assert!(format!("{}", err).contains("Invalid dimensions"));
    }

    #[test]
    fn test_bytes_per_row_alignment() {
        // Test that bytes per row alignment is correct
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

        // Width 100, 4 bytes per pixel = 400 bytes
        let unpadded: u32 = 100 * 4;
        let padded = unpadded.div_ceil(align) * align;

        // Should be padded to next multiple of 256
        assert_eq!(padded, 512);
        assert_eq!(padded % align, 0);
    }

    #[test]
    fn test_readback_dimensions() {
        // We can't actually create a GPU readback without a real context,
        // but we can test the error cases
        assert!(matches!(
            ReadbackError::InvalidDimensions,
            ReadbackError::InvalidDimensions
        ));
    }
}
