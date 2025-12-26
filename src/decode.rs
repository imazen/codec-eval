//! Utility functions for decoding images with ICC profile extraction.
//!
//! This module provides helpers for decoding JPEG images while preserving
//! ICC profile information, which is critical for accurate quality metrics.
//!
//! # Example
//!
//! ```ignore
//! use codec_eval::{decode::decode_jpeg_with_icc, ImageData};
//!
//! let jpeg_data = std::fs::read("test.jpg")?;
//! let image = decode_jpeg_with_icc(&jpeg_data)?;
//!
//! // If the JPEG has an ICC profile, it will be automatically applied
//! // when computing metrics via EvalSession::evaluate_image()
//! ```

use crate::error::{Error, Result};
use crate::eval::session::ImageData;

/// Decode a JPEG image with ICC profile extraction.
///
/// This function decodes JPEG data and extracts any embedded ICC profile.
/// The result can be passed to `EvalSession::evaluate_image()` for
/// accurate quality metric calculation.
///
/// # Arguments
///
/// * `data` - JPEG-compressed image data
///
/// # Returns
///
/// `ImageData` containing the decoded pixels and ICC profile (if present).
/// Returns `ImageData::RgbSliceWithIcc` if an ICC profile was found,
/// or `ImageData::RgbSlice` if no profile was embedded.
///
/// # Errors
///
/// Returns an error if the JPEG data is invalid or decoding fails.
#[cfg(feature = "jpeg-decode")]
pub fn decode_jpeg_with_icc(data: &[u8]) -> Result<ImageData> {
    use std::io::Cursor;

    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(data));
    let pixels = decoder.decode().map_err(|e| Error::Codec {
        codec: "jpeg-decoder".to_string(),
        message: e.to_string(),
    })?;

    let info = decoder.info().ok_or_else(|| Error::Codec {
        codec: "jpeg-decoder".to_string(),
        message: "Missing JPEG info after decode".to_string(),
    })?;

    let width = info.width as usize;
    let height = info.height as usize;

    // Handle different pixel formats
    let rgb = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => pixels,
        jpeg_decoder::PixelFormat::L8 => {
            // Grayscale to RGB
            pixels.iter().flat_map(|&g| [g, g, g]).collect()
        }
        jpeg_decoder::PixelFormat::L16 => {
            // 16-bit grayscale - take high byte and convert to RGB
            pixels
                .chunks_exact(2)
                .flat_map(|c| {
                    let g = c[0]; // High byte (assuming big endian)
                    [g, g, g]
                })
                .collect()
        }
        jpeg_decoder::PixelFormat::CMYK32 => {
            return Err(Error::Codec {
                codec: "jpeg-decoder".to_string(),
                message: "CMYK JPEGs are not currently supported".to_string(),
            });
        }
    };

    // Extract ICC profile if present
    let icc_profile = decoder.icc_profile();

    Ok(match icc_profile {
        Some(icc) if !icc.is_empty() => ImageData::RgbSliceWithIcc {
            data: rgb,
            width,
            height,
            icc_profile: icc.clone(),
        },
        _ => ImageData::RgbSlice {
            data: rgb,
            width,
            height,
        },
    })
}

/// Type alias for JPEG decode callbacks.
#[cfg(feature = "jpeg-decode")]
pub type JpegDecodeCallback = Box<dyn Fn(&[u8]) -> Result<ImageData> + Send + Sync + 'static>;

/// Create an ICC-aware decode callback for use with `EvalSession::add_codec_with_decode`.
///
/// This returns a boxed callback that can be passed directly to codec registration.
///
/// # Example
///
/// ```ignore
/// use codec_eval::{EvalSession, decode::jpeg_decode_callback};
///
/// session.add_codec_with_decode(
///     "my-codec",
///     "1.0.0",
///     Box::new(my_encode_fn),
///     jpeg_decode_callback(),
/// );
/// ```
#[cfg(feature = "jpeg-decode")]
pub fn jpeg_decode_callback() -> JpegDecodeCallback {
    Box::new(decode_jpeg_with_icc)
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "jpeg-decode")]
    use super::*;

    #[test]
    #[cfg(feature = "jpeg-decode")]
    fn test_decode_jpeg_no_icc() {
        // This test would require a test JPEG file
        // For now, just verify the function exists and has correct signature
        let _: fn(&[u8]) -> Result<ImageData> = decode_jpeg_with_icc;
    }
}
