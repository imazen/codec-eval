//! WebP encoder implementation.
//!
//! Uses libwebp for encoding/decoding.

use super::CodecImpl;
use codec_eval::eval::ImageData;
use codec_eval::eval::session::{DecodeFn, EncodeFn, EncodeRequest};

// ============================================================================
// WebP Codec
// ============================================================================

#[cfg(feature = "webp")]
pub struct WebPCodec {
    version: String,
}

#[cfg(feature = "webp")]
impl WebPCodec {
    pub fn new() -> Self {
        Self {
            version: "1.4".to_string(), // libwebp version
        }
    }
}

#[cfg(feature = "webp")]
impl Default for WebPCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "webp")]
impl CodecImpl for WebPCodec {
    fn id(&self) -> &str {
        "webp"
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn format(&self) -> &str {
        "webp"
    }

    fn encode_fn(&self) -> EncodeFn {
        Box::new(|image: &ImageData, request: &EncodeRequest| encode_webp(image, request.quality))
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(decode_webp)
    }
}

#[cfg(feature = "webp")]
fn encode_webp(image: &ImageData, quality: f64) -> codec_eval::error::Result<Vec<u8>> {
    use webp::Encoder;

    let width = image.width() as u32;
    let height = image.height() as u32;
    let rgb_data = image.to_rgb8_vec();

    let encoder = Encoder::from_rgb(&rgb_data, width, height);
    let webp_data = encoder.encode(quality as f32);

    Ok(webp_data.to_vec())
}

#[cfg(feature = "webp")]
fn decode_webp(data: &[u8]) -> codec_eval::error::Result<ImageData> {
    use webp::Decoder;

    let decoder = Decoder::new(data);
    let webp_image = decoder
        .decode()
        .ok_or_else(|| codec_eval::error::Error::Codec {
            codec: "webp".to_string(),
            message: "Failed to decode WebP".to_string(),
        })?;

    let rgb = webp_image.to_image().to_rgb8();
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

#[cfg(not(feature = "webp"))]
pub struct WebPCodec;

#[cfg(not(feature = "webp"))]
impl WebPCodec {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(feature = "webp"))]
impl Default for WebPCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "webp"))]
impl CodecImpl for WebPCodec {
    fn id(&self) -> &str {
        "webp"
    }

    fn version(&self) -> &str {
        "unavailable"
    }

    fn format(&self) -> &str {
        "webp"
    }

    fn encode_fn(&self) -> EncodeFn {
        Box::new(|_, _| {
            Err(codec_eval::error::Error::Codec {
                codec: "webp".to_string(),
                message: "WebP not compiled in (enable 'webp' feature)".to_string(),
            })
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(|_| {
            Err(codec_eval::error::Error::Codec {
                codec: "webp".to_string(),
                message: "WebP not compiled in".to_string(),
            })
        })
    }

    fn is_available(&self) -> bool {
        false
    }
}
