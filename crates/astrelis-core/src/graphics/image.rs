//! Supporing types and functions for parsing and dealing with Images
//! Currently supported formats:
//!  - QOI

/// An image represented by the QOI image format
pub struct QoiImage {}

pub enum ColorFormat {
    /// A RGBA format where each channel takes up 1 byte
    R8G8B8A8,
    /// A RGB format where each channel takes up 1 byte
    R8G8B8,
}

/// A raw, decompressed image which can be used with graphics libraries
pub struct BitmapImage {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: ColorFormat,
    pub data: Vec<u8>,
}
