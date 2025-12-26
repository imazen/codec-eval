//! JPEG encoder implementations.
//!
//! Supports:
//! - MozJPEG: Mozilla's optimized JPEG encoder
//! - jpegli: Google's perceptually-optimized JPEG encoder

use super::CodecImpl;
use codec_eval::eval::ImageData;
use codec_eval::eval::session::{DecodeFn, EncodeFn, EncodeRequest};
use std::io::Cursor;

// ============================================================================
// MozJPEG
// ============================================================================

#[cfg(feature = "mozjpeg")]
pub struct MozJpegCodec {
    version: String,
}

#[cfg(feature = "mozjpeg")]
impl MozJpegCodec {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[cfg(feature = "mozjpeg")]
impl Default for MozJpegCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "mozjpeg")]
impl CodecImpl for MozJpegCodec {
    fn id(&self) -> &str {
        "mozjpeg"
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn format(&self) -> &str {
        "jpg"
    }

    fn encode_fn(&self) -> EncodeFn {
        Box::new(|image: &ImageData, request: &EncodeRequest| {
            encode_mozjpeg(image, request.quality)
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(decode_jpeg)
    }
}

#[cfg(feature = "mozjpeg")]
fn encode_mozjpeg(image: &ImageData, quality: f64) -> codec_eval::error::Result<Vec<u8>> {
    use mozjpeg::{ColorSpace, Compress};

    let width = image.width();
    let height = image.height();
    let rgb_data = image.to_rgb8_vec();

    let mut comp = Compress::new(ColorSpace::JCS_RGB);
    comp.set_size(width, height);
    comp.set_quality(quality as f32);
    comp.set_optimize_scans(true);

    let mut comp =
        comp.start_compress(Vec::new())
            .map_err(|e| codec_eval::error::Error::Codec {
                codec: "mozjpeg".to_string(),
                message: format!("Failed to start compression: {}", e),
            })?;

    comp.write_scanlines(&rgb_data)
        .map_err(|e| codec_eval::error::Error::Codec {
            codec: "mozjpeg".to_string(),
            message: format!("Failed to write scanlines: {}", e),
        })?;

    comp.finish().map_err(|e| codec_eval::error::Error::Codec {
        codec: "mozjpeg".to_string(),
        message: format!("Failed to finish compression: {}", e),
    })
}

// ============================================================================
// jpegli
// ============================================================================

#[cfg(feature = "jpegli")]
pub struct JpegliCodec {
    version: String,
}

#[cfg(feature = "jpegli")]
impl JpegliCodec {
    pub fn new() -> Self {
        Self {
            version: "0.4".to_string(), // jpegli crate version
        }
    }
}

#[cfg(feature = "jpegli")]
impl Default for JpegliCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "jpegli")]
impl CodecImpl for JpegliCodec {
    fn id(&self) -> &str {
        "jpegli"
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn format(&self) -> &str {
        "jpg"
    }

    fn encode_fn(&self) -> EncodeFn {
        Box::new(|image: &ImageData, request: &EncodeRequest| encode_jpegli(image, request.quality))
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(decode_jpegli)
    }
}

#[cfg(feature = "jpegli")]
fn encode_jpegli(image: &ImageData, quality: f64) -> codec_eval::error::Result<Vec<u8>> {
    use jpegli::{ColorSpace, Compress};

    let width = image.width();
    let height = image.height();
    let rgb_data = image.to_rgb8_vec();

    std::panic::catch_unwind(|| -> std::io::Result<Vec<u8>> {
        let mut comp = Compress::new(ColorSpace::JCS_RGB);
        comp.set_size(width, height);
        comp.set_quality(quality as f32);

        let mut comp = comp.start_compress(Vec::new())?;
        comp.write_scanlines(&rgb_data)?;
        comp.finish()
    })
    .map_err(|_| codec_eval::error::Error::Codec {
        codec: "jpegli".to_string(),
        message: "Compression panicked".to_string(),
    })?
    .map_err(|e| codec_eval::error::Error::Codec {
        codec: "jpegli".to_string(),
        message: format!("Failed to encode: {}", e),
    })
}

#[cfg(feature = "jpegli")]
fn decode_jpegli(data: &[u8]) -> codec_eval::error::Result<ImageData> {
    use jpegli::Decompress;

    std::panic::catch_unwind(|| -> std::io::Result<(usize, usize, Vec<u8>)> {
        let d = Decompress::new_mem(data)?;
        let width = d.width();
        let height = d.height();
        let mut d = d.rgb()?;
        let pixels = d.read_scanlines::<rgb::RGB8>()?;
        d.finish()?;
        // Convert Vec<RGB8> to Vec<u8>
        let data: Vec<u8> = pixels.into_iter().flat_map(|p| [p.r, p.g, p.b]).collect();
        Ok((width, height, data))
    })
    .map_err(|_| codec_eval::error::Error::Codec {
        codec: "jpegli".to_string(),
        message: "Decompression panicked".to_string(),
    })?
    .map(|(width, height, data)| ImageData::RgbSlice {
        data,
        width,
        height,
    })
    .map_err(|e| codec_eval::error::Error::Codec {
        codec: "jpegli".to_string(),
        message: format!("Failed to decode: {}", e),
    })
}

// ============================================================================
// Common JPEG decoder (using image crate for mozjpeg output)
// ============================================================================

fn decode_jpeg(data: &[u8]) -> codec_eval::error::Result<ImageData> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Jpeg).map_err(|e| {
        codec_eval::error::Error::Codec {
            codec: "jpeg".to_string(),
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
// Feature stubs
// ============================================================================

#[cfg(not(feature = "mozjpeg"))]
pub struct MozJpegCodec;

#[cfg(not(feature = "mozjpeg"))]
impl MozJpegCodec {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(feature = "mozjpeg"))]
impl Default for MozJpegCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "mozjpeg"))]
impl CodecImpl for MozJpegCodec {
    fn id(&self) -> &str {
        "mozjpeg"
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
                codec: "mozjpeg".to_string(),
                message: "MozJPEG not compiled in (enable 'mozjpeg' feature)".to_string(),
            })
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(|_| {
            Err(codec_eval::error::Error::Codec {
                codec: "mozjpeg".to_string(),
                message: "MozJPEG not compiled in".to_string(),
            })
        })
    }

    fn is_available(&self) -> bool {
        false
    }
}

#[cfg(not(feature = "jpegli"))]
pub struct JpegliCodec;

#[cfg(not(feature = "jpegli"))]
impl JpegliCodec {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(feature = "jpegli"))]
impl Default for JpegliCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "jpegli"))]
impl CodecImpl for JpegliCodec {
    fn id(&self) -> &str {
        "jpegli"
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
                codec: "jpegli".to_string(),
                message: "jpegli not compiled in (enable 'jpegli' feature)".to_string(),
            })
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(|_| {
            Err(codec_eval::error::Error::Codec {
                codec: "jpegli".to_string(),
                message: "jpegli not compiled in".to_string(),
            })
        })
    }

    fn is_available(&self) -> bool {
        false
    }
}
