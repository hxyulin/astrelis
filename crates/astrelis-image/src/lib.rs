//! Optional encoded raster image decoding for Astrelis.

#![warn(missing_docs)]

use std::{error::Error, fmt};

use astrelis_core::geometry::{Physical, Size};
use astrelis_paint::Image;

/// Error returned when encoded image bytes cannot be decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageDecodeError(String);

impl fmt::Display for ImageDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for ImageDecodeError {}

/// Decodes supported encoded bytes into an immutable RGBA image.
///
/// Supported formats are selected through this crate's Cargo features.
pub fn decode_image(bytes: &[u8]) -> Result<Image, ImageDecodeError> {
    let decoded =
        image::load_from_memory(bytes).map_err(|error| ImageDecodeError(error.to_string()))?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    Image::from_rgba8(Size::<Physical, u32>::new(width, height), rgba.into_raw())
        .map_err(|error| ImageDecodeError(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "png")]
    #[test]
    fn decodes_png_into_astrelis_rgba() {
        let source = image::RgbaImage::from_pixel(2, 1, image::Rgba([4, 8, 16, 255]));
        let mut encoded = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(source)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let decoded = decode_image(encoded.get_ref()).unwrap();
        assert_eq!(decoded.size(), Size::new(2, 1));
        assert_eq!(decoded.rgba8(), &[4, 8, 16, 255, 4, 8, 16, 255]);
    }

    #[test]
    fn rejects_invalid_encoded_bytes() {
        assert!(decode_image(b"not an image").is_err());
    }
}
