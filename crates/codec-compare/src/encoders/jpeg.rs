//! JPEG encoder implementations.
//!
//! Supports:
//! - MozJPEG: Mozilla's optimized JPEG encoder
//! - jpegli: Google's perceptually-optimized JPEG encoder
//!
//! Each encoder supports multiple variants:
//! - Subsampling: 4:4:4 (no chroma subsampling) or 4:2:0 (default)
//! - Mode: Progressive (optimized scan) or Baseline

use super::CodecImpl;
use codec_eval::eval::ImageData;
use codec_eval::eval::session::{DecodeFn, EncodeFn, EncodeRequest};

/// Chroma subsampling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JpegSubsampling {
    /// 4:4:4 - No chroma subsampling (highest quality, larger files)
    S444,
    /// 4:2:0 - Standard chroma subsampling (good balance)
    #[default]
    S420,
}

impl JpegSubsampling {
    pub fn suffix(&self) -> &'static str {
        match self {
            Self::S444 => "-444",
            Self::S420 => "-420",
        }
    }
}

/// JPEG encoding mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JpegMode {
    /// Progressive encoding with optimized scans
    #[default]
    Progressive,
    /// Baseline (sequential) encoding
    Baseline,
}

impl JpegMode {
    pub fn suffix(&self) -> &'static str {
        match self {
            Self::Progressive => "-prog",
            Self::Baseline => "-base",
        }
    }
}

// ============================================================================
// MozJPEG
// ============================================================================

#[cfg(feature = "mozjpeg")]
pub struct MozJpegCodec {
    version: String,
    subsampling: JpegSubsampling,
    mode: JpegMode,
    id: String,
}

#[cfg(feature = "mozjpeg")]
impl MozJpegCodec {
    pub fn new() -> Self {
        Self::with_config(JpegSubsampling::default(), JpegMode::default())
    }

    pub fn with_config(subsampling: JpegSubsampling, mode: JpegMode) -> Self {
        let id = format!("mozjpeg{}{}", subsampling.suffix(), mode.suffix());
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            subsampling,
            mode,
            id,
        }
    }

    /// Create all variants of MozJPEG codec.
    pub fn all_variants() -> Vec<Self> {
        vec![
            Self::with_config(JpegSubsampling::S420, JpegMode::Progressive),
            Self::with_config(JpegSubsampling::S444, JpegMode::Progressive),
            Self::with_config(JpegSubsampling::S420, JpegMode::Baseline),
            Self::with_config(JpegSubsampling::S444, JpegMode::Baseline),
        ]
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
        &self.id
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn format(&self) -> &str {
        "jpg"
    }

    fn encode_fn(&self) -> EncodeFn {
        let subsampling = self.subsampling;
        let mode = self.mode;
        Box::new(move |image: &ImageData, request: &EncodeRequest| {
            encode_mozjpeg(image, request.quality, subsampling, mode)
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(decode_jpeg)
    }
}

#[cfg(feature = "mozjpeg")]
fn encode_mozjpeg(
    image: &ImageData,
    quality: f64,
    subsampling: JpegSubsampling,
    mode: JpegMode,
) -> codec_eval::error::Result<Vec<u8>> {
    use mozjpeg::{ColorSpace, Compress};

    let width = image.width();
    let height = image.height();
    let rgb_data = image.to_rgb8_vec();

    let mut comp = Compress::new(ColorSpace::JCS_RGB);
    comp.set_size(width, height);
    comp.set_quality(quality as f32);

    // Set subsampling before starting compression
    match subsampling {
        JpegSubsampling::S444 => {
            comp.set_chroma_sampling_pixel_sizes((1, 1), (1, 1));
        }
        JpegSubsampling::S420 => {
            comp.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));
        }
    }

    // Enable Huffman optimization (2-pass for optimal tables)
    comp.set_optimize_coding(true);

    // Set progressive mode
    match mode {
        JpegMode::Progressive => {
            comp.set_progressive_mode();
            comp.set_optimize_scans(true);
        }
        JpegMode::Baseline => {
            // Default is baseline
        }
    }

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
    subsampling: JpegSubsampling,
    mode: JpegMode,
    id: String,
}

