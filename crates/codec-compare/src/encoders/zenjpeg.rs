//! Zenjpeg encoder implementation.
//!
//! Zenjpeg is a pure Rust JPEG encoder with perceptual optimizations,
//! based on jpegli technology.

use super::CodecImpl;
use codec_eval::eval::ImageData;
use codec_eval::eval::session::{DecodeFn, EncodeFn, EncodeRequest};

#[cfg(feature = "zenjpeg")]
pub struct ZenjpegCodec {
    version: String,
}

#[cfg(feature = "zenjpeg")]
impl ZenjpegCodec {
    pub fn new() -> Self {
        Self {
            version: "0.3".to_string(),
        }
    }

    /// Return all variants (just one for zenjpeg).
    pub fn all_variants() -> Vec<Self> {
        vec![Self::new()]
    }
}

#[cfg(feature = "zenjpeg")]
impl Default for ZenjpegCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "zenjpeg")]
impl CodecImpl for ZenjpegCodec {
    fn id(&self) -> &str {
        "zenjpeg"
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn format(&self) -> &str {
        "jpg"
    }

    fn encode_fn(&self) -> EncodeFn {
        Box::new(move |image: &ImageData, request: &EncodeRequest| {
            encode_zenjpeg(image, request.quality)
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(decode_zenjpeg)
    }
}

#[cfg(feature = "zenjpeg")]
fn encode_zenjpeg(image: &ImageData, quality: f64) -> codec_eval::error::Result<Vec<u8>> {
    use zenjpeg::encoder::{ChromaSubsampling, EncoderConfig, PixelLayout, Unstoppable};

    let width = image.width();
    let height = image.height();
    let rgb_data = image.to_rgb8_vec();

    std::panic::catch_unwind(|| {
        let config = EncoderConfig::ycbcr(quality as u8, ChromaSubsampling::Quarter)
            .progressive(true)
            .optimize_huffman(true);

        let mut encoder = config
            .encode_from_bytes(width as u32, height as u32, PixelLayout::Rgb8Srgb)
            .map_err(|e| format!("Failed to create encoder: {}", e))?;

        encoder
            .push_packed(&rgb_data, Unstoppable)
            .map_err(|e| format!("Failed to encode: {}", e))?;

        encoder
            .finish()
            .map_err(|e| format!("Failed to finish: {}", e))
    })
    .map_err(|_| codec_eval::error::Error::Codec {
        codec: "zenjpeg".to_string(),
        message: "Compression panicked".to_string(),
    })?
    .map_err(|e| codec_eval::error::Error::Codec {
        codec: "zenjpeg".to_string(),
        message: e,
    })
}

#[cfg(feature = "zenjpeg")]
fn decode_zenjpeg(data: &[u8]) -> codec_eval::error::Result<ImageData> {
    // Use the image crate for decoding (standard JPEG decoder)
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Jpeg).map_err(|e| {
        codec_eval::error::Error::Codec {
            codec: "zenjpeg".to_string(),
            message: format!("Failed to decode JPEG: {}", e),
        }
    })?;

    let rgb = img.to_rgb8();
    let (width, height) = (rgb.width() as usize, rgb.height() as usize);
    let pixels = rgb.into_raw();

    Ok(ImageData::RgbSlice {
        data: pixels,
        width,
        height,
    })
}

// ============================================================================
// Feature stub
// ============================================================================

#[cfg(not(feature = "zenjpeg"))]
pub struct ZenjpegCodec;

#[cfg(not(feature = "zenjpeg"))]
impl ZenjpegCodec {
    pub fn new() -> Self {
        Self
    }

    pub fn all_variants() -> Vec<Self> {
        vec![Self::new()]
    }
}

#[cfg(not(feature = "zenjpeg"))]
impl Default for ZenjpegCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "zenjpeg"))]
impl CodecImpl for ZenjpegCodec {
    fn id(&self) -> &str {
        "zenjpeg"
    }

    fn version(&self) -> &str {
        "unavailable"
    }

    fn format(&self) -> &str {
        "jpg"
    }

    fn encode_fn(&self) -> EncodeFn {
        Box::new(|_, _| {
            Err(codec_eval::error::Error::Codec {
                codec: "zenjpeg".to_string(),
                message: "zenjpeg not compiled in (enable 'zenjpeg' feature)".to_string(),
            })
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(|_| {
            Err(codec_eval::error::Error::Codec {
                codec: "zenjpeg".to_string(),
                message: "zenjpeg not compiled in".to_string(),
            })
        })
    }

    fn is_available(&self) -> bool {
        false
    }
}
