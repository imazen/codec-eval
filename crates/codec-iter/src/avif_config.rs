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
    pub sgr_full: Option<bool>,
    pub lru_on_skip: Option<bool>,
    pub segmentation_complex: Option<bool>,
    pub encode_bottomup: Option<bool>,
    pub enable_trellis: bool,
    pub bit_depth_8: bool,
}

impl AvifConfig {
    /// Create config from a named preset.
    pub fn from_preset(name: &str) -> Result<Self> {
        let base = Self {
            speed: 6,
            enable_qm: true,
            rdo_tx: None,
            cdef: None,
            sgr_full: None,
            lru_on_skip: None,
            segmentation_complex: None,
            encode_bottomup: None,
            enable_trellis: false,
            bit_depth_8: false,
        };
        match name {
            "baseline" => Ok(Self { enable_qm: false, ..base }),
            "qm" => Ok(base),
            "qm-rdotx" => Ok(Self { rdo_tx: Some(true), ..base }),
            "qm-cdef-rdotx" => Ok(Self { rdo_tx: Some(true), cdef: Some(true), ..base }),
            "qm-sgr" => Ok(Self { sgr_full: Some(true), ..base }),
            "qm-lrf" => Ok(Self { sgr_full: Some(true), lru_on_skip: Some(true), ..base }),
            "qm-seg" => Ok(Self { segmentation_complex: Some(true), ..base }),
            "qm-bottomup" => Ok(Self { encode_bottomup: Some(true), ..base }),
            "qm-trellis" => Ok(Self { enable_trellis: true, ..base }),
            "qm-best" => Ok(Self {
                sgr_full: Some(true),
                lru_on_skip: Some(true),
                segmentation_complex: Some(true),
                enable_trellis: true,
                ..base
            }),
            other => anyhow::bail!(
                "Unknown AVIF preset: '{other}'. Available: baseline, qm, qm-rdotx, \
                 qm-cdef-rdotx, qm-sgr, qm-lrf, qm-seg, qm-bottomup, qm-trellis, qm-best"
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
    if config.sgr_full == Some(true) {
        features.push("sgr");
    }
    if config.lru_on_skip == Some(true) {
        features.push("lrf");
    }
    if config.segmentation_complex == Some(true) {
        features.push("seg");
    }
    if config.encode_bottomup == Some(true) {
        features.push("bottomup");
    }
    if config.enable_trellis {
        features.push("trellis");
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
/// Convert 10-bit (0-1023) component to 8-bit (0-255).
fn to_8bit(v: u16) -> u8 {
    // Proper rounding: (v * 255 + 512) / 1023
    ((u32::from(v) * 255 + 512) / 1023).min(255) as u8
}

fn pixel_data_to_rgb8(pd: zenavif::PixelData) -> Result<ImgVec<Rgb8>> {
    match pd {
        zenavif::PixelData::Rgb8(img) => Ok(img),
        zenavif::PixelData::Rgba8(img) => {
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
        zenavif::PixelData::Rgb16(img) => {
            let pixels: Vec<Rgb8> = img
                .pixels()
                .map(|p| RGB {
                    r: to_8bit(p.r),
                    g: to_8bit(p.g),
                    b: to_8bit(p.b),
                })
                .collect();
            Ok(ImgVec::new(pixels, img.width(), img.height()))
        }
        zenavif::PixelData::Rgba16(img) => {
            let pixels: Vec<Rgb8> = img
                .pixels()
                .map(|p| RGB {
                    r: to_8bit(p.r),
                    g: to_8bit(p.g),
                    b: to_8bit(p.b),
                })
                .collect();
            Ok(ImgVec::new(pixels, img.width(), img.height()))
        }
        other => anyhow::bail!(
            "Unexpected AVIF decode output: expected RGB/RGBA 8/16-bit, got {}x{} {:?}",
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
    let sgr_full = config.sgr_full;
    let lru_on_skip = config.lru_on_skip;
    let segmentation_complex = config.segmentation_complex;
    let encode_bottomup = config.encode_bottomup;
    let enable_trellis = config.enable_trellis;
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
                if let Some(v) = sgr_full {
                    enc = enc.with_sgr_full(Some(v));
                }
                if let Some(v) = lru_on_skip {
                    enc = enc.with_lru_on_skip(Some(v));
                }
                if let Some(v) = segmentation_complex {
                    enc = enc.with_segmentation_complex(Some(v));
                }
                if let Some(v) = encode_bottomup {
                    enc = enc.with_encode_bottomup(Some(v));
                }
                if enable_trellis {
                    enc = enc.with_trellis(true);
                }
            }
            #[cfg(not(feature = "avif-imazen"))]
            {
                let _ = (enable_qm, rdo_tx, cdef, sgr_full, lru_on_skip,
                         segmentation_complex, encode_bottomup, enable_trellis);
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
