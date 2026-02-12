//! AVIF encoder implementations.
//!
//! Uses ravif (pure Rust) for AVIF encoding with rav1e.
//! For other encoders (AOM, SVT-AV1), use Docker mode.

use super::CodecImpl;
use codec_eval::eval::session::{DecodeFn, EncodeFn};

#[cfg(feature = "ravif")]
use codec_eval::eval::ImageData;
#[cfg(feature = "ravif")]
use codec_eval::eval::session::EncodeRequest;

/// AVIF encoder selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AvifEncoder {
    /// rav1e - Pure Rust AV1 encoder (default for native builds)
    Rav1e,
    /// rav1e with imazen fork enhancements (QM, VAQ, StillImage tuning)
    #[cfg(feature = "avif-imazen")]
    Rav1eImazen,
}

impl AvifEncoder {
    /// All available encoder variants.
    pub fn all() -> Vec<AvifEncoder> {
        let mut v = vec![Self::Rav1e];
        #[cfg(feature = "avif-imazen")]
        v.push(Self::Rav1eImazen);
        v
    }

    /// Get the codec ID string.
    pub fn id(&self) -> &'static str {
        match self {
            Self::Rav1e => "avif-rav1e",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eImazen => "avif-rav1e-imazen",
        }
    }

    /// Get human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rav1e => "AVIF (rav1e)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eImazen => "AVIF (rav1e-imazen)",
        }
    }
}

// ============================================================================
// AVIF Codec (using ravif - pure Rust)
// ============================================================================

#[cfg(feature = "ravif")]
pub struct AvifCodec {
    encoder: AvifEncoder,
    version: String,
    speed: u8,
}

#[cfg(feature = "ravif")]
impl AvifCodec {
    /// Create a new AVIF codec.
    pub fn new(encoder: AvifEncoder) -> Self {
        Self {
            encoder,
            version: env!("CARGO_PKG_VERSION").to_string(),
            speed: 6,
        }
    }

    /// Set the speed/effort tradeoff (1-10, lower = slower/better).
    pub fn with_speed(mut self, speed: u8) -> Self {
        self.speed = speed.clamp(1, 10);
        self
    }

    /// Create all AVIF encoder variants.
    pub fn all() -> Vec<Self> {
        AvifEncoder::all().iter().map(|&e| Self::new(e)).collect()
    }
}

#[cfg(feature = "ravif")]
impl CodecImpl for AvifCodec {
    fn id(&self) -> &str {
        self.encoder.id()
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn format(&self) -> &str {
        "avif"
    }

    fn encode_fn(&self) -> EncodeFn {
        let speed = self.speed;
        let encoder = self.encoder;

        Box::new(move |image: &ImageData, request: &EncodeRequest| {
            encode_avif_ravif(image, request.quality, speed, encoder)
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(decode_avif)
    }
}

#[cfg(feature = "ravif")]
fn encode_avif_ravif(
    image: &ImageData,
    quality: f64,
    speed: u8,
    variant: AvifEncoder,
) -> codec_eval::error::Result<Vec<u8>> {
    use ravif::{Encoder, Img};
    use rgb::RGBA8;

    let width = image.width();
    let height = image.height();
    let rgb_data = image.to_rgb8_vec();

    // Convert RGB to RGBA (ravif requires RGBA)
    let rgba_data: Vec<RGBA8> = rgb_data
        .chunks_exact(3)
        .map(|c| RGBA8::new(c[0], c[1], c[2], 255))
        .collect();

    let img = Img::new(rgba_data.as_slice(), width, height);

    // ravif quality is 0-100 where 100 is best
    let mut encoder = Encoder::new()
        .with_quality(quality as f32)
        .with_speed(speed);

    // Apply imazen fork enhancements when using the imazen variant
    #[cfg(feature = "avif-imazen")]
    if matches!(variant, AvifEncoder::Rav1eImazen) {
        encoder = encoder
            .with_qm(true)
            .with_vaq(true, 0.5)
            .with_still_image_tuning(true);
    }
    let _ = variant; // suppress unused warning when avif-imazen not enabled

    let result = encoder
        .encode_rgba(img)
        .map_err(|e| codec_eval::error::Error::Codec {
            codec: "avif-rav1e".to_string(),
            message: format!("Encoding failed: {}", e),
        })?;

    Ok(result.avif_file)
}

#[cfg(feature = "ravif")]
fn decode_avif(data: &[u8]) -> codec_eval::error::Result<ImageData> {
    // Use image crate for decoding AVIF
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Avif).map_err(|e| {
        codec_eval::error::Error::Codec {
            codec: "avif".to_string(),
            message: format!("Decoding failed: {}", e),
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

#[cfg(not(feature = "ravif"))]
pub struct AvifCodec {
    encoder: AvifEncoder,
}

#[cfg(not(feature = "ravif"))]
impl AvifCodec {
    pub fn new(encoder: AvifEncoder) -> Self {
        Self { encoder }
    }

    pub fn with_speed(self, _speed: u8) -> Self {
        self
    }

    pub fn all() -> Vec<Self> {
        AvifEncoder::all().into_iter().map(|e| Self::new(e)).collect()
    }
}

#[cfg(not(feature = "ravif"))]
impl CodecImpl for AvifCodec {
    fn id(&self) -> &str {
        self.encoder.id()
    }

    fn version(&self) -> &str {
        "unavailable"
    }

    fn format(&self) -> &str {
        "avif"
    }

    fn encode_fn(&self) -> EncodeFn {
        let id = self.encoder.id().to_string();
        Box::new(move |_, _| {
            Err(codec_eval::error::Error::Codec {
                codec: id.clone(),
                message: "AVIF not compiled in (enable 'avif' feature)".to_string(),
            })
        })
    }

    fn decode_fn(&self) -> DecodeFn {
        Box::new(|_| {
            Err(codec_eval::error::Error::Codec {
                codec: "avif".to_string(),
                message: "AVIF not compiled in".to_string(),
            })
        })
    }

    fn is_available(&self) -> bool {
        false
    }
}
