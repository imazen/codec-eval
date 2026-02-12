//! AVIF codec configuration and presets for codec-iter.
//!
//! Presets map to tested combinations from the imazen/rav1e benchmark results:
//! - `baseline`: stock ravif, no imazen features
//! - `qm`: quantization matrices only (~10% BD-rate, ~1x time)
//! - `qm-rdotx`: QM + `rdo_tx_decision` (~10.3% BD-rate, ~3x time)
//! - `qm-cdef-rdotx`: QM + CDEF + `rdo_tx_decision` (~10.7% BD-rate, ~3.5x time)

use anyhow::Result;
use imgref::ImgVec;
use ravif::{BitDepth, Encoder, Img, RGBA8};
use rgb::RGB;

use crate::eval::Codec;
use crate::source::Rgb8;

/// AVIF encoder configuration.
pub struct AvifConfig {
    pub speed: u8,
    pub enable_qm: bool,
    pub rdo_tx: Option<bool>,
    pub cdef: Option<bool>,
    pub bit_depth_8: bool,
}

impl AvifConfig {
    /// Create config from a named preset.
    pub fn from_preset(name: &str) -> Result<Self> {
        match name {
            "baseline" => Ok(Self {
                speed: 6,
                enable_qm: false,
                rdo_tx: None,
                cdef: None,
                bit_depth_8: false,
            }),
            "qm" => Ok(Self {
                speed: 6,
                enable_qm: true,
                rdo_tx: None,
                cdef: None,
                bit_depth_8: false,
            }),
            "qm-rdotx" => Ok(Self {
                speed: 6,
                enable_qm: true,
                rdo_tx: Some(true),
                cdef: None,
                bit_depth_8: false,
            }),
            "qm-cdef-rdotx" => Ok(Self {
                speed: 6,
                enable_qm: true,
                rdo_tx: Some(true),
                cdef: Some(true),
                bit_depth_8: false,
            }),
            other => anyhow::bail!(
                "Unknown AVIF preset: '{other}'. Available: baseline, qm, qm-rdotx, qm-cdef-rdotx"
            ),
        }
    }
}

fn avif_config_summary(config: &AvifConfig) -> String {
    let depth = if config.bit_depth_8 { "8bit" } else { "10bit" };
    let mut features = Vec::new();
    if config.enable_qm {
        features.push("qm");
    }
    if config.rdo_tx == Some(true) {
        features.push("rdotx");
    }
    if config.cdef == Some(true) {
        features.push("cdef");
    }
    let feat_str = if features.is_empty() {
        "stock".to_string()
    } else {
        features.join("+")
    };
    format!("ravif-s{}-{}-{}", config.speed, depth, feat_str)
}

/// Convert RGB8 pixels to RGBA8 for ravif (which requires RGBA input).
fn rgb8_to_rgba8(img: imgref::ImgRef<'_, Rgb8>) -> ImgVec<RGBA8> {
    let pixels: Vec<RGBA8> = img
        .pixels()
        .map(|p| RGBA8 {
            r: p.r,
            g: p.g,
            b: p.b,
            a: 255,
        })
        .collect();
    ImgVec::new(pixels, img.width(), img.height())
}

/// Convert decoded `PixelData` to `ImgVec<Rgb8>`, handling common variants.
fn pixel_data_to_rgb8(pd: zenavif::PixelData) -> Result<ImgVec<Rgb8>> {
    match pd {
        zenavif::PixelData::Rgb8(img) => Ok(img),
        zenavif::PixelData::Rgba8(img) => {
            // Drop alpha channel
            let pixels: Vec<Rgb8> = img
                .pixels()
                .map(|p| RGB {
                    r: p.r,
                    g: p.g,
                    b: p.b,
                })
                .collect();
            Ok(ImgVec::new(pixels, img.width(), img.height()))
        }
        other => anyhow::bail!(
            "Unexpected AVIF decode output: expected Rgb8 or Rgba8, got {}x{} {:?}",
            other.width(),
            other.height(),
            std::mem::discriminant(&other),
        ),
    }
}

/// Build a Codec for AVIF encoding/decoding with the given config.
///
/// Uses ravif directly for encoding (with imazen feature knobs) and
/// zenavif for decoding.
pub fn build_avif_codec(config: &AvifConfig) -> Codec {
    let summary = avif_config_summary(config);
    let speed = config.speed;
    let enable_qm = config.enable_qm;
    let rdo_tx = config.rdo_tx;
    let cdef = config.cdef;
    let bit_depth_8 = config.bit_depth_8;

    Codec {
        encode: Box::new(move |img, quality| {
            let rgba = rgb8_to_rgba8(img);
            #[allow(unused_mut)]
            let mut enc = Encoder::new()
                .with_quality(f32::from(quality))
                .with_speed(speed)
                .with_bit_depth(if bit_depth_8 {
                    BitDepth::Eight
                } else {
                    BitDepth::Auto
                });

            #[cfg(feature = "avif-imazen")]
            {
                enc = enc.with_qm(enable_qm);
                if let Some(rdo) = rdo_tx {
                    enc = enc.with_rdo_tx_decision(Some(rdo));
                }
                if let Some(cdef_on) = cdef {
                    enc = enc.with_cdef(Some(cdef_on));
                }
            }
            #[cfg(not(feature = "avif-imazen"))]
            {
                let _ = (enable_qm, rdo_tx, cdef);
            }

            let result = enc
                .encode_rgba(Img::new(rgba.buf().as_slice(), rgba.width(), rgba.height()))
                .map_err(|e| anyhow::anyhow!("AVIF encode: {e}"))?;

            Ok(result.avif_file)
        }),
        decode: Box::new(|data| {
            let pd = zenavif::decode(data).map_err(|e| anyhow::anyhow!("AVIF decode: {e}"))?;
            pixel_data_to_rgb8(pd)
        }),
        summary,
    }
}