#[cfg(feature = "jpegli")]
impl JpegliCodec {
    pub fn new() -> Self {
        Self::with_config(JpegSubsampling::default(), JpegMode::default())
    }

    pub fn with_config(subsampling: JpegSubsampling, mode: JpegMode) -> Self {
        let id = format!("jpegli{}{}", subsampling.suffix(), mode.suffix());
        Self {
            version: "0.12".to_string(), // jpegli-rs crate version
            subsampling,
            mode,
            id,
        }
    }

    /// Create all variants of jpegli codec.
    pub fn all_variants() -> Vec<Self> {
        vec![
            Self::with_config(JpegSubsampling::S420, JpegMode::Progressive),
            Self::with_config(JpegSubsampling::S444, JpegMode::Progressive),
            Self::with_config(JpegSubsampling::S420, JpegMode::Baseline),
            Self::with_config(JpegSubsampling::S444, JpegMode::Baseline),
        ]
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
        &self.id
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn format(&self) -> &str {
        "jpg"
    }

    fn encode_fn(&self) -> EncodeFn {
        let subsampling = self.subsampling;
        let mode = self.mode;
        Box::new(move |image: &ImageData, request: &EncodeRequest| {
            encode_jpegli(image, request.quality, subsampling, mode)
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(decode_jpegli)
    }
}

#[cfg(feature = "jpegli")]
fn encode_jpegli(
    image: &ImageData,
    quality: f64,
    subsampling: JpegSubsampling,
    mode: JpegMode,
) -> codec_eval::error::Result<Vec<u8>> {
    use jpegli::encoder::{ChromaSubsampling, EncoderConfig, PixelLayout, Unstoppable};

    let width = image.width();
    let height = image.height();
    let rgb_data = image.to_rgb8_vec();

    // Map our subsampling enum to jpegli's
    let chroma = match subsampling {
        JpegSubsampling::S444 => ChromaSubsampling::None,
        JpegSubsampling::S420 => ChromaSubsampling::Quarter,
    };

    // Build encoder config
    let config = EncoderConfig::ycbcr(quality as u8, chroma)
        .progressive(mode == JpegMode::Progressive)
        .optimize_huffman(true);

    std::panic::catch_unwind(|| {
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
        codec: "jpegli".to_string(),
        message: "Compression panicked".to_string(),
    })?
    .map_err(|e| codec_eval::error::Error::Codec {
        codec: "jpegli".to_string(),
        message: e,
    })
}

#[cfg(feature = "jpegli")]
fn decode_jpegli(data: &[u8]) -> codec_eval::error::Result<ImageData> {
    // jpegli decoder is in prerelease, use image crate for now
    decode_jpeg(data)
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
pub struct MozJpegCodec {
    id: String,
}

#[cfg(not(feature = "mozjpeg"))]
impl MozJpegCodec {
    pub fn new() -> Self {
        Self {
            id: "mozjpeg-420-prog".to_string(),
        }
    }

    pub fn with_config(_subsampling: JpegSubsampling, _mode: JpegMode) -> Self {
        Self::new()
    }

    pub fn all_variants() -> Vec<Self> {
        vec![Self::new()]
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
        &self.id
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
pub struct JpegliCodec {
    id: String,
}

#[cfg(not(feature = "jpegli"))]
impl JpegliCodec {
    pub fn new() -> Self {
        Self {
            id: "jpegli-420-prog".to_string(),
        }
    }

    pub fn with_config(_subsampling: JpegSubsampling, _mode: JpegMode) -> Self {
        Self::new()
    }

    pub fn all_variants() -> Vec<Self> {
        vec![Self::new()]
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
        &self.id
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
