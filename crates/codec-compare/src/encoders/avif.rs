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
    /// rav1e with all imazen features (QM + VAQ + StillImage)
    #[cfg(feature = "avif-imazen")]
    Rav1eImazen,
    /// rav1e with QM only
    #[cfg(feature = "avif-imazen")]
    Rav1eQmOnly,
    /// rav1e with VAQ only
    #[cfg(feature = "avif-imazen")]
    Rav1eVaqOnly,
    /// rav1e with StillImage tuning only
    #[cfg(feature = "avif-imazen")]
    Rav1eStillOnly,
}

impl AvifEncoder {
    /// All available encoder variants.
    pub fn all() -> Vec<AvifEncoder> {
        let mut v = vec![Self::Rav1e];
        #[cfg(feature = "avif-imazen")]
        {
            v.push(Self::Rav1eImazen);
            v.push(Self::Rav1eVaqOnly);
        }
        v
    }

    /// Get the codec ID string.
    pub fn id(&self) -> &'static str {
        match self {
            Self::Rav1e => "avif-rav1e",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eImazen => "avif-rav1e-imazen",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmOnly => "avif-rav1e-qm",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eVaqOnly => "avif-rav1e-qm-vb",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eStillOnly => "avif-rav1e-still",
        }
    }

    /// Get human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rav1e => "AVIF (rav1e)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eImazen => "AVIF (rav1e-imazen all)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmOnly => "AVIF (rav1e QM only)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eVaqOnly => "AVIF (rav1e QM+VarBoost)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eStillOnly => "AVIF (rav1e StillImage only)",
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

    // When the imazen feature is compiled in, explicitly set each feature
    // per variant. Baseline rav1e disables everything to match upstream.
    #[cfg(feature = "avif-imazen")]
    {
        let (qm, vaq, vaq_str, still) = match variant {
            AvifEncoder::Rav1e => (false, false, 1.0, false),
            AvifEncoder::Rav1eImazen => (true, false, 1.0, false),
            AvifEncoder::Rav1eQmOnly => (true, false, 1.0, false),
            AvifEncoder::Rav1eVaqOnly => (true, true, 0.3, false),
            AvifEncoder::Rav1eStillOnly => (false, false, 1.0, true),
        };
        encoder = encoder
            .with_qm(qm)
            .with_vaq(vaq, vaq_str)
            .with_still_image_tuning(still);
    }
    let _ = variant;

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
    let decoded = zenavif::decode(data).map_err(|e| {
        codec_eval::error::Error::Codec {
            codec: "avif".to_string(),
            message: format!("Decoding failed: {}", e),
        }
    })?;

    let (width, height, pixels) = match decoded {
        zenavif::PixelData::Rgb8(img) => {
            let w = img.width();
            let h = img.height();
            let mut rgb_data = Vec::with_capacity(w * h * 3);
            for pixel in img.pixels() {
                rgb_data.push(pixel.r);
                rgb_data.push(pixel.g);
                rgb_data.push(pixel.b);
            }
            (w, h, rgb_data)
        }
        zenavif::PixelData::Rgba8(img) => {
            let w = img.width();
            let h = img.height();
            let mut rgb_data = Vec::with_capacity(w * h * 3);
            for pixel in img.pixels() {
                rgb_data.push(pixel.r);
                rgb_data.push(pixel.g);
                rgb_data.push(pixel.b);
            }
            (w, h, rgb_data)
        }
        zenavif::PixelData::Rgb16(img) => {
            // 10/12-bit decoded as u16, scale down to 8-bit
            let w = img.width();
            let h = img.height();
            let mut rgb_data = Vec::with_capacity(w * h * 3);
            for pixel in img.pixels() {
                // Assume 10-bit range (0-1023), scale to 0-255
                rgb_data.push((pixel.r >> 2).min(255) as u8);
                rgb_data.push((pixel.g >> 2).min(255) as u8);
                rgb_data.push((pixel.b >> 2).min(255) as u8);
            }
            (w, h, rgb_data)
        }
        zenavif::PixelData::Rgba16(img) => {
            let w = img.width();
            let h = img.height();
            let mut rgb_data = Vec::with_capacity(w * h * 3);
            for pixel in img.pixels() {
                rgb_data.push((pixel.r >> 2).min(255) as u8);
                rgb_data.push((pixel.g >> 2).min(255) as u8);
                rgb_data.push((pixel.b >> 2).min(255) as u8);
            }
            (w, h, rgb_data)
        }
        other => {
            return Err(codec_eval::error::Error::Codec {
                codec: "avif".to_string(),
                message: format!(
                    "Unsupported decoded format: {:?}",
                    std::mem::discriminant(&other)
                ),
            });
        }
    };

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
