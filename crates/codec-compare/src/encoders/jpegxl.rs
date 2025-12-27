//! JPEG XL encoder implementation.
//!
//! Uses jpegxl-rs (libjxl bindings) for encoding/decoding.

use super::CodecImpl;
use codec_eval::eval::ImageData;
use codec_eval::eval::session::{DecodeFn, EncodeFn, EncodeRequest};

// ============================================================================
// JPEG XL Codec
// ============================================================================

#[cfg(feature = "jpegxl")]
pub struct JpegxlCodec {
    version: String,
    /// Encoder speed (0-9, higher = faster but lower quality)
    speed: u8,
}

#[cfg(feature = "jpegxl")]
impl JpegxlCodec {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            speed: 7, // Squirrel (default)
        }
    }

    /// Set encoder speed (0-9).
    /// 0 = slowest/best, 9 = fastest/worst.
    pub fn with_speed(mut self, speed: u8) -> Self {
        self.speed = speed.min(9);
        self
    }
}

#[cfg(feature = "jpegxl")]
impl Default for JpegxlCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "jpegxl")]
impl CodecImpl for JpegxlCodec {
    fn id(&self) -> &str {
        "jpegxl"
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn format(&self) -> &str {
        "jxl"
    }

    fn encode_fn(&self) -> EncodeFn {
        let speed = self.speed;
        Box::new(move |image: &ImageData, request: &EncodeRequest| {
            encode_jpegxl(image, request.quality, speed)
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(decode_jpegxl)
    }
}

#[cfg(feature = "jpegxl")]
fn encode_jpegxl(image: &ImageData, quality: f64, speed: u8) -> codec_eval::error::Result<Vec<u8>> {
    use jpegxl_rs::encode::EncoderSpeed;
    use jpegxl_rs::encoder_builder;

    let width = image.width() as u32;
    let height = image.height() as u32;
    let rgb_data = image.to_rgb8_vec();

    // Map speed 0-9 to EncoderSpeed enum
    let encoder_speed = match speed {
        0 => EncoderSpeed::Tortoise,
        1 => EncoderSpeed::Kitten,
        2 => EncoderSpeed::Kitten,
        3 => EncoderSpeed::Wombat,
        4 => EncoderSpeed::Wombat,
        5 => EncoderSpeed::Squirrel,
        6 => EncoderSpeed::Squirrel,
        7 => EncoderSpeed::Squirrel,
        8 => EncoderSpeed::Cheetah,
        _ => EncoderSpeed::Lightning,
    };

    // Create encoder with JPEG-style quality mapping
    // jpegxl-rs has a jpeg_quality() method that uses JxlEncoderDistanceFromQuality
    let mut encoder = encoder_builder()
        .speed(encoder_speed)
        .jpeg_quality(quality as f32)
        .build()
        .map_err(|e| codec_eval::error::Error::Codec {
            codec: "jpegxl".to_string(),
            message: format!("Failed to create encoder: {e}"),
        })?;

    let result: jpegxl_rs::encode::EncoderResult<u8> = encoder
        .encode(&rgb_data, width, height)
        .map_err(|e| codec_eval::error::Error::Codec {
            codec: "jpegxl".to_string(),
            message: format!("Failed to encode: {e}"),
        })?;

    Ok(result.data)
}

#[cfg(feature = "jpegxl")]
fn decode_jpegxl(data: &[u8]) -> codec_eval::error::Result<ImageData> {
    use jpegxl_rs::decode::PixelFormat;
    use jpegxl_rs::decoder_builder;

    let decoder = decoder_builder()
        .pixel_format(PixelFormat {
            num_channels: 3, // RGB output
            ..Default::default()
        })
        .build()
        .map_err(|e| codec_eval::error::Error::Codec {
            codec: "jpegxl".to_string(),
            message: format!("Failed to create decoder: {e}"),
        })?;

    let (metadata, pixels) =
        decoder
            .decode_with::<u8>(data)
            .map_err(|e| codec_eval::error::Error::Codec {
                codec: "jpegxl".to_string(),
                message: format!("Failed to decode: {e}"),
            })?;

    Ok(ImageData::RgbSlice {
        data: pixels,
        width: metadata.width as usize,
        height: metadata.height as usize,
    })
}

// ============================================================================
// Feature stub
// ============================================================================

#[cfg(not(feature = "jpegxl"))]
pub struct JpegxlCodec;

#[cfg(not(feature = "jpegxl"))]
impl JpegxlCodec {
    pub fn new() -> Self {
        Self
    }

    pub fn with_speed(self, _speed: u8) -> Self {
        self
    }
}

#[cfg(not(feature = "jpegxl"))]
impl Default for JpegxlCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "jpegxl"))]
impl CodecImpl for JpegxlCodec {
    fn id(&self) -> &str {
        "jpegxl"
    }

    fn version(&self) -> &str {
        "unavailable"
    }

    fn format(&self) -> &str {
        "jxl"
    }

    fn encode_fn(&self) -> EncodeFn {
        Box::new(|_, _| {
            Err(codec_eval::error::Error::Codec {
                codec: "jpegxl".to_string(),
                message: "JPEG XL not compiled in (enable 'jpegxl' feature)".to_string(),
            })
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(|_| {
            Err(codec_eval::error::Error::Codec {
                codec: "jpegxl".to_string(),
                message: "JPEG XL not compiled in".to_string(),
            })
        })
    }

    fn is_available(&self) -> bool {
        false
    }
}
