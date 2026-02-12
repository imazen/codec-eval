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
    /// rav1e with QM only (best single feature: -10% BD-Rate)
    #[cfg(feature = "avif-imazen")]
    Rav1eImazen,
    /// QM + CDEF forced on at all quality levels
    #[cfg(feature = "avif-imazen")]
    Rav1eQmCdef,
    /// QM + rdo_tx_decision forced on
    #[cfg(feature = "avif-imazen")]
    Rav1eQmRdoTx,
    /// QM + VAQ strength 1.5 (expanded SSIM boost range)
    #[cfg(feature = "avif-imazen")]
    Rav1eQmVaq15,
    /// QM + CDEF + rdo_tx_decision (both forced on)
    #[cfg(feature = "avif-imazen")]
    Rav1eQmCdefRdoTx,
    /// QM + separated segmentation boost 1.25
    #[cfg(feature = "avif-imazen")]
    Rav1eQmSeg125,
    /// QM + separated segmentation boost 1.5
    #[cfg(feature = "avif-imazen")]
    Rav1eQmSeg150,
    /// QM + separated segmentation boost 2.0
    #[cfg(feature = "avif-imazen")]
    Rav1eQmSegBoost,
    /// QM + RdoTx + SegBoost 2.0
    #[cfg(feature = "avif-imazen")]
    Rav1eQmRdoTxSegBoost,
}

impl AvifEncoder {
    /// All available encoder variants.
    pub fn all() -> Vec<AvifEncoder> {
        let mut v = vec![Self::Rav1e];
        #[cfg(feature = "avif-imazen")]
        {
            v.push(Self::Rav1eImazen);
            v.push(Self::Rav1eQmRdoTx);
            v.push(Self::Rav1eQmSeg125);
            v.push(Self::Rav1eQmSeg150);
            v.push(Self::Rav1eQmSegBoost);
            v.push(Self::Rav1eQmRdoTxSegBoost);
        }
        v
    }

    /// Get the codec ID string.
    pub fn id(&self) -> &'static str {
        match self {
            Self::Rav1e => "avif-rav1e",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eImazen => "avif-rav1e-qm",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmCdef => "avif-rav1e-qm-cdef",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmRdoTx => "avif-rav1e-qm-rdotx",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmVaq15 => "avif-rav1e-qm-vaq15",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmCdefRdoTx => "avif-rav1e-qm-cdef-rdotx",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmSeg125 => "avif-rav1e-qm-seg125",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmSeg150 => "avif-rav1e-qm-seg150",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmSegBoost => "avif-rav1e-qm-seg2",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmRdoTxSegBoost => "avif-rav1e-qm-rdotx-seg2",
        }
    }

    /// Get human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rav1e => "AVIF (rav1e)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eImazen => "AVIF (rav1e QM)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmCdef => "AVIF (QM+CDEF)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmRdoTx => "AVIF (QM+RdoTx)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmVaq15 => "AVIF (QM+VAQ1.5)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmCdefRdoTx => "AVIF (QM+CDEF+RdoTx)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmSeg125 => "AVIF (QM+Seg1.25)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmSeg150 => "AVIF (QM+Seg1.5)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmSegBoost => "AVIF (QM+Seg2.0)",
            #[cfg(feature = "avif-imazen")]
            Self::Rav1eQmRdoTxSegBoost => "AVIF (QM+RdoTx+Seg2.0)",
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

    // When the imazen feature is compiled in, configure each variant.
    // Baseline rav1e disables everything to match upstream.
    #[cfg(feature = "avif-imazen")]
    {
        // (qm, vaq, vaq_str, cdef_override, rdo_tx_override, seg_boost)
        let (qm, vaq, vaq_str, cdef, rdo_tx, seg_boost) = match variant {
            AvifEncoder::Rav1e => (false, false, 1.0, None, None, 1.0),
            AvifEncoder::Rav1eImazen => (true, false, 1.0, None, None, 1.0),
            AvifEncoder::Rav1eQmCdef => (true, false, 1.0, Some(true), None, 1.0),
            AvifEncoder::Rav1eQmRdoTx => (true, false, 1.0, None, Some(true), 1.0),
            AvifEncoder::Rav1eQmVaq15 => (true, true, 1.5, None, None, 1.0),
            AvifEncoder::Rav1eQmCdefRdoTx => (true, false, 1.0, Some(true), Some(true), 1.0),
            AvifEncoder::Rav1eQmSeg125 => (true, false, 1.0, None, None, 1.25),
            AvifEncoder::Rav1eQmSeg150 => (true, false, 1.0, None, None, 1.5),
            AvifEncoder::Rav1eQmSegBoost => (true, false, 1.0, None, None, 2.0),
            AvifEncoder::Rav1eQmRdoTxSegBoost => (true, false, 1.0, None, Some(true), 2.0),
        };
        encoder = encoder
            .with_qm(qm)
            .with_vaq(vaq, vaq_str)
            .with_still_image_tuning(false)
            .with_cdef(cdef)
            .with_rdo_tx_decision(rdo_tx)
            .with_seg_boost(seg_boost);
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
